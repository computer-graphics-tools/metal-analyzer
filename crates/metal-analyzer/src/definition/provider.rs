//! Definition provider implementation.

use std::sync::Arc;

use dashmap::DashMap;
use tower_lsp::lsp_types::*;
use tracing::{debug, warn};

use crate::syntax::ast::{self, AstNode};
use crate::syntax::kind::SyntaxKind;
use crate::syntax::SyntaxTree;
use crate::syntax::helpers;

use super::ast_index::AstIndex;
use super::clang_nodes::Node;
use super::compiler::run_ast_dump;
use super::indexer::build_index;
use super::index_cache;
use super::project_index::ProjectIndex;
use super::ref_site::RefSite;
use super::symbol_def::SymbolDef;
use super::utils::{def_to_location, is_system_header, paths_match};

/// Provides go-to-definition by querying the Metal compiler's AST.
///
/// Maintains a per-document cache of parsed AST indices so that repeated
/// jumps within the same file are instant.
pub struct DefinitionProvider {
    /// Cache: document URI → (content_hash, AstIndex).
    cache: DashMap<Url, (String, Arc<AstIndex>)>,
    /// Per-document build locks to avoid duplicate AST dumps under bursty requests.
    build_locks: DashMap<Url, Arc<tokio::sync::Mutex<()>>>,
    /// Project-wide AST index for cross-file navigation.
    project_index: Arc<ProjectIndex>,
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
        }
    }

    /// Access the project-wide AST index.
    pub fn project_index(&self) -> &ProjectIndex {
        &self.project_index
    }

    /// Index a workspace file into the project-wide index.
    ///
    /// Call this during initial workspace scan and on file saves.
    pub async fn index_workspace_file(
        &self,
        path: &std::path::Path,
        include_paths: &[String],
    ) -> bool {
        let source = match tokio::fs::read_to_string(path).await {
            Ok(s) => s,
            Err(_) => return false,
        };
        let uri = match Url::from_file_path(path) {
            Ok(u) => u,
            Err(_) => return false,
        };
        self.load_or_build_index(&uri, &source, include_paths)
            .await
            .is_some()
    }

    /// Pre-index a document in the background.
    ///
    /// Call this from `did_open` / `did_save` so the index is ready before
    /// the user requests go-to-definition. If the source text hasn't changed
    /// since the last indexing (same content hash), this is a no-op.
    pub async fn index_document(&self, uri: &Url, source: &str, include_paths: &[String]) {
        if let Some((_, load_source)) = self.load_or_build_index(uri, source, include_paths).await {
            match load_source {
                IndexLoadSource::Memory => {
                    debug!("Pre-indexing AST memory hit for {uri}");
                }
                IndexLoadSource::Disk => {
                    debug!("Pre-indexing AST cache hit for {uri}");
                }
                IndexLoadSource::AstDump => {
                    debug!("Pre-indexing AST built via dump for {uri}");
                }
            }
        }
    }

    /// Drop the cached index for a document (on close).
    pub fn evict(&self, uri: &Url) {
        self.cache.remove(uri);
        self.build_locks.remove(uri);
    }

    /// Retrieve the cached AST index for a given URI, if available.
    pub fn get_cached_index(&self, uri: &Url) -> Option<Arc<AstIndex>> {
        self.cache.get(uri).map(|entry| Arc::clone(&entry.1))
    }

    /// Resolve the definition of the symbol at `position` in the document.
    ///
    /// Uses a multi-tier approach:
    /// 1. Fast regex-based same-file lookup (~1-5ms)
    /// 2. Cached AST index (instant if available)
    /// 3. Full AST dump (1-3 seconds, only if needed)
    pub async fn provide(
        &self,
        uri: &Url,
        position: Position,
        source: &str,
        include_paths: &[String],
        snapshot: &SyntaxTree,
    ) -> Option<GotoDefinitionResponse> {
        let (include_info, word) = {
            let root = snapshot.root();
            let include_info = helpers::include_at_position(&root, source, position)
                .or_else(|| helpers::include_at_position_text_fallback(source, position));
            let word = helpers::navigation_word_at_position(&root, source, position);
            (include_info, word)
        };

        // 0. Handle #include directives (fast path for headers)
        if let Some((path, is_system)) = include_info {
            debug!("Go-to-definition for include: {path} (system={is_system})");

            let check_path = |p: std::path::PathBuf| -> Option<GotoDefinitionResponse> {
                if p.exists()
                    && let Ok(target_uri) = Url::from_file_path(p)
                {
                    return Some(GotoDefinitionResponse::Scalar(Location {
                        uri: target_uri,
                        range: Range::default(),
                    }));
                }
                None
            };

            for dir in include_paths {
                let dir_path = std::path::Path::new(dir);
                if let Some(loc) = check_path(dir_path.join(&path)) {
                    return Some(loc);
                }
                if is_system
                    && path == "metal_stdlib"
                    && let Some(loc) = check_path(dir_path.join("metal/metal_stdlib"))
                {
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

        // 1. Extract the word at the cursor.
        let word = word?;
        if word.is_empty() {
            return None;
        }
        if is_non_navigable_symbol(word.as_str()) {
            debug!("[goto-def] skipping non-navigable symbol: {word}");
            return None;
        }
        debug!("[goto-def] word={word} at {}:{}", position.line, position.character);

        if let Some(result) = resolve_local_template_parameter(uri, snapshot, source, position, &word) {
            debug!("[goto-def] TIER-1 (local template param): hit");
            return Some(result);
        }

        // 2. AST-based resolution (scope-aware via Clang).
        let (index, load_source) = self.load_or_build_index(uri, source, include_paths).await?;
        debug!("[goto-def] AST index source: {}", load_source.as_str());

        let source_file = uri
            .to_file_path()
            .ok()
            .map(|p| p.display().to_string())
            .unwrap_or_default();

        if let Some(def) = resolve_precise(&index, &source_file, position, &word) {
            debug!("[goto-def] TIER-4 (AST precise): hit");
            return Some(def);
        }
        debug!("[goto-def] TIER-4 (AST precise): miss for {word}, trying ranked fallback");

        if let Some(result) = resolve_by_name(&index, &source_file, source, position, &word) {
            debug!("[goto-def] TIER-5 (AST by-name): hit");
            return Some(result);
        }
        debug!("[goto-def] TIER-5 (AST by-name): miss/ambiguous for {word}");

        // 6. Project-wide index: cross-file definition lookup by name.
        if let Some(result) = resolve_from_project_index(&self.project_index, &source_file, &word)
        {
            debug!("[goto-def] TIER-6 (project index): hit");
            return Some(result);
        }

        // 7. Macro fallback: search for `#define <word>` in the current file.
        //    Clang's AST doesn't include preprocessor macro definitions, so
        //    this text-based fallback is needed for go-to-def on macros.
        if let Some(result) = resolve_macro_definition(uri, source, &word) {
            debug!("[goto-def] TIER-7 (macro fallback): hit");
            return Some(result);
        }

        debug!("[goto-def] no definition found for {word}");
        None
    }

    /// Resolve the declaration of the symbol at `position`.
    /// Returns declarations (non-definitions) for the symbol.
    pub async fn provide_declaration(
        &self,
        uri: &Url,
        position: Position,
        source: &str,
        include_paths: &[String],
        snapshot: &SyntaxTree,
    ) -> Option<GotoDefinitionResponse> {
        let word = {
            let root = snapshot.root();
            helpers::navigation_word_at_position(&root, source, position)
        }?;

        if word.is_empty() {
            return None;
        }
        if is_non_navigable_symbol(word.as_str()) {
            debug!("[goto-declaration] skipping non-navigable symbol: {word}");
            return None;
        }

        let (index, load_source) = self.load_or_build_index(uri, source, include_paths).await?;
        debug!("[goto-declaration] AST index source: {}", load_source.as_str());

        let declarations = index.get_declarations(&word);
        if declarations.is_empty() {
            return self
                .provide(uri, position, source, include_paths, snapshot)
                .await;
        }

        let locations: Vec<Location> = declarations
            .iter()
            .filter_map(|d| def_to_location(d))
            .collect();

        match locations.len() {
            0 => None,
            1 => Some(GotoDefinitionResponse::Scalar(locations[0].clone())),
            _ => Some(GotoDefinitionResponse::Array(locations)),
        }
    }

    /// Resolve the type definition for the symbol at `position`.
    /// For variables/fields/parameters, jumps to the type's definition.
    pub async fn provide_type_definition(
        &self,
        uri: &Url,
        position: Position,
        source: &str,
        include_paths: &[String],
        snapshot: &SyntaxTree,
    ) -> Option<GotoDefinitionResponse> {
        let word = {
            let root = snapshot.root();
            helpers::navigation_word_at_position(&root, source, position)
        }?;

        if word.is_empty() {
            return None;
        }
        if is_non_navigable_symbol(word.as_str()) {
            debug!("[goto-type-definition] skipping non-navigable symbol: {word}");
            return None;
        }

        let (index, load_source) = self.load_or_build_index(uri, source, include_paths).await?;
        debug!("[goto-type-definition] AST index source: {}", load_source.as_str());

        let source_file = uri
            .to_file_path()
            .ok()
            .map(|p| p.display().to_string())
            .unwrap_or_default();

        if let Some(def) = resolve_precise_def(&index, &source_file, position, &word)
            && let Some(type_def) = index.get_type_definition(def) {
                return def_to_location(type_def).map(GotoDefinitionResponse::Scalar);
            }

        let indices = index.name_to_defs.get(&word)?;
        let candidates: Vec<&SymbolDef> = indices
            .iter()
            .map(|&i| &index.defs[i])
            .filter(|d| {
                matches!(
                    d.kind.as_str(),
                    "CXXRecordDecl" | "TypedefDecl" | "TypeAliasDecl" | "EnumDecl"
                )
            })
            .collect();
        if candidates.is_empty() {
            return None;
        }

        let locations: Vec<Location> = candidates
            .iter()
            .filter_map(|d| def_to_location(d))
            .collect();
        match locations.len() {
            0 => None,
            1 => Some(GotoDefinitionResponse::Scalar(locations[0].clone())),
            _ => Some(GotoDefinitionResponse::Array(locations)),
        }
    }

    /// Resolve implementations for the symbol at `position`.
    /// Currently returns definitions (in the future could distinguish interfaces).
    pub async fn provide_implementation(
        &self,
        uri: &Url,
        position: Position,
        source: &str,
        include_paths: &[String],
        snapshot: &SyntaxTree,
    ) -> Option<GotoDefinitionResponse> {
        let word = {
            let root = snapshot.root();
            helpers::navigation_word_at_position(&root, source, position)
        }?;

        if word.is_empty() {
            return None;
        }
        if is_non_navigable_symbol(word.as_str()) {
            debug!("[goto-implementation] skipping non-navigable symbol: {word}");
            return None;
        }

        let (index, load_source) = self.load_or_build_index(uri, source, include_paths).await?;
        debug!("[goto-implementation] AST index source: {}", load_source.as_str());

        let source_file = uri
            .to_file_path()
            .ok()
            .map(|p| p.display().to_string())
            .unwrap_or_default();

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

        let user_candidates: Vec<&SymbolDef> = candidates
            .iter()
            .copied()
            .filter(|d| !is_system_header(&d.file))
            .collect();
        let pool = if !user_candidates.is_empty() {
            &user_candidates
        } else {
            &candidates
        };

        let same_file: Vec<&SymbolDef> = pool
            .iter()
            .copied()
            .filter(|d| paths_match(&d.file, &source_file))
            .collect();
        let pool = if !same_file.is_empty() {
            &same_file
        } else {
            pool
        };

        let locations: Vec<Location> = pool.iter().filter_map(|d| def_to_location(d)).collect();
        match locations.len() {
            0 => None,
            1 => Some(GotoDefinitionResponse::Scalar(locations[0].clone())),
            _ => Some(GotoDefinitionResponse::Array(locations)),
        }
    }

    /// Find all references to the symbol at `position`.
    pub async fn provide_references(
        &self,
        uri: &Url,
        position: Position,
        source: &str,
        include_paths: &[String],
        snapshot: &SyntaxTree,
        include_declaration: bool,
    ) -> Option<Vec<Location>> {
        let word = {
            let root = snapshot.root();
            helpers::navigation_word_at_position(&root, source, position)
        }?;

        if word.is_empty() {
            return None;
        }
        if is_non_navigable_symbol(word.as_str()) {
            debug!("[references] skipping non-navigable symbol: {word}");
            return None;
        }

        let (index, load_source) = self.load_or_build_index(uri, source, include_paths).await?;
        debug!("[references] AST index source: {}", load_source.as_str());

        let source_file = uri
            .to_file_path()
            .ok()
            .map(|p| p.display().to_string())
            .unwrap_or_default();

        let target_id =
            if let Some(def) = resolve_precise_def(&index, &source_file, position, &word) {
                Some(def.id.clone())
            } else {
                index
                    .name_to_defs
                    .get(&word)?
                    .first()
                    .map(|&idx| index.defs[idx].id.clone())
            }?;

        let mut locations = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // Per-file AST references (same translation unit).
        if include_declaration
            && let Some(&def_idx) = index.id_to_def.get(&target_id)
            && let Some(loc) = def_to_location(&index.defs[def_idx])
        {
            seen.insert((loc.uri.clone(), loc.range.start.line, loc.range.start.character));
            locations.push(loc);
        }

        for ref_site in index.get_references(&target_id) {
            if let Some(loc) = ref_site_to_location(ref_site) {
                let key = (loc.uri.clone(), loc.range.start.line, loc.range.start.character);
                if seen.insert(key) {
                    locations.push(loc);
                }
            }
        }

        // Project-wide references by name (cross-file).
        if include_declaration {
            for def in self.project_index.find_definitions(&word) {
                if let Some(loc) = def_to_location(&def) {
                    let key = (loc.uri.clone(), loc.range.start.line, loc.range.start.character);
                    if seen.insert(key) {
                        locations.push(loc);
                    }
                }
            }
        }

        for ref_site in self.project_index.find_references_by_name(&word) {
            if let Some(loc) = ref_site_to_location(&ref_site) {
                let key = (loc.uri.clone(), loc.range.start.line, loc.range.start.character);
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

    /// Prepare rename - check if the symbol at position can be renamed.
    pub async fn prepare_rename(
        &self,
        uri: &Url,
        position: Position,
        source: &str,
        _include_paths: &[String],
        snapshot: &SyntaxTree,
    ) -> Option<Range> {
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

        let hash = content_hash(source);
        if let Some(entry) = self.cache.get(uri).filter(|e| e.0 == hash) {
            let index = &entry.1;
            let source_file = uri
                .to_file_path()
                .ok()
                .map(|p| p.display().to_string())
                .unwrap_or_default();

            if let Some(def) = resolve_precise_def(index, &source_file, position, &word)
                && is_system_header(&def.file)
            {
                return None;
            }
        }

        helpers::word_at_position_text_fallback(source, position).and_then(|_w| {
            let line = source.lines().nth(position.line as usize)?;
            let chars: Vec<char> = line.chars().collect();
            let mut start = position.character as usize;
            let mut end = start;

            while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
                start -= 1;
            }
            while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
                end += 1;
            }

            if start < end {
                Some(Range {
                    start: Position {
                        line: position.line,
                        character: start as u32,
                    },
                    end: Position {
                        line: position.line,
                        character: end as u32,
                    },
                })
            } else {
                None
            }
        })
    }

    async fn load_or_build_index(
        &self,
        uri: &Url,
        source: &str,
        include_paths: &[String],
    ) -> Option<(Arc<AstIndex>, IndexLoadSource)> {
        let hash = content_hash(source);
        if let Some(entry) = self.cache.get(uri).filter(|e| e.0 == hash) {
            debug!(
                "[goto-def] using in-memory AST index ({} defs, {} refs)",
                entry.1.defs.len(),
                entry.1.refs.len(),
            );
            return Some((Arc::clone(&entry.1), IndexLoadSource::Memory));
        }

        let build_lock = self.build_lock(uri);
        let _build_guard = build_lock.lock().await;

        // Re-check after lock acquisition in case another request already built it.
        if let Some(entry) = self.cache.get(uri).filter(|e| e.0 == hash) {
            debug!(
                "[goto-def] using in-memory AST index after wait ({} defs, {} refs)",
                entry.1.defs.len(),
                entry.1.refs.len(),
            );
            return Some((Arc::clone(&entry.1), IndexLoadSource::Memory));
        }

        if let Ok(path) = uri.to_file_path()
            && let Some(index) = index_cache::load(&path, &hash, include_paths).await
        {
            debug!("[goto-def] disk AST index cache hit for {}", path.display());
            self.project_index.update_file(path.clone(), index.clone());
            let idx = Arc::new(index);
            self.cache.insert(uri.clone(), (hash, Arc::clone(&idx)));
            return Some((idx, IndexLoadSource::Disk));
        }

        debug!("[goto-def] AST cache miss, running AST dump for {uri}");
        let index = self.run_and_build_index(uri, source, include_paths).await?;
        if let Ok(path) = uri.to_file_path() {
            index_cache::save(&path, &hash, include_paths, &index).await;
            self.project_index.update_file(path, index.clone());
        }
        let idx = Arc::new(index);
        self.cache.insert(uri.clone(), (hash, Arc::clone(&idx)));
        Some((idx, IndexLoadSource::AstDump))
    }

    async fn run_and_build_index(
        &self,
        uri: &Url,
        source: &str,
        include_paths: &[String],
    ) -> Option<AstIndex> {
        let (ast_json, tmp_files) = run_ast_dump(source, uri, include_paths).await?;

        let root: Node = match serde_json::from_str(&ast_json) {
            Ok(v) => v,
            Err(e) => {
                warn!("Failed to parse AST JSON: {e}");
                return None;
            }
        };

        let source_path = uri.to_file_path().ok().map(|p| p.display().to_string());
        Some(build_index(&root, &tmp_files, source_path.as_deref()))
    }

    fn build_lock(&self, uri: &Url) -> Arc<tokio::sync::Mutex<()>> {
        self.build_locks
            .entry(uri.clone())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
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

fn resolve_local_template_parameter(
    uri: &Url,
    snapshot: &SyntaxTree,
    source: &str,
    position: Position,
    word: &str,
) -> Option<GotoDefinitionResponse> {
    let root = snapshot.root();
    let node = helpers::node_at_position(&root, source, position)?;

    for declaration_node in node.ancestors().filter(|ancestor| {
        matches!(
            ancestor.kind(),
            SyntaxKind::FunctionDef | SyntaxKind::StructDef | SyntaxKind::ClassDef
        )
    }) {
        let Some(template_def) = nearest_preceding_template_def(&root, &declaration_node) else {
            continue;
        };
        let Some(template_def) = ast::TemplateDef::cast(template_def) else {
            continue;
        };

        let mut matches = template_def
            .parameters()
            .filter_map(|param| param.name_token())
            .filter(|name| name.text() == word);
        let first = matches.next()?;
        if matches.next().is_some() {
            // Duplicate names in the same template list are ambiguous.
            return None;
        }
        let range = helpers::range_to_lsp(first.text_range(), source);
        return Some(GotoDefinitionResponse::Scalar(Location {
            uri: uri.clone(),
            range,
        }));
    }

    None
}

fn nearest_preceding_template_def(
    root: &crate::syntax::cst::SyntaxNode,
    declaration_node: &crate::syntax::cst::SyntaxNode,
) -> Option<crate::syntax::cst::SyntaxNode> {
    let declaration_start = declaration_node.text_range().start();

    root.descendants()
        .filter(|node| node.kind() == SyntaxKind::TemplateDef)
        .filter(|node| node.text_range().end() <= declaration_start)
        .max_by_key(|node| node.text_range().end())
}

fn resolve_precise(
    index: &AstIndex,
    source_file: &str,
    position: Position,
    word: &str,
) -> Option<GotoDefinitionResponse> {
    resolve_precise_def(index, source_file, position, word)
        .and_then(|def| def_to_location(def).map(GotoDefinitionResponse::Scalar))
}

fn resolve_precise_def<'a>(
    index: &'a AstIndex,
    source_file: &str,
    position: Position,
    word: &str,
) -> Option<&'a SymbolDef> {
    let cursor_line = position.line + 1;
    let cursor_col = position.character + 1;

    for r in &index.refs {
        let Some(matched_site) = match_ref_site(r, source_file, cursor_line, cursor_col) else {
            continue;
        };
        if r.target_name != word {
            continue;
        }

        if let Some(&def_idx) = index.id_to_def.get(&r.target_id) {
            let def = &index.defs[def_idx];
            // Macro-expanded references frequently point to synthetic parameter
            // declarations at macro call-sites. Suppress those to avoid wrong
            // deterministic jumps; fallback may still find a better candidate.
            if !matches!(matched_site, MatchSite::Primary)
                && matches!(def.kind.as_str(), "ParmVarDecl")
            {
                continue;
            }
            debug!(
                "Precise ({matched_site}): {} → {}:{}:{}",
                word, def.file, def.line, def.col
            );
            return Some(def);
        }
    }

    None
}

fn match_ref_site(
    r: &RefSite,
    source_file: &str,
    cursor_line: u32,
    cursor_col: u32,
) -> Option<MatchSite> {
    if matches_position(&r.file, r.line, r.col, r.tok_len, source_file, cursor_line, cursor_col) {
        return Some(MatchSite::Primary);
    }
    if let Some(expansion) = &r.expansion
        && matches_position(
            &expansion.file,
            expansion.line,
            expansion.col,
            expansion.tok_len,
            source_file,
            cursor_line,
            cursor_col,
        )
    {
        return Some(MatchSite::Expansion);
    }
    if let Some(spelling) = &r.spelling
        && matches_position(
            &spelling.file,
            spelling.line,
            spelling.col,
            spelling.tok_len,
            source_file,
            cursor_line,
            cursor_col,
        )
    {
        return Some(MatchSite::Spelling);
    }
    None
}

#[derive(Copy, Clone)]
enum MatchSite {
    Primary,
    Expansion,
    Spelling,
}

impl std::fmt::Display for MatchSite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Primary => "primary",
            Self::Expansion => "expansion",
            Self::Spelling => "spelling",
        };
        f.write_str(name)
    }
}

