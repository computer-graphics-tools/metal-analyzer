//! Definition provider implementation.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use dashmap::DashMap;
use tower_lsp::lsp_types::*;
use tracing::{debug, warn};

#[cfg(test)]
use crate::definition::precise_lookup::matches_position;
#[cfg(test)]
use crate::definition::system_lookup::{
    find_scoped_enum_member_offset, find_word_boundary_offset, should_fast_lookup_system_symbol,
    system_builtin_header_candidates,
};
use crate::{
    definition::{
        ast_index::AstIndex,
        clang_nodes::Node,
        compiler::run_ast_dump,
        fallback_lookup::{ref_site_to_location, resolve_by_name, resolve_from_project_index},
        index_cache,
        indexer::build_index,
        perf::GotoDefPerf,
        precise_lookup::{resolve_local_template_parameter, resolve_precise, resolve_precise_def},
        project_graph::ProjectGraph,
        project_index::ProjectIndex,
        symbol_def::SymbolDef,
        symbol_text::line_chars_and_cursor,
        system_lookup::{resolve_fast_system_symbol_location, resolve_system_header_symbol_location},
        utils::{def_to_location, is_system_header, paths_match},
    },
    ide::navigation::{IdeLocation, IdePosition, IdeRange, NavigationTarget},
    metal::builtins::{BuiltinKind, lookup as lookup_builtin},
    syntax::{SyntaxTree, helpers},
    text_pos::utf16_column_of_byte_offset,
    vfs::FileId,
};

/// Provides go-to-definition by querying the Metal compiler's AST.
///
/// Maintains a per-document cache of parsed AST indices so that repeated
/// jumps within the same file are instant.
pub struct DefinitionProvider {
    cache: DashMap<FileId, (String, Arc<AstIndex>)>,
    build_locks: DashMap<FileId, Arc<std::sync::Mutex<()>>>,
    project_index: Arc<ProjectIndex>,
    project_graph: Arc<ProjectGraph>,
    project_graph_depth: AtomicUsize,
    project_graph_max_nodes: AtomicUsize,
    goto_def_perf: GotoDefPerf,
}

impl Default for DefinitionProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl DefinitionProvider {
    pub fn new() -> Self {
        Self {
            cache: DashMap::new(),
            build_locks: DashMap::new(),
            project_index: Arc::new(ProjectIndex::new()),
            project_graph: Arc::new(ProjectGraph::new()),
            project_graph_depth: AtomicUsize::new(3),
            project_graph_max_nodes: AtomicUsize::new(256),
            goto_def_perf: GotoDefPerf::default(),
        }
    }

    pub fn project_index(&self) -> &ProjectIndex {
        &self.project_index
    }

    pub fn log_perf_summary(&self) {
        self.goto_def_perf.log_summary();
    }

    pub fn configure_project_graph_scope(
        &self,
        depth: usize,
        max_nodes: usize,
    ) {
        self.project_graph_depth.store(depth, Ordering::Relaxed);
        self.project_graph_max_nodes.store(max_nodes, Ordering::Relaxed);
    }

