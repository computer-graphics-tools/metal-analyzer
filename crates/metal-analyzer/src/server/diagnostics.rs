use std::collections::{BTreeSet, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::Ordering;

use dashmap::DashMap;
use tower_lsp::lsp_types::{
    Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, Location, Position, Range, Url,
};
use tracing::{debug, info, warn};
use walkdir::{DirEntry, WalkDir};

use crate::progress::ProgressToken;
use crate::metal::compiler::MetalDiagnostic;

use super::state::MetalLanguageServer;
use super::header_owners::{
    collect_included_headers, get_owner_candidates_for_header, is_header_file, normalize_path,
    update_owner_links,
};
use super::settings::ServerSettings;

const HEADER_OWNER_COMPILE_CAP: usize = 256;

impl MetalLanguageServer {
    /// Run the Metal compiler on the document identified by `uri` and publish
    /// the resulting diagnostics to the client.
    pub(crate) async fn run_diagnostics(&self, uri: &Url) {
        let document = match self.document_store.get(uri) {
            Some(d) => d,
            None => {
                warn!("run_diagnostics called for unknown document: {uri}");
                return;
            }
        };
        let text = document.text;
        let version = document.version;

        let generation = next_diagnostic_generation(&self.diagnostics_generation, uri);
        let workspace_roots: Vec<PathBuf> = self
            .workspace_roots
            .read()
            .await
            .iter()
            .filter_map(|f| f.uri.to_file_path().ok())
            .collect();

        let progress = ProgressToken::begin(
            &self.client,
            "Diagnostics",
            Some("Running compiler…".into()),
        )
        .await;
        let workspace_generation = self.workspace_generation.load(Ordering::Relaxed);

        let diagnostics = compile_filtered_diagnostics_for_document(
            &self.compiler,
            &workspace_roots,
            &self.header_owners,
            &self.owner_headers,
            &self.include_paths_cache,
            workspace_generation,
            uri,
            &text,
        )
        .await;

        let count = diagnostics.len();
        if !is_latest_diagnostic_generation(&self.diagnostics_generation, uri, generation) {
            debug!("Skipping stale diagnostics for {uri} (generation={generation})");
            progress.end(Some("Skipped stale diagnostics".to_owned())).await;
            return;
        }

        debug!("Publishing {count} diagnostic(s) for {uri} (v{version}, generation={generation})");

        self.diagnostics_cache
            .insert(uri.clone(), diagnostics.clone());

        self.client
            .publish_diagnostics(uri.clone(), diagnostics, Some(version))
            .await;

        let end_msg = match count {
            0 => "No issues found".to_owned(),
            1 => "1 diagnostic".to_owned(),
            n => format!("{n} diagnostics"),
        };
        progress.end(Some(end_msg)).await;
    }

    /// Collect include paths for a specific file, using workspace-aware discovery.
    ///
    /// Computes include paths that include:
    /// - Ancestors of the file's directory (up to workspace roots)
    /// - Immediate child directories of those ancestors (e.g., `generated/`, `common/`)
    /// - System include paths from the compiler
    pub(crate) async fn include_paths(&self, uri: &Url) -> Vec<String> {
        let workspace_roots: Vec<PathBuf> = self
            .workspace_roots
            .read()
            .await
            .iter()
            .filter_map(|f| f.uri.to_file_path().ok())
            .collect();
        let workspace_generation = self.workspace_generation.load(Ordering::Relaxed);
        compute_include_paths_for_uri_cached(
            &self.compiler,
            uri,
            &workspace_roots,
            &self.include_paths_cache,
            workspace_generation,
        )
        .await
    }

    /// Clear any previously published diagnostics for a document.
    pub(crate) async fn clear_diagnostics(&self, uri: &Url) {
        self.diagnostics_cache.remove(uri);
        self.diagnostics_generation.remove(uri);
        self.client
            .publish_diagnostics(uri.clone(), Vec::new(), None)
            .await;
    }

    /// Create a lightweight handle suitable for passing into `tokio::spawn`.
    pub(crate) async fn clone_for_background(&self) -> BackgroundHandle {
        let workspace_roots = self
            .workspace_roots
            .read()
            .await
            .iter()
            .filter_map(|f| f.uri.to_file_path().ok())
            .collect();
        BackgroundHandle {
            client: self.client.clone(),
            compiler: self.compiler.clone(),
            definition_provider: self.definition_provider.clone(),
            document_store: self.document_store.clone(),
            workspace_roots,
            header_owners: self.header_owners.clone(),
            owner_headers: self.owner_headers.clone(),
            include_paths_cache: self.include_paths_cache.clone(),
            workspace_generation: self.workspace_generation.load(Ordering::Relaxed),
            settings: self.settings.clone(),
        }
    }
}

/// Minimal handle used by background tasks that need access to the
/// definition provider and include-path computation without holding
/// references to the full server state.
pub(crate) struct BackgroundHandle {
    client: tower_lsp::Client,
    compiler: std::sync::Arc<crate::metal::compiler::MetalCompiler>,
    definition_provider: std::sync::Arc<crate::definition::DefinitionProvider>,
    document_store: std::sync::Arc<crate::document::DocumentStore>,
    workspace_roots: Vec<PathBuf>,
    header_owners: std::sync::Arc<DashMap<PathBuf, std::collections::BTreeSet<PathBuf>>>,
    owner_headers: std::sync::Arc<DashMap<PathBuf, std::collections::BTreeSet<PathBuf>>>,
    include_paths_cache: std::sync::Arc<DashMap<PathBuf, (u64, Vec<String>)>>,
    workspace_generation: u64,
    settings: std::sync::Arc<tokio::sync::RwLock<ServerSettings>>,
}

impl BackgroundHandle {
    /// Scan workspace `.metal` files for indexing and diagnostics.
    pub async fn index_workspace(&self) {
        let settings = self.settings.read().await.clone();
        let indexing_enabled = settings.indexing.enabled;
        let workspace_diagnostics_enabled = settings.diagnostics.scope.is_workspace();
        if !indexing_enabled && !workspace_diagnostics_enabled {
            info!(
                "Skipping workspace scan because metal-analyzer.indexing.enabled=false and \
                 metal-analyzer.diagnostics.scope=openFiles"
            );
            return;
        }

        self.compiler.ensure_system_includes_ready().await;
        let max_indexed_file_size = settings.indexing.max_file_size_bytes();
        let excluded_prefixes = build_workspace_scan_exclude_prefixes(
            &self.workspace_roots,
            &settings.indexing.exclude_paths,
        );
        let metal_files = self.discover_metal_files(max_indexed_file_size, &excluded_prefixes);
        let total = metal_files.len();
        if total == 0 {
            info!("No .metal files found in workspace");
            return;
        }

        if indexing_enabled {
            self.run_workspace_indexing(&settings, &metal_files).await;
        } else {
            info!("Skipping workspace indexing because metal-analyzer.indexing.enabled=false");
        }

        if workspace_diagnostics_enabled {
            self.run_workspace_diagnostics(&settings, &metal_files).await;
        }
    }

    async fn run_workspace_indexing(&self, settings: &ServerSettings, metal_files: &[PathBuf]) {
        let total = metal_files.len();
        info!("Indexing {total} .metal file(s) in workspace…");
        let progress = ProgressToken::begin(
            &self.client,
            "Indexing",
            Some(format!("0 / {total} files")),
        )
        .await;

        let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(settings.indexing.concurrency));
        let indexed = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let mut handles = Vec::with_capacity(total);

        for path in metal_files.iter().cloned() {
            let sem = semaphore.clone();
            let provider = self.definition_provider.clone();
            let compiler = self.compiler.clone();
            let roots = self.workspace_roots.clone();
            let count = indexed.clone();
            let header_owners = self.header_owners.clone();
            let owner_headers = self.owner_headers.clone();

            handles.push(tokio::spawn(async move {
                let _permit = sem.acquire().await;
                let include_paths = compute_include_paths_for(&path, &roots, &compiler);
                if let Ok(source) = tokio::fs::read_to_string(&path).await {
                    let headers = collect_included_headers(&path, &source, &include_paths);
                    update_owner_links(&header_owners, &owner_headers, &path, headers);
                }
                let ok = provider.index_workspace_file(&path, &include_paths).await;
                count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                (path, ok)
            }));
        }

        for handle in handles {
            if let Ok((path, ok)) = handle.await {
                let done = indexed.load(std::sync::atomic::Ordering::Relaxed);
                if done % 5 == 0 || done == total {
                    progress
                        .report(Some(format!("{done} / {total} files")), Some((done * 100 / total) as u32))
                        .await;
                }
                if !ok {
                    debug!("Failed to index: {}", path.display());
                }
            }
        }

        let count = self.definition_provider.project_index().file_count();
        info!("Project index complete: {count} file(s) indexed");
        progress
            .end(Some(format!("{count} file(s) indexed")))
            .await;
    }

    async fn run_workspace_diagnostics(&self, settings: &ServerSettings, metal_files: &[PathBuf]) {
        let total = metal_files.len();
        info!("Analyzing diagnostics for {total} .metal file(s) in workspace…");
        let progress = ProgressToken::begin(
            &self.client,
            "Diagnostics",
            Some(format!("0 / {total} files")),
        )
        .await;

        let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(settings.indexing.concurrency));
        let processed = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut handles = Vec::with_capacity(total);

        for path in metal_files.iter().cloned() {
            let sem = semaphore.clone();
            let compiler = self.compiler.clone();
            let workspace_roots = self.workspace_roots.clone();
            let header_owners = self.header_owners.clone();
            let owner_headers = self.owner_headers.clone();
            let include_paths_cache = self.include_paths_cache.clone();
            let workspace_generation = self.workspace_generation;
            let open_documents = self.document_store.clone();
            let client = self.client.clone();
            let count = processed.clone();

            handles.push(tokio::spawn(async move {
                let _permit = sem.acquire().await;
                let result = publish_workspace_diagnostics_for_file(
                    &client,
                    &compiler,
                    &workspace_roots,
                    &header_owners,
                    &owner_headers,
                    &include_paths_cache,
                    workspace_generation,
                    &open_documents,
                    path,
                )
                .await;
                count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                result
            }));
        }

        let mut files_with_diagnostics = 0usize;
        let mut published = 0usize;
        let mut skipped_open = 0usize;
        for handle in handles {
            if let Ok(result) = handle.await {
                let done = processed.load(std::sync::atomic::Ordering::Relaxed);
                if done % 5 == 0 || done == total {
                    progress
                        .report(Some(format!("{done} / {total} files")), Some((done * 100 / total) as u32))
                        .await;
                }

                if result.skipped_open_document {
                    skipped_open += 1;
                    continue;
                }
                if result.published {
                    published += 1;
                    if result.diagnostic_count > 0 {
                        files_with_diagnostics += 1;
                    }
                } else {
                    debug!("Failed to publish workspace diagnostics for {}", result.path.display());
                }
            }
        }

        info!(
            "Workspace diagnostics complete: {published} file(s) published, \
             {files_with_diagnostics} file(s) with diagnostics, {skipped_open} open file(s) skipped"
        );
        let end_message = if files_with_diagnostics == 0 {
            "No issues found".to_string()
        } else {
            format!("{files_with_diagnostics} file(s) with diagnostics")
        };
        progress.end(Some(end_message)).await;
    }

    fn discover_metal_files(
        &self,
        max_file_size_bytes: u64,
        excluded_prefixes: &[PathBuf],
    ) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for root in &self.workspace_roots {
            for entry in WalkDir::new(root)
                .follow_links(true)
                .into_iter()
                .filter_entry(|entry| {
                    should_descend_into_workspace_entry(entry, excluded_prefixes)
                })
                .filter_map(|e| e.ok())
            {
                if !entry.file_type().is_file() {
                    continue;
                }

                let path = entry.path();
                if !path.extension().is_some_and(|ext| ext == "metal") {
                    continue;
                }

                if let Ok(metadata) = entry.metadata()
                    && metadata.len() > max_file_size_bytes
                {
                    debug!(
                        "Skipping large workspace shader file ({} bytes): {}",
                        metadata.len(),
                        path.display()
                    );
                    continue;
                }

                let normalized = normalize_path(path);
                if seen.insert(normalized.clone()) {
                    files.push(normalized);
                }
            }
        }
        files
    }
}