fn matches_position(
    file: &str,
    line: u32,
    col: u32,
    tok_len: u32,
    source_file: &str,
    cursor_line: u32,
    cursor_col: u32,
) -> bool {
    if !paths_match(file, source_file) {
        return false;
    }
    if line != cursor_line {
        return false;
    }
    let token_end = col.saturating_add(tok_len);
    cursor_col >= col && cursor_col <= token_end
}

fn resolve_by_name(
    index: &AstIndex,
    source_file: &str,
    source: &str,
    position: Position,
    word: &str,
) -> Option<GotoDefinitionResponse> {
    let indices = index.name_to_defs.get(word)?;

    let all_defs: Vec<&SymbolDef> = indices
        .iter()
        .map(|&i| &index.defs[i])
        .filter(|d| !d.file.is_empty() && d.line > 0)
        .collect();

    if all_defs.is_empty() {
        return None;
    }

    let mut seen = std::collections::HashSet::new();
    let mut deduped: Vec<&SymbolDef> = all_defs
        .iter()
        .filter(|d| seen.insert((&d.file, d.line, d.col)))
        .copied()
        .collect();

    if deduped.is_empty() {
        return None;
    }

    deduped.sort_by(|a, b| {
        rank_definition(word, a, source_file)
            .cmp(&rank_definition(word, b, source_file))
            .then_with(|| a.file.cmp(&b.file))
            .then_with(|| a.line.cmp(&b.line))
            .then_with(|| a.col.cmp(&b.col))
    });

    let best = deduped.first().copied()?;
    let best_rank = rank_definition(word, best, source_file);
    let has_tie = deduped
        .get(1)
        .is_some_and(|second| rank_definition(word, second, source_file) == best_rank);
    if has_tie {
        let tied: Vec<&SymbolDef> = deduped
            .iter()
            .copied()
            .take_while(|candidate| rank_definition(word, candidate, source_file) == best_rank)
            .collect();

        if let Some(disambiguated) =
            disambiguate_member_field_tie(index, &tied, source_file, source, position, word)
        {
            debug!(
                "[goto-def] TIER-5 disambiguated member field '{word}' to {}:{}:{}",
                disambiguated.file, disambiguated.line, disambiguated.col
            );
            return def_to_location(disambiguated).map(GotoDefinitionResponse::Scalar);
        }

        debug!(
            "[goto-def] TIER-5 ambiguous for '{word}' (top rank tie), suppressing fallback hit"
        );
        return None;
    }

    debug!(
        "[goto-def] TIER-5 candidate for '{word}': {}:{}:{} kind={}",
        best.file, best.line, best.col, best.kind
    );
    def_to_location(best).map(GotoDefinitionResponse::Scalar)
}