    pub fn index_workspace_file(
        &self,
        path: &std::path::Path,
        include_paths: &[String],
    ) -> bool {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => return false,
        };
        let uri = match Url::from_file_path(path) {
            Ok(u) => u,
            Err(_) => return false,
        };
        self.load_or_build_index(&uri, &source, include_paths).is_some()
    }

    pub fn index_document(
        &self,
        uri: &Url,
        source: &str,
        include_paths: &[String],
    ) {
        if let Some((_, load_source)) = self.load_or_build_index(uri, source, include_paths) {
            match load_source {
                IndexLoadSource::Memory => {
                    debug!("Pre-indexing AST memory hit for {uri}");
                },
                IndexLoadSource::Disk => {
                    debug!("Pre-indexing AST cache hit for {uri}");
                },
                IndexLoadSource::AstDump => {
                    debug!("Pre-indexing AST built via dump for {uri}");
                },
            }
        }
    }

    pub fn evict(
        &self,
        uri: &Url,
    ) {
        let file_id = FileId::from_url(uri);
        self.cache.remove(&file_id);
        self.build_locks.remove(&file_id);
    }

    pub fn get_cached_index(
        &self,
        uri: &Url,
    ) -> Option<Arc<AstIndex>> {
        let file_id = FileId::from_url(uri);
        self.cache.get(&file_id).map(|entry| Arc::clone(&entry.1))
    }

    pub fn provide(
        &self,
        uri: &Url,
        position: Position,
        source: &str,
        include_paths: &[String],
        snapshot: &SyntaxTree,
    ) -> Option<NavigationTarget> {
        let started = std::time::Instant::now();
        let mut index_source: Option<&'static str> = None;
        let result = self.provide_inner(uri, position, source, include_paths, snapshot, &mut index_source);
        self.goto_def_perf.record(started.elapsed(), index_source, result.is_some());
        result
    }

    fn provide_inner(
        &self,
        uri: &Url,
        position: Position,
        source: &str,
        include_paths: &[String],
        snapshot: &SyntaxTree,
        index_source: &mut Option<&'static str>,
    ) -> Option<NavigationTarget> {
        let (include_info, word) = {
            let root = snapshot.root();
            let include_info = helpers::include_at_position(&root, source, position)
                .or_else(|| helpers::include_at_position_text_fallback(source, position));
            let word = helpers::navigation_word_at_position(&root, source, position);
            (include_info, word)
        };

        // TIER-0: Handle #include directives
        if let Some((path, is_system)) = include_info {
            debug!("Go-to-definition for include: {path} (system={is_system})");

            let check_path = |p: std::path::PathBuf| -> Option<NavigationTarget> {
                if p.exists() {
                    return Some(NavigationTarget::Single(IdeLocation::new(p, IdeRange::default())));
                }
                None
            };

            for dir in include_paths {
                let dir_path = std::path::Path::new(dir);
                if let Some(loc) = check_path(dir_path.join(&path)) {
                    return Some(loc);
                }
                if is_system && let Some(loc) = check_path(dir_path.join("metal").join(&path)) {
                    return Some(loc);
                }
            }

            if !is_system
                && let Ok(current_path) = uri.to_file_path()
                && let Some(parent) = current_path.parent()
                && let Some(loc) = check_path(parent.join(&path))
            {
                return Some(loc);
            }
        }

        // TIER-1: Extract the word at the cursor
        let word = word?;
        if word.is_empty() {
            return None;
        }
        if is_non_navigable_symbol(word.as_str()) {
            debug!("[goto-def] skipping non-navigable symbol: {word}");
            return None;
        }
        debug!("[goto-def] word={word} at {}:{}", position.line, position.character);

        let source_path = uri.to_file_path().ok();
        let source_file = source_path.as_ref().map(|path| path.display().to_string()).unwrap_or_default();
        let source_file_id = source_path.as_ref().map(|path| FileId::from_path(path));

        if let Some(result) = resolve_local_template_parameter(uri, snapshot, source, position, &word) {
            debug!("[goto-def] TIER-1 (local template param): hit");
            return Some(result);
        }

        // TIER-2: Fast system-header path for obvious Metal SDK symbols
        if let Some(result) = resolve_fast_system_symbol_location(source, position, &word, include_paths) {
            debug!("[goto-def] TIER-2 (system header fast path): hit");
            return Some(result);
        }

        // TIER-4: AST-based resolution (scope-aware via Clang)
        if let Some((index, load_source)) = self.load_or_build_index(uri, source, include_paths) {
            *index_source = Some(load_source.as_str());
            debug!("[goto-def] AST index source: {}", load_source.as_str());

            if let Some(def) = resolve_precise(&index, &source_file, position, &word) {
                debug!("[goto-def] TIER-4 (AST precise): hit");
                return Some(def);
            }
            debug!("[goto-def] TIER-4 (AST precise): miss for {word}, trying ranked fallback");

            // TIER-5: AST by-name fallback with ranking
            if let Some(result) = resolve_by_name(&index, &source_file, source, position, &word) {
                debug!("[goto-def] TIER-5 (AST by-name): hit");
                return Some(result);
            }
            debug!("[goto-def] TIER-5 (AST by-name): miss/ambiguous for {word}");
        } else {
            debug!("[goto-def] AST index unavailable for {uri}; trying fallback tiers");
        }

        // TIER-6: Project-wide index: cross-file definition lookup by name
        if let Some(result) = resolve_from_project_index(
            &self.project_index,
            &self.project_graph,
            &source_file,
            source_file_id.as_ref(),
            self.project_graph_depth.load(Ordering::Relaxed),
            self.project_graph_max_nodes.load(Ordering::Relaxed),
            &word,
            position,
        ) {
            debug!("[goto-def] TIER-6 (project index): hit");
            return Some(result);
        }

        // TIER-7: Builtin/system fallback: map known Metal builtins to SDK headers
        if let Some(result) = resolve_builtin_symbol_location(&word, include_paths) {
            debug!("[goto-def] TIER-7 (system builtin header): hit");
            return Some(result);
        }

        // TIER-8: Macro fallback: search for `#define <word>` in the current file
        if let Some(result) = resolve_macro_definition(uri, source, &word) {
            debug!("[goto-def] TIER-8 (macro fallback): hit");
            return Some(result);
        }

        debug!("[goto-def] no definition found for {word}");
        None
    }

    pub fn provide_declaration(
        &self,
        uri: &Url,
        position: Position,
        source: &str,
        include_paths: &[String],
        snapshot: &SyntaxTree,
    ) -> Option<NavigationTarget> {
        let word = {
            let root = snapshot.root();
            helpers::navigation_word_at_position(&root, source, position)
        }?;

        if word.is_empty() {
            return None;
        }
        if is_non_navigable_symbol(word.as_str()) {
            return None;
        }

        let (index, load_source) = self.load_or_build_index(uri, source, include_paths)?;
        debug!("[goto-declaration] AST index source: {}", load_source.as_str());

        let declarations = index.get_declarations(&word);
        if declarations.is_empty() {
            return self.provide(uri, position, source, include_paths, snapshot);
        }

        let locations: Vec<IdeLocation> = declarations.iter().filter_map(|d| def_to_location(d)).collect();

        NavigationTarget::from_locations(locations)
    }

    pub fn provide_type_definition(
        &self,
        uri: &Url,
        position: Position,
        source: &str,
        include_paths: &[String],
        snapshot: &SyntaxTree,
    ) -> Option<NavigationTarget> {
        let word = {
            let root = snapshot.root();
            helpers::navigation_word_at_position(&root, source, position)
        }?;

        if word.is_empty() {
            return None;
        }
        if is_non_navigable_symbol(word.as_str()) {
            return None;
        }

        let (index, load_source) = self.load_or_build_index(uri, source, include_paths)?;
        debug!("[goto-type-definition] AST index source: {}", load_source.as_str());

        let source_file = uri.to_file_path().ok().map(|p| p.display().to_string()).unwrap_or_default();

        if let Some(def) = resolve_precise_def(&index, &source_file, position, &word)
            && let Some(type_def) = index.get_type_definition(def)
        {
            return def_to_location(type_def).map(NavigationTarget::Single);
        }

        let indices = index.name_to_defs.get(&word)?;
        let candidates: Vec<&SymbolDef> = indices
            .iter()
            .map(|&i| &index.defs[i])
            .filter(|d| matches!(d.kind.as_str(), "CXXRecordDecl" | "TypedefDecl" | "TypeAliasDecl" | "EnumDecl"))
            .collect();
        if candidates.is_empty() {
            return None;
        }

        let locations: Vec<IdeLocation> = candidates.iter().filter_map(|d| def_to_location(d)).collect();
        NavigationTarget::from_locations(locations)
    }

    pub fn provide_implementation(
        &self,
        uri: &Url,
        position: Position,
        source: &str,
        include_paths: &[String],
        snapshot: &SyntaxTree,
    ) -> Option<NavigationTarget> {
        let word = {
            let root = snapshot.root();
            helpers::navigation_word_at_position(&root, source, position)
        }?;

        if word.is_empty() {
            return None;
        }
        if is_non_navigable_symbol(word.as_str()) {
            return None;
        }

        let (index, load_source) = self.load_or_build_index(uri, source, include_paths)?;
        debug!("[goto-implementation] AST index source: {}", load_source.as_str());

        let source_file = uri.to_file_path().ok().map(|p| p.display().to_string()).unwrap_or_default();

        let indices = index.name_to_defs.get(&word)?;

        let candidates: Vec<&SymbolDef> = indices
            .iter()
            .map(|&i| &index.defs[i])
            .filter(|d| {
                d.is_definition
                    && matches!(d.kind.as_str(), "FunctionDecl" | "CXXMethodDecl")
                    && !d.file.is_empty()
                    && d.line > 0
            })
            .collect();

        if candidates.is_empty() {
            return None;
        }

        let user_candidates: Vec<&SymbolDef> =
            candidates.iter().copied().filter(|d| !is_system_header(&d.file)).collect();
        let pool = if !user_candidates.is_empty() {
            &user_candidates
        } else {
            &candidates
        };

        let same_file: Vec<&SymbolDef> = pool.iter().copied().filter(|d| paths_match(&d.file, &source_file)).collect();
        let pool = if !same_file.is_empty() {
            &same_file
        } else {
            pool
        };

        let locations: Vec<IdeLocation> = pool.iter().filter_map(|d| def_to_location(d)).collect();
        NavigationTarget::from_locations(locations)
    }

    pub fn provide_references(
        &self,
        uri: &Url,
        position: Position,
        source: &str,
        include_paths: &[String],
        snapshot: &SyntaxTree,
        include_declaration: bool,
    ) -> Option<Vec<IdeLocation>> {
        let word = {
            let root = snapshot.root();
            helpers::navigation_word_at_position(&root, source, position)
        }?;

        if word.is_empty() {
            return None;
        }
        if is_non_navigable_symbol(word.as_str()) {
            return None;
        }

        let (index, load_source) = self.load_or_build_index(uri, source, include_paths)?;
        debug!("[references] AST index source: {}", load_source.as_str());

        let source_file = uri.to_file_path().ok().map(|p| p.display().to_string()).unwrap_or_default();

        let target_id = if let Some(def) = resolve_precise_def(&index, &source_file, position, &word) {
            Some(def.id.clone())
        } else {
            index.name_to_defs.get(&word)?.first().map(|&idx| index.defs[idx].id.clone())
        }?;

        let mut locations = Vec::new();
        let mut seen = std::collections::HashSet::new();

        if include_declaration
            && let Some(&def_idx) = index.id_to_def.get(&target_id)
            && let Some(loc) = def_to_location(&index.defs[def_idx])
        {
            seen.insert((loc.file_path.clone(), loc.range.start.line, loc.range.start.character));
            locations.push(loc);
        }

        for ref_site in index.get_references(&target_id) {
            if let Some(loc) = ref_site_to_location(ref_site) {
                let key = (loc.file_path.clone(), loc.range.start.line, loc.range.start.character);
                if seen.insert(key) {
                    locations.push(loc);
                }
            }
        }

        if include_declaration {
            for def in self.project_index.find_definitions(&word) {
                if let Some(loc) = def_to_location(&def) {
                    let key = (loc.file_path.clone(), loc.range.start.line, loc.range.start.character);
                    if seen.insert(key) {
                        locations.push(loc);
                    }
                }
            }
        }

        for ref_site in self.project_index.find_references_by_name(&word) {
            if let Some(loc) = ref_site_to_location(&ref_site) {
                let key = (loc.file_path.clone(), loc.range.start.line, loc.range.start.character);
                if seen.insert(key) {
                    locations.push(loc);
                }
            }
        }

        if locations.is_empty() {
            None
        } else {
            Some(locations)
        }
    }

    pub fn prepare_rename(
        &self,
        uri: &Url,
        position: Position,
        source: &str,
        _include_paths: &[String],
        snapshot: &SyntaxTree,
    ) -> Option<IdeRange> {
        let word = {
            let root = snapshot.root();
            helpers::navigation_word_at_position(&root, source, position)
        }?;

        if word.is_empty() {
            return None;
        }
        if is_non_navigable_symbol(word.as_str()) {
            return None;
        }

        let file_id = FileId::from_url(uri);
        let hash = content_hash(source);
        if let Some(entry) = self.cache.get(&file_id).filter(|e| e.0 == hash) {
            let index = &entry.1;
            let source_file = uri.to_file_path().ok().map(|p| p.display().to_string()).unwrap_or_default();

            if let Some(def) = resolve_precise_def(index, &source_file, position, &word)
                && is_system_header(&def.file)
            {
                return None;
            }
        }

        helpers::word_at_position_text_fallback(source, position).and_then(|_w| {
            let (chars, cursor) = line_chars_and_cursor(source, position)?;
            let mut start = cursor;
            let mut end = start;

            while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
                start -= 1;
            }
            while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
                end += 1;
            }

            if start < end {
                let start_utf16: u32 = chars[..start].iter().map(|c| c.len_utf16() as u32).sum();
                let end_utf16: u32 = chars[..end].iter().map(|c| c.len_utf16() as u32).sum();
                Some(IdeRange::new(
                    IdePosition::new(position.line, start_utf16),
                    IdePosition::new(position.line, end_utf16),
                ))
            } else {
                None
            }
        })
    }

    fn load_or_build_index(
        &self,
        uri: &Url,
        source: &str,
        include_paths: &[String],
    ) -> Option<(Arc<AstIndex>, IndexLoadSource)> {
        let file_id = FileId::from_url(uri);
        let source_path = uri.to_file_path().ok();
        if let Some(path) = source_path.as_ref() {
            self.project_graph.update_file(path, source, include_paths);
        }
        let hash = content_hash(source);
        if let Some(entry) = self.cache.get(&file_id).filter(|e| e.0 == hash) {
            debug!("[goto-def] using in-memory AST index ({} defs, {} refs)", entry.1.defs.len(), entry.1.refs.len(),);
            return Some((Arc::clone(&entry.1), IndexLoadSource::Memory));
        }

        let build_lock = self.build_lock(&file_id);
        let _build_guard = build_lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

        if let Some(entry) = self.cache.get(&file_id).filter(|e| e.0 == hash) {
            debug!(
                "[goto-def] using in-memory AST index after wait ({} defs, {} refs)",
                entry.1.defs.len(),
                entry.1.refs.len(),
            );
            return Some((Arc::clone(&entry.1), IndexLoadSource::Memory));
        }

        if let Some(path) = source_path.as_ref()
            && let Some(index) = index_cache::load(path, &hash, include_paths)
        {
            debug!("[goto-def] disk AST index cache hit for {}", path.display());
            self.project_index.update_file(path.clone(), index.clone());
            let idx = Arc::new(index);
            self.cache.insert(file_id.clone(), (hash, Arc::clone(&idx)));
            return Some((idx, IndexLoadSource::Disk));
        }

        debug!("[goto-def] AST cache miss, running AST dump for {uri}");
        let index = self.run_and_build_index(uri, source, include_paths)?;
        if let Some(path) = source_path {
            index_cache::save(&path, &hash, include_paths, &index);
            self.project_index.update_file(path, index.clone());
        }
        let idx = Arc::new(index);
        self.cache.insert(file_id, (hash, Arc::clone(&idx)));
        Some((idx, IndexLoadSource::AstDump))
    }

    fn run_and_build_index(
        &self,
        uri: &Url,
        source: &str,
        include_paths: &[String],
    ) -> Option<AstIndex> {
        let (ast_json, tmp_files) = run_ast_dump(source, uri, include_paths)?;

        let root: Node = match serde_json::from_str(&ast_json) {
            Ok(v) => v,
            Err(error) => {
                warn!("Failed to parse AST JSON: {error}");
                return None;
            },
        };

        let source_path = uri.to_file_path().ok().map(|p| p.display().to_string());
        Some(build_index(&root, &tmp_files, source_path.as_deref()))
    }

    fn build_lock(
        &self,
        file_id: &FileId,
    ) -> Arc<std::sync::Mutex<()>> {
        self.build_locks.entry(file_id.clone()).or_insert_with(|| Arc::new(std::sync::Mutex::new(()))).clone()
    }
}