struct WorkspaceDiagnosticsFileResult {
    path: PathBuf,
    published: bool,
    skipped_open_document: bool,
    diagnostic_count: usize,
}

async fn publish_workspace_diagnostics_for_file(
    client: &tower_lsp::Client,
    compiler: &crate::metal::compiler::MetalCompiler,
    workspace_roots: &[PathBuf],
    header_owners: &DashMap<PathBuf, BTreeSet<PathBuf>>,
    owner_headers: &DashMap<PathBuf, BTreeSet<PathBuf>>,
    include_paths_cache: &DashMap<PathBuf, (u64, Vec<String>)>,
    workspace_generation: u64,
    open_documents: &crate::document::DocumentStore,
    path: PathBuf,
) -> WorkspaceDiagnosticsFileResult {
    let Some(uri) = Url::from_file_path(&path).ok() else {
        return WorkspaceDiagnosticsFileResult {
            path,
            published: false,
            skipped_open_document: false,
            diagnostic_count: 0,
        };
    };

    if open_documents.get(&uri).is_some() {
        return WorkspaceDiagnosticsFileResult {
            path,
            published: false,
            skipped_open_document: true,
            diagnostic_count: 0,
        };
    }

    let Ok(source) = tokio::fs::read_to_string(&path).await else {
        return WorkspaceDiagnosticsFileResult {
            path,
            published: false,
            skipped_open_document: false,
            diagnostic_count: 0,
        };
    };

    let diagnostics = compile_filtered_diagnostics_for_document(
        compiler,
        workspace_roots,
        header_owners,
        owner_headers,
        include_paths_cache,
        workspace_generation,
        &uri,
        &source,
    )
    .await;
    let diagnostic_count = diagnostics.len();

    client.publish_diagnostics(uri, diagnostics, None).await;
    WorkspaceDiagnosticsFileResult {
        path,
        published: true,
        skipped_open_document: false,
        diagnostic_count,
    }
}