fn ref_site_to_location(ref_site: &RefSite) -> Option<Location> {
    let (file, line, col, tok_len) = if let Some(loc) = ref_site.expansion.as_ref() {
        (&loc.file, loc.line, loc.col, loc.tok_len)
    } else {
        (&ref_site.file, ref_site.line, ref_site.col, ref_site.tok_len)
    };
    let uri = Url::from_file_path(std::path::Path::new(file)).ok()?;
    Some(Location {
        uri,
        range: Range::new(
            Position::new(line.saturating_sub(1), col.saturating_sub(1)),
            Position::new(
                line.saturating_sub(1),
                col.saturating_sub(1) + tok_len,
            ),
        ),
    })
}

fn resolve_from_project_index(
    project_index: &ProjectIndex,
    source_file: &str,
    word: &str,
) -> Option<GotoDefinitionResponse> {
    let defs = project_index.find_definitions(word);
    if defs.is_empty() {
        return None;
    }

    // Prefer definitions from other files over the current file.
    let other_file: Vec<&SymbolDef> = defs
        .iter()
        .filter(|d| !paths_match(&d.file, source_file))
        .collect();
    let pool = if !other_file.is_empty() {
        other_file
    } else {
        defs.iter().collect()
    };

    let mut seen = std::collections::HashSet::new();
    let mut deduped: Vec<&SymbolDef> = pool
        .iter()
        .filter(|d| seen.insert((&d.file, d.line, d.col)))
        .copied()
        .collect();

    if deduped.is_empty() {
        return None;
    }

    deduped.sort_by(|a, b| {
        rank_definition(word, a, source_file)
            .cmp(&rank_definition(word, b, source_file))
            .then_with(|| a.file.cmp(&b.file))
            .then_with(|| a.line.cmp(&b.line))
            .then_with(|| a.col.cmp(&b.col))
    });

    let best = deduped.first().copied()?;
    let best_rank = rank_definition(word, best, source_file);
    let has_tie = deduped
        .get(1)
        .is_some_and(|second| rank_definition(word, second, source_file) == best_rank);
    if has_tie {
        debug!(
            "[goto-def] TIER-6 ambiguous for '{word}' (top rank tie), suppressing fallback hit"
        );
        return None;
    }

    debug!(
        "[goto-def] TIER-6 candidate for '{word}': {}:{}:{} kind={}",
        best.file, best.line, best.col, best.kind
    );
    def_to_location(best).map(GotoDefinitionResponse::Scalar)
}