#[derive(Copy, Clone, Debug)]
enum IndexLoadSource {
    Memory,
    Disk,
    AstDump,
}

impl IndexLoadSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Memory => "memory",
            Self::Disk => "disk",
            Self::AstDump => "ast_dump",
        }
    }
}

fn is_non_navigable_symbol(word: &str) -> bool {
    matches!(word, "static_cast" | "dynamic_cast" | "reinterpret_cast" | "const_cast")
}

fn is_builtin_navigation_candidate(word: &str) -> bool {
    let Some(entry) = lookup_builtin(word) else {
        return false;
    };
    !matches!(entry.kind, BuiltinKind::Keyword | BuiltinKind::Snippet)
}

fn resolve_builtin_symbol_location(
    word: &str,
    include_paths: &[String],
) -> Option<NavigationTarget> {
    if !is_builtin_navigation_candidate(word) {
        return None;
    }

    resolve_system_header_symbol_location(word, include_paths)
}

fn resolve_macro_definition(
    uri: &Url,
    source: &str,
    word: &str,
) -> Option<NavigationTarget> {
    let file_path = uri.to_file_path().ok()?;
    let pattern = format!("#define {word}");
    for (line_idx, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with(&pattern) {
            let col = line.find(&pattern).unwrap_or(0) + "#define ".len();
            let start_col = utf16_column_of_byte_offset(line, col);
            let end_col = utf16_column_of_byte_offset(line, col + word.len());
            let range =
                IdeRange::new(IdePosition::new(line_idx as u32, start_col), IdePosition::new(line_idx as u32, end_col));
            return Some(NavigationTarget::Single(IdeLocation::new(file_path.clone(), range)));
        }
    }
    None
}

fn content_hash(source: &str) -> String {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

#[cfg(test)]
#[path = "../../tests/src/definition/provider_tests.rs"]
mod tests;