fn should_descend_into_workspace_entry(entry: &DirEntry, excluded_prefixes: &[PathBuf]) -> bool {
    let normalized = normalize_path(entry.path());
    if is_path_excluded(&normalized, excluded_prefixes) {
        return false;
    }

    if !entry.file_type().is_dir() {
        return true;
    }

    let Some(name) = entry.file_name().to_str() else {
        return false;
    };

    if name.starts_with('.') {
        return false;
    }

    if name.ends_with(".bundle") {
        return false;
    }

    !matches!(
        name,
        "target" | "build" | "node_modules" | "out" | "bin" | "obj" | "DerivedData"
    )
}

fn build_workspace_scan_exclude_prefixes(
    workspace_roots: &[PathBuf],
    exclude_paths: &[String],
) -> Vec<PathBuf> {
    let mut excluded_prefixes = Vec::new();
    let mut seen = HashSet::new();

    for raw_path in exclude_paths {
        let exclude_path = PathBuf::from(raw_path);
        if exclude_path.is_absolute() {
            let normalized = normalize_path(&exclude_path);
            if seen.insert(normalized.clone()) {
                excluded_prefixes.push(normalized);
            }
            continue;
        }

        for workspace_root in workspace_roots {
            let normalized = normalize_path(&workspace_root.join(&exclude_path));
            if seen.insert(normalized.clone()) {
                excluded_prefixes.push(normalized);
            }
        }
    }

    excluded_prefixes
}