fn rank_definition(word: &str, def: &SymbolDef, source_file: &str) -> (u8, u8, u8, u8, u8) {
    let same_file = if paths_match(&def.file, source_file) { 0 } else { 1 };
    let is_definition = if def.is_definition { 0 } else { 1 };
    let is_parm_var = if matches!(def.kind.as_str(), "ParmVarDecl") {
        1
    } else {
        0
    };

    let system_rank = if looks_like_builtin_symbol(word) {
        if is_system_header(&def.file) { 0 } else { 1 }
    } else if is_system_header(&def.file) {
        1
    } else {
        0
    };

    let helper_penalty = if looks_like_builtin_symbol(word) && looks_like_project_helper_header(&def.file)
    {
        1
    } else {
        0
    };

    (
        same_file,
        is_definition,
        is_parm_var,
        system_rank,
        helper_penalty,
    )
}

fn disambiguate_member_field_tie<'a>(
    index: &'a AstIndex,
    tied_candidates: &[&'a SymbolDef],
    source_file: &str,
    source: &str,
    position: Position,
    word: &str,
) -> Option<&'a SymbolDef> {
    let receiver = extract_member_receiver_identifier(source, position, word)?;
    let cursor_line = position.line + 1;
    let cursor_col = position.character + 1;
    let receiver_type =
        infer_local_identifier_type_name(index, source_file, cursor_line, cursor_col, &receiver)?;
    let receiver_type = short_type_name(&receiver_type);

    let matches: Vec<&SymbolDef> = tied_candidates
        .iter()
        .copied()
        .filter(|candidate| candidate.kind == "FieldDecl")
        .filter(|candidate| {
            enclosing_record_name_for_field(index, candidate)
                .is_some_and(|owner_name| short_type_name(owner_name) == receiver_type)
        })
        .collect();

    if matches.len() == 1 {
        return matches.first().copied();
    }

    None
}

fn infer_local_identifier_type_name(
    index: &AstIndex,
    source_file: &str,
    cursor_line: u32,
    cursor_col: u32,
    identifier: &str,
) -> Option<String> {
    let indices = index.name_to_defs.get(identifier)?;

    let mut candidates: Vec<&SymbolDef> = indices
        .iter()
        .map(|&idx| &index.defs[idx])
        .filter(|def| paths_match(&def.file, source_file))
        .filter(|def| matches!(def.kind.as_str(), "ParmVarDecl" | "VarDecl" | "FieldDecl"))
        .filter(|def| def.line < cursor_line || (def.line == cursor_line && def.col <= cursor_col))
        .collect();

    if candidates.is_empty() {
        return None;
    }

    candidates.sort_by(|a, b| {
        b.line
            .cmp(&a.line)
            .then_with(|| b.col.cmp(&a.col))
            .then_with(|| local_value_kind_rank(&a.kind).cmp(&local_value_kind_rank(&b.kind)))
    });

    candidates.first().and_then(|def| def.type_name.clone())
}

fn local_value_kind_rank(kind: &str) -> u8 {
    match kind {
        "ParmVarDecl" => 0,
        "VarDecl" => 1,
        "FieldDecl" => 2,
        _ => 3,
    }
}

fn enclosing_record_name_for_field<'a>(index: &'a AstIndex, field: &SymbolDef) -> Option<&'a str> {
    if field.kind != "FieldDecl" {
        return None;
    }

    index
        .defs
        .iter()
        .filter(|def| paths_match(&def.file, &field.file))
        .filter(|def| matches!(def.kind.as_str(), "CXXRecordDecl" | "ClassTemplateSpecializationDecl"))
        .filter(|def| def.line <= field.line)
        .max_by_key(|def| def.line)
        .map(|def| def.name.as_str())
}