fn is_path_excluded(path: &Path, excluded_prefixes: &[PathBuf]) -> bool {
    excluded_prefixes
        .iter()
        .any(|excluded_prefix| path.starts_with(excluded_prefix))
}

/// Compute include paths for a file during project scanning.
fn compute_include_paths_for(
    file: &PathBuf,
    workspace_roots: &[PathBuf],
    compiler: &crate::metal::compiler::MetalCompiler,
) -> Vec<String> {
    let mut paths = crate::metal::compiler::compute_include_paths(file, Some(workspace_roots));
    let system_paths = compiler.get_system_include_paths();
    paths.extend(system_paths.iter().map(|p| p.display().to_string()));
    paths
}

pub(super) async fn compute_include_paths_for_uri_cached(
    compiler: &crate::metal::compiler::MetalCompiler,
    uri: &Url,
    workspace_roots: &[PathBuf],
    include_paths_cache: &DashMap<PathBuf, (u64, Vec<String>)>,
    workspace_generation: u64,
) -> Vec<String> {
    compiler.ensure_system_includes_ready().await;

    let Some(file_path) = uri.to_file_path().ok() else {
        return compiler
            .get_system_include_paths()
            .iter()
            .map(|p| p.display().to_string())
            .collect();
    };
    let cache_key = normalize_path(&file_path);
    if let Some(entry) = include_paths_cache.get(&cache_key)
        && entry.0 == workspace_generation
    {
        return entry.1.clone();
    }

    let mut paths = crate::metal::compiler::compute_include_paths(&cache_key, Some(workspace_roots));
    let system_paths = compiler.get_system_include_paths();
    paths.extend(system_paths.iter().map(|p| p.display().to_string()));
    include_paths_cache.insert(cache_key, (workspace_generation, paths.clone()));
    paths
}

pub(super) async fn compile_filtered_diagnostics_for_document(
    compiler: &crate::metal::compiler::MetalCompiler,
    workspace_roots: &[PathBuf],
    header_owners: &DashMap<PathBuf, BTreeSet<PathBuf>>,
    owner_headers: &DashMap<PathBuf, BTreeSet<PathBuf>>,
    include_paths_cache: &DashMap<PathBuf, (u64, Vec<String>)>,
    workspace_generation: u64,
    uri: &Url,
    text: &str,
) -> Vec<Diagnostic> {
    let target_path = uri.to_file_path().ok().map(|p| normalize_path(&p));
    let strict_file_match = target_path
        .as_ref()
        .is_some_and(|path| is_header_file(path));

    let raw_diagnostics = if strict_file_match {
        if let Some(path) = target_path.as_deref() {
            compile_header_owner_diagnostics(
                compiler,
                workspace_roots,
                header_owners,
                owner_headers,
                include_paths_cache,
                workspace_generation,
                uri,
                path,
            )
            .await
        } else {
            Vec::new()
        }
    } else {
        let include_paths = compute_include_paths_for_uri_cached(
            compiler,
            uri,
            workspace_roots,
            include_paths_cache,
            workspace_generation,
        )
        .await;
        compiler
            .compile_with_include_paths(text, uri.as_str(), &include_paths)
            .await
    };

    filter_target_diagnostics(raw_diagnostics, target_path.as_deref(), strict_file_match)
}

async fn compile_header_owner_diagnostics(
    compiler: &crate::metal::compiler::MetalCompiler,
    workspace_roots: &[PathBuf],
    header_owners: &DashMap<PathBuf, BTreeSet<PathBuf>>,
    owner_headers: &DashMap<PathBuf, BTreeSet<PathBuf>>,
    include_paths_cache: &DashMap<PathBuf, (u64, Vec<String>)>,
    workspace_generation: u64,
    header_uri: &Url,
    header_path: &Path,
) -> Vec<MetalDiagnostic> {
    let normalized_header = normalize_path(header_path);
    let mut owners = get_owner_candidates_for_header(
        header_owners,
        &normalized_header,
        HEADER_OWNER_COMPILE_CAP,
    );
    if owners.is_empty() {
        owners = discover_header_owners_on_demand(
            compiler,
            workspace_roots,
            header_owners,
            owner_headers,
            &normalized_header,
        )
        .await;
    }
    if owners.is_empty() {
        debug!(
            "No owner `.metal` files found for header {}; skipping standalone diagnostics",
            header_uri
        );
        return Vec::new();
    }

    let mut diagnostics = Vec::new();
    for owner in owners.into_iter().take(HEADER_OWNER_COMPILE_CAP) {
        let Ok(owner_uri) = Url::from_file_path(&owner) else {
            continue;
        };
        let Ok(source) = tokio::fs::read_to_string(&owner).await else {
            continue;
        };
        let include_paths = compute_include_paths_for_uri_cached(
            compiler,
            &owner_uri,
            workspace_roots,
            include_paths_cache,
            workspace_generation,
        )
        .await;
        let mut owner_diags = compiler
            .compile_with_include_paths(&source, owner_uri.as_str(), &include_paths)
            .await;
        diagnostics.append(&mut owner_diags);
    }
    diagnostics
}

async fn discover_header_owners_on_demand(
    compiler: &crate::metal::compiler::MetalCompiler,
    workspace_roots: &[PathBuf],
    header_owners: &DashMap<PathBuf, BTreeSet<PathBuf>>,
    owner_headers: &DashMap<PathBuf, BTreeSet<PathBuf>>,
    normalized_header: &Path,
) -> Vec<PathBuf> {
    let mut owners = Vec::new();
    for root in workspace_roots {
        for entry in WalkDir::new(root)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.extension().is_some_and(|ext| ext == "metal") {
                continue;
            }
            let Ok(source) = tokio::fs::read_to_string(path).await else {
                continue;
            };
            let include_paths =
                compute_include_paths_for(&path.to_path_buf(), workspace_roots, compiler);
            let headers = collect_included_headers(path, &source, &include_paths);
            update_owner_links(header_owners, owner_headers, path, headers.clone());
            if headers.contains(normalized_header) {
                owners.push(path.to_path_buf());
            }
        }
    }

    owners.sort();
    owners.dedup();
    owners
}