fn short_type_name(type_name: &str) -> &str {
    let without_namespace = type_name.rsplit("::").next().unwrap_or(type_name);
    without_namespace
        .split('<')
        .next()
        .unwrap_or(without_namespace)
}

fn extract_member_receiver_identifier(source: &str, position: Position, word: &str) -> Option<String> {
    let line = source.lines().nth(position.line as usize)?;
    let chars: Vec<char> = line.chars().collect();
    if chars.is_empty() {
        return None;
    }

    let mut cursor = position.character as usize;
    if cursor >= chars.len() {
        cursor = chars.len().saturating_sub(1);
    }

    let mut word_start = cursor;
    while word_start > 0 && is_ident_char(chars[word_start - 1]) {
        word_start -= 1;
    }
    let mut word_end = cursor;
    while word_end < chars.len() && is_ident_char(chars[word_end]) {
        word_end += 1;
    }

    let token: String = chars[word_start..word_end].iter().collect();
    if token != word {
        return None;
    }

    let mut idx = word_start;
    while idx > 0 && chars[idx - 1].is_whitespace() {
        idx -= 1;
    }
    if idx == 0 {
        return None;
    }

    let operator_start = if chars[idx - 1] == '.' {
        idx - 1
    } else if idx >= 2 && chars[idx - 1] == '>' && chars[idx - 2] == '-' {
        idx - 2
    } else {
        return None;
    };

    let mut base_end = operator_start;
    while base_end > 0 && chars[base_end - 1].is_whitespace() {
        base_end -= 1;
    }
    if base_end == 0 {
        return None;
    }

    let mut base_start = base_end;
    while base_start > 0 && is_ident_char(chars[base_start - 1]) {
        base_start -= 1;
    }
    if base_start == base_end {
        return None;
    }

    Some(chars[base_start..base_end].iter().collect())
}

fn is_ident_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn looks_like_builtin_symbol(word: &str) -> bool {
    word.starts_with("simd_") || word.starts_with("metal::")
}

fn is_non_navigable_symbol(word: &str) -> bool {
    matches!(
        word,
        "static_cast" | "dynamic_cast" | "reinterpret_cast" | "const_cast"
    )
}

fn looks_like_project_helper_header(path: &str) -> bool {
    path.ends_with("bf16_math.h") || path.contains("/common/")
}

fn resolve_macro_definition(
    uri: &Url,
    source: &str,
    word: &str,
) -> Option<GotoDefinitionResponse> {
    let pattern = format!("#define {word}");
    for (line_idx, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with(&pattern) {
            let col = line.find(&pattern).unwrap_or(0) + "#define ".len();
            let range = Range::new(
                Position::new(line_idx as u32, col as u32),
                Position::new(line_idx as u32, (col + word.len()) as u32),
            );
            return Some(GotoDefinitionResponse::Scalar(Location {
                uri: uri.clone(),
                range,
            }));
        }
    }
    None
}

fn content_hash(source: &str) -> String {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut h = DefaultHasher::new();
    source.hash(&mut h);
    format!("{:x}", h.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    static AST_DUMP_COUNTER_TEST_LOCK: once_cell::sync::Lazy<std::sync::Mutex<()>> =
        once_cell::sync::Lazy::new(|| std::sync::Mutex::new(()));

    fn has_metal_compiler() -> bool {
        std::process::Command::new("xcrun")
            .args(["--find", "metal"])
            .output()
            .is_ok_and(|o| o.status.success())
    }

    fn position_of(source: &str, needle: &str) -> Position {
        let idx = source.find(needle).expect("needle must exist");
        let before = &source[..idx];
        let line = before.as_bytes().iter().filter(|&&b| b == b'\n').count() as u32;
        let col = before
            .rsplit_once('\n')
            .map(|(_, tail)| tail.chars().count() as u32)
            .unwrap_or_else(|| before.chars().count() as u32);
        Position::new(line, col)
    }

    #[test]
    fn local_template_parameter_fast_path_resolves_usage() {
        let source = r#"
template <typename T, const int BN, const int TM>
struct Kernel {
  int value = BN * TM;
};
"#;
        let snapshot = SyntaxTree::parse(source);
        let uri = Url::parse("file:///tmp/kernel.metal").expect("valid uri");
        let usage = position_of(source, "BN * TM");

        let result =
            resolve_local_template_parameter(&uri, &snapshot, source, usage, "BN")
                .expect("template param should resolve");
        let GotoDefinitionResponse::Scalar(location) = result else {
            panic!("expected scalar response");
        };

        let definition_line = source
            .lines()
            .nth(location.range.start.line as usize)
            .expect("definition line");
        assert!(
            definition_line.contains("const int BN"),
            "expected BN template parameter definition, got line: {definition_line}"
        );
    }

    #[test]
    fn by_name_field_tie_uses_member_receiver_type_to_disambiguate() {
        let source = r#"
static METAL_FUNC void run(constant GEMMParams* params) {
  int gemm_k_iterations = params->gemm_k_iterations_aligned;
}
"#;
        let position = position_of(source, "gemm_k_iterations_aligned");
        let source_file = "/tmp/gemm.h";
        let params_file = "/tmp/params.h";

        let defs = vec![
            SymbolDef {
                id: "record-gemm".into(),
                name: "GEMMParams".into(),
                kind: "CXXRecordDecl".into(),
                file: params_file.into(),
                line: 10,
                col: 8,
                is_definition: true,
                type_name: None,
                qual_type: None,
            },
            SymbolDef {
                id: "field-gemm".into(),
                name: "gemm_k_iterations_aligned".into(),
                kind: "FieldDecl".into(),
                file: params_file.into(),
                line: 28,
                col: 13,
                is_definition: true,
                type_name: Some("int".into()),
                qual_type: Some("const int".into()),
            },
            SymbolDef {
                id: "record-splitk".into(),
                name: "GEMMSpiltKParams".into(),
                kind: "CXXRecordDecl".into(),
                file: params_file.into(),
                line: 33,
                col: 8,
                is_definition: true,
                type_name: None,
                qual_type: None,
            },
            SymbolDef {
                id: "field-splitk".into(),
                name: "gemm_k_iterations_aligned".into(),
                kind: "FieldDecl".into(),
                file: params_file.into(),
                line: 49,
                col: 13,
                is_definition: true,
                type_name: Some("int".into()),
                qual_type: Some("const int".into()),
            },
            SymbolDef {
                id: "parm-params".into(),
                name: "params".into(),
                kind: "ParmVarDecl".into(),
                file: source_file.into(),
                line: 2,
                col: 49,
                is_definition: true,
                type_name: Some("GEMMParams".into()),
                qual_type: Some("constant GEMMParams *".into()),
            },
        ];

        let mut name_to_defs = std::collections::HashMap::new();
        name_to_defs.insert("gemm_k_iterations_aligned".to_string(), vec![1, 3]);
        name_to_defs.insert("params".to_string(), vec![4]);

        let index = AstIndex {
            defs,
            refs: Vec::new(),
            id_to_def: std::collections::HashMap::new(),
            name_to_defs,
            target_id_to_refs: std::collections::HashMap::new(),
            file_to_defs: std::collections::HashMap::new(),
            file_to_refs: std::collections::HashMap::new(),
        };

        let result = resolve_by_name(
            &index,
            source_file,
            source,
            position,
            "gemm_k_iterations_aligned",
        )
        .expect("tie should be disambiguated to GEMMParams field");
        let GotoDefinitionResponse::Scalar(location) = result else {
            panic!("expected scalar response");
        };

        assert!(
            location.uri.path().ends_with("/tmp/params.h"),
            "expected params.h target, got {}",
            location.uri.path()
        );
        assert_eq!(location.range.start.line, 27);
    }

    #[test]
    fn by_name_field_tie_without_receiver_type_remains_ambiguous() {
        let source = "int v = ptr->gemm_k_iterations_aligned;";
        let position = position_of(source, "gemm_k_iterations_aligned");
        let source_file = "/tmp/gemm.h";
        let params_file = "/tmp/params.h";

        let defs = vec![
            SymbolDef {
                id: "record-gemm".into(),
                name: "GEMMParams".into(),
                kind: "CXXRecordDecl".into(),
                file: params_file.into(),
                line: 10,
                col: 8,
                is_definition: true,
                type_name: None,
                qual_type: None,
            },
            SymbolDef {
                id: "field-gemm".into(),
                name: "gemm_k_iterations_aligned".into(),
                kind: "FieldDecl".into(),
                file: params_file.into(),
                line: 28,
                col: 13,
                is_definition: true,
                type_name: Some("int".into()),
                qual_type: Some("const int".into()),
            },
            SymbolDef {
                id: "record-splitk".into(),
                name: "GEMMSpiltKParams".into(),
                kind: "CXXRecordDecl".into(),
                file: params_file.into(),
                line: 33,
                col: 8,
                is_definition: true,
                type_name: None,
                qual_type: None,
            },
            SymbolDef {
                id: "field-splitk".into(),
                name: "gemm_k_iterations_aligned".into(),
                kind: "FieldDecl".into(),
                file: params_file.into(),
                line: 49,
                col: 13,
                is_definition: true,
                type_name: Some("int".into()),
                qual_type: Some("const int".into()),
            },
        ];

        let mut name_to_defs = std::collections::HashMap::new();
        name_to_defs.insert("gemm_k_iterations_aligned".to_string(), vec![1, 3]);

        let index = AstIndex {
            defs,
            refs: Vec::new(),
            id_to_def: std::collections::HashMap::new(),
            name_to_defs,
            target_id_to_refs: std::collections::HashMap::new(),
            file_to_defs: std::collections::HashMap::new(),
            file_to_refs: std::collections::HashMap::new(),
        };

        let result = resolve_by_name(
            &index,
            source_file,
            source,
            position,
            "gemm_k_iterations_aligned",
        );
        assert!(
            result.is_none(),
            "without receiver type info, tie should remain ambiguous"
        );
    }

    #[test]
    fn cast_operators_are_non_navigable_symbols() {
        assert!(is_non_navigable_symbol("static_cast"));
        assert!(is_non_navigable_symbol("dynamic_cast"));
        assert!(is_non_navigable_symbol("reinterpret_cast"));
        assert!(is_non_navigable_symbol("const_cast"));
        assert!(!is_non_navigable_symbol("AccumType"));
    }

    #[tokio::test]
    async fn provide_returns_none_for_static_cast_keyword() {
        let source = r#"
kernel void k(device float* sinks [[buffer(0)]], uint tid [[thread_position_in_grid]]) {
  float value = static_cast<float>(sinks[tid]);
}
"#;
        let uri = Url::parse("file:///tmp/static_cast_keyword.metal").expect("valid uri");
        let position = position_of(source, "static_cast<float>");
        let snapshot = SyntaxTree::parse(source);
        let provider = DefinitionProvider::new();

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            provider.provide(&uri, position, source, &Vec::new(), &snapshot),
        )
        .await
        .expect("static_cast lookup should not block");

        assert!(
            result.is_none(),
            "goto-definition on static_cast should return no symbol location"
        );
    }

    #[tokio::test]
    async fn provide_returns_none_for_control_flow_keyword_without_ast_dump() {
        let _guard = AST_DUMP_COUNTER_TEST_LOCK
            .lock()
            .expect("AST dump test lock should not be poisoned");

        let source = r#"
kernel void k(device float* sinks [[buffer(0)]], uint tid [[thread_position_in_grid]]) {
  if (tid > 0) {
    sinks[tid] = 0.0f;
  }
}
"#;
        let uri = Url::parse("file:///tmp/if_keyword.metal").expect("valid uri");
        let position = position_of(source, "if (tid > 0)");
        let snapshot = SyntaxTree::parse(source);
        let provider = DefinitionProvider::new();
        let before = super::super::compiler::ast_dump_counter();

        let result = provider
            .provide(&uri, position, source, &Vec::new(), &snapshot)
            .await;

        let after = super::super::compiler::ast_dump_counter();
        assert!(
            result.is_none(),
            "goto-definition on `if` should return no symbol location"
        );
        assert_eq!(
            after - before,
            0,
            "language keyword lookup should not trigger AST dump work"
        );
    }

    #[tokio::test]
    async fn concurrent_goto_definition_for_same_file_runs_single_ast_dump() {
        if !has_metal_compiler() {
            return;
        }
        let _guard = AST_DUMP_COUNTER_TEST_LOCK
            .lock()
            .expect("AST dump test lock should not be poisoned");

        let fixtures_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let source_path = fixtures_dir.join("functions.metal");
        let uri = Url::from_file_path(&source_path).expect("fixture URI");
        let unique_suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("valid clock")
            .as_nanos();
        let source = format!(
            "{}\n// concurrent-goto-def-test-{unique_suffix}\n",
            std::fs::read_to_string(&source_path).expect("fixture source")
        );
        let snapshot = Arc::new(SyntaxTree::parse(&source));
        let include_paths = Arc::new(vec![fixtures_dir.display().to_string()]);
        let source = Arc::new(source);
        let provider = Arc::new(DefinitionProvider::new());

        let cursor = position_of(source.as_str(), "transform(data[id].position");
        let workers = 8usize;
        let barrier = Arc::new(tokio::sync::Barrier::new(workers));
        let before = super::super::compiler::ast_dump_counter();

        let responses = futures::future::join_all((0..workers).map(|_| {
            let provider = Arc::clone(&provider);
            let uri = uri.clone();
            let source = Arc::clone(&source);
            let include_paths = Arc::clone(&include_paths);
            let snapshot = Arc::clone(&snapshot);
            let barrier = Arc::clone(&barrier);
            async move {
                barrier.wait().await;
                provider
                    .provide(
                        &uri,
                        cursor,
                        source.as_str(),
                        include_paths.as_slice(),
                        snapshot.as_ref(),
                    )
                    .await
            }
        }))
        .await;

        for response in responses {
            assert!(
                response.is_some(),
                "all rapid navigation requests should resolve definition"
            );
        }

        let after = super::super::compiler::ast_dump_counter();
        assert_eq!(
            after - before,
            1,
            "concurrent jumps on the same document should share one AST dump build"
        );
    }

    #[tokio::test]
    async fn concurrent_index_document_and_provide_share_single_ast_dump() {
        if !has_metal_compiler() {
            return;
        }
        let _guard = AST_DUMP_COUNTER_TEST_LOCK
            .lock()
            .expect("AST dump test lock should not be poisoned");

        let fixtures_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let source_path = fixtures_dir.join("functions.metal");
        let uri = Url::from_file_path(&source_path).expect("fixture URI");
        let unique_suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("valid clock")
            .as_nanos();
        let source = format!(
            "{}\n// concurrent-index-and-provide-test-{unique_suffix}\n",
            std::fs::read_to_string(&source_path).expect("fixture source")
        );
        let snapshot = Arc::new(SyntaxTree::parse(&source));
        let include_paths = Arc::new(vec![fixtures_dir.display().to_string()]);
        let source = Arc::new(source);
        let provider = Arc::new(DefinitionProvider::new());
        let cursor = position_of(source.as_str(), "transform(data[id].position");
        let barrier = Arc::new(tokio::sync::Barrier::new(2));
        let before = super::super::compiler::ast_dump_counter();

        let index_task = {
            let provider = Arc::clone(&provider);
            let uri = uri.clone();
            let source = Arc::clone(&source);
            let include_paths = Arc::clone(&include_paths);
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                barrier.wait().await;
                provider
                    .index_document(&uri, source.as_str(), include_paths.as_slice())
                    .await;
            })
        };

        let provide_task = {
            let provider = Arc::clone(&provider);
            let uri = uri.clone();
            let source = Arc::clone(&source);
            let include_paths = Arc::clone(&include_paths);
            let snapshot = Arc::clone(&snapshot);
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                barrier.wait().await;
                provider
                    .provide(
                        &uri,
                        cursor,
                        source.as_str(),
                        include_paths.as_slice(),
                        snapshot.as_ref(),
                    )
                    .await
            })
        };

        let _ = index_task.await.expect("index task should not panic");
        let provide_result = provide_task.await.expect("provide task should not panic");
        assert!(
            provide_result.is_some(),
            "navigation request should still resolve while concurrent indexing runs"
        );

        let after = super::super::compiler::ast_dump_counter();
        assert_eq!(
            after - before,
            1,
            "index_document and provide should share one AST dump build for same source hash"
        );
    }

    #[test]
    fn matches_position_accepts_cursor_at_token_end_boundary() {
        assert!(matches_position(
            "/tmp/gemm_attention.metal",
            71,
            20,
            10,
            "/tmp/gemm_attention.metal",
            71,
            30,
        ));
    }

    #[test]
    fn matches_position_rejects_cursor_past_token_end_boundary() {
        assert!(!matches_position(
            "/tmp/gemm_attention.metal",
            71,
            20,
            10,
            "/tmp/gemm_attention.metal",
            71,
            31,
        ));
    }

}