fn filter_target_diagnostics(
    diagnostics: Vec<MetalDiagnostic>,
    target_path: Option<&std::path::Path>,
    strict_file_match: bool,
) -> Vec<Diagnostic> {
    let target = target_path.map(|p| p.display().to_string());
    let mut dedupe = HashSet::new();
    let mut out: Vec<Diagnostic> = Vec::new();
    // Tracks whether the most recent primary was kept so that only its
    // immediately-following notes are attached (avoids misattribution when
    // a dropped primary's notes would otherwise land on an unrelated primary).
    let mut last_primary_kept = false;

    for diag in diagnostics {
        if diag.severity == DiagnosticSeverity::INFORMATION {
            if last_primary_kept {
                let note_location = diag
                    .file
                    .as_deref()
                    .and_then(|f| Url::from_file_path(f).ok())
                    .map(|uri| {
                        let pos = Position::new(diag.line, diag.column);
                        Location {
                            uri,
                            range: Range::new(pos, pos),
                        }
                    });
                if let Some(location) = note_location {
                    let last = out.last_mut().expect("last_primary_kept implies non-empty");
                    let related = last.related_information.get_or_insert_with(Vec::new);
                    related.push(DiagnosticRelatedInformation {
                        location,
                        message: diag.message,
                    });
                }
            }
            continue;
        }
        if should_suppress_primary_diagnostic(&diag) {
            last_primary_kept = false;
            continue;
        }

        let belongs = match (strict_file_match, target.as_deref(), diag.file.as_deref()) {
            (true, Some(target), Some(file)) => diagnostic_paths_match(file, target),
            (true, Some(_), None) => false,
            (false, Some(target), Some(file)) => diagnostic_paths_match(file, target),
            (false, Some(_), None) => true,
            (false, None, _) => true,
            (true, None, _) => true,
        };
        if !belongs {
            last_primary_kept = false;
            continue;
        }
        let key = (
            diag.line,
            diag.column,
            format!("{:?}", diag.severity),
            diag.message.clone(),
        );
        if !dedupe.insert(key) {
            last_primary_kept = false;
            continue;
        }
        last_primary_kept = true;
        out.push(diag.into_lsp_diagnostic());
    }

    out
}

fn should_suppress_primary_diagnostic(diag: &MetalDiagnostic) -> bool {
    diag.severity == DiagnosticSeverity::WARNING && diag.message.contains("[-Wmacro-redefined]")
}

/// Match diagnostic file paths conservatively.
///
/// For diagnostics we intentionally avoid the filename-only fallback used by
/// definition resolution, since it can misattribute errors from `utils.h` in
/// another directory to the currently opened `utils.h`.
fn diagnostic_paths_match(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }

    let pa = Path::new(a);
    let pb = Path::new(b);

    // Prefer canonical comparison when both files exist.
    if let (Ok(ca), Ok(cb)) = (pa.canonicalize(), pb.canonicalize()) {
        return ca == cb;
    }

    // Fallback to purely lexical normalization for absolute paths.
    // This handles `.` / `..` differences without accepting basename-only matches.
    match (normalize_absolute_path(pa), normalize_absolute_path(pb)) {
        (Some(na), Some(nb)) => na == nb,
        _ => false,
    }
}

fn normalize_absolute_path(path: &Path) -> Option<PathBuf> {
    if !path.is_absolute() {
        return None;
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                let _ = normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    Some(normalized)
}

fn next_diagnostic_generation(generations: &DashMap<Url, u64>, uri: &Url) -> u64 {
    let mut generation = generations.entry(uri.clone()).or_insert(0);
    *generation += 1;
    *generation
}

fn is_latest_diagnostic_generation(generations: &DashMap<Url, u64>, uri: &Url, value: u64) -> bool {
    generations.get(uri).is_some_and(|current| *current == value)
}

#[cfg(test)]
#[path = "../../tests/src/server/diagnostics_tests.rs"]
mod tests;
