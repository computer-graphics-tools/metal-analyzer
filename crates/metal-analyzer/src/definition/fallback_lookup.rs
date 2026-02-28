use std::collections::HashSet;

use tower_lsp::lsp_types::Position;
use tracing::debug;

use crate::{
    definition::{
        ast_index::AstIndex,
        project_graph::ProjectGraph,
        project_index::ProjectIndex,
        ref_site::RefSite,
        symbol_def::SymbolDef,
        symbol_rank::{disambiguate_member_tie, rank_definition},
        utils::{def_to_location, paths_match},
    },
    ide::navigation::{IdeLocation, IdePosition, IdeRange, NavigationTarget},
    vfs::FileId,
};

pub(super) fn resolve_by_name(
    index: &AstIndex,
    source_file: &str,
    source: &str,
    position: Position,
    word: &str,
) -> Option<NavigationTarget> {
    let indices = index.name_to_defs.get(word)?;

    let all_defs: Vec<&SymbolDef> =
        indices.iter().map(|&i| &index.defs[i]).filter(|d| !d.file.is_empty() && d.line > 0).collect();

    if all_defs.is_empty() {
        return None;
    }

    let mut seen = HashSet::new();
    let mut deduped: Vec<&SymbolDef> =
        all_defs.iter().filter(|d| seen.insert((&d.file, d.line, d.col))).copied().collect();

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
    let has_tie = deduped.get(1).is_some_and(|second| rank_definition(word, second, source_file) == best_rank);
    if has_tie {
        let tied: Vec<&SymbolDef> = deduped
            .iter()
            .copied()
            .take_while(|candidate| rank_definition(word, candidate, source_file) == best_rank)
            .collect();

        if let Some(disambiguated) = disambiguate_member_tie(index, &tied, source_file, source, position, word) {
            debug!(
                "[goto-def] TIER-5 disambiguated member tie '{word}' to {}:{}:{}",
                disambiguated.file, disambiguated.line, disambiguated.col
            );
            return def_to_location(disambiguated).map(NavigationTarget::Single);
        }

        if let Some(disambiguated) = disambiguate_parameter_tie(&tied, source_file, position) {
            debug!(
                "[goto-def] TIER-5 disambiguated parameter tie '{word}' to {}:{}:{}",
                disambiguated.file, disambiguated.line, disambiguated.col
            );
            return def_to_location(disambiguated).map(NavigationTarget::Single);
        }

        debug!("[goto-def] TIER-5 ambiguous for '{word}' (top rank tie), suppressing fallback hit");
        return None;
    }

    debug!("[goto-def] TIER-5 candidate for '{word}': {}:{}:{} kind={}", best.file, best.line, best.col, best.kind);
    def_to_location(best).map(NavigationTarget::Single)
}

pub(super) fn ref_site_to_location(ref_site: &RefSite) -> Option<IdeLocation> {
    let (file, line, col, tok_len) = if let Some(loc) = ref_site.expansion.as_ref() {
        (&loc.file, loc.line, loc.col, loc.tok_len)
    } else {
        (&ref_site.file, ref_site.line, ref_site.col, ref_site.tok_len)
    };
    if file.is_empty() {
        return None;
    }
    Some(IdeLocation::new(
        file,
        IdeRange::new(
            IdePosition::new(line.saturating_sub(1), col.saturating_sub(1)),
            IdePosition::new(line.saturating_sub(1), col.saturating_sub(1) + tok_len),
        ),
    ))
}

fn disambiguate_parameter_tie<'a>(
    tied: &[&'a SymbolDef],
    source_file: &str,
    position: Position,
) -> Option<&'a SymbolDef> {
    let cursor_line = position.line + 1;

    let same_file_params: Vec<&'a SymbolDef> = tied
        .iter()
        .copied()
        .filter(|d| paths_match(&d.file, source_file))
        .filter(|d| matches!(d.kind.as_str(), "ParmVarDecl" | "TemplateTypeParmDecl" | "NonTypeTemplateParmDecl"))
        .collect();

    if same_file_params.is_empty() {
        return None;
    }

    let before_cursor: Vec<&SymbolDef> = same_file_params.iter().copied().filter(|d| d.line <= cursor_line).collect();

    let pool = if !before_cursor.is_empty() {
        &before_cursor
    } else {
        &same_file_params
    };

    pool.iter().copied().max_by_key(|d| d.line)
}

pub(super) fn resolve_from_project_index(
    project_index: &ProjectIndex,
    project_graph: &ProjectGraph,
    source_file: &str,
    source_file_id: Option<&FileId>,
    project_graph_depth: usize,
    project_graph_max_nodes: usize,
    word: &str,
    position: Position,
) -> Option<NavigationTarget> {
    let defs = source_file_id
        .map(|file_id| project_graph.scoped_files(file_id, project_graph_depth, project_graph_max_nodes))
        .and_then(|scope| {
            let scoped_defs = project_index.find_definitions_in_files(word, &scope);
            if scoped_defs.is_empty() {
                None
            } else {
                debug!(
                    "[goto-def] TIER-6 graph-scoped candidates for '{word}': {} files, {} defs",
                    scope.len(),
                    scoped_defs.len()
                );
                Some(scoped_defs)
            }
        })
        .unwrap_or_else(|| project_index.find_definitions(word));
    if defs.is_empty() {
        return None;
    }

    let other_file: Vec<&SymbolDef> = defs.iter().filter(|d| !paths_match(&d.file, source_file)).collect();
    let pool = if !other_file.is_empty() {
        other_file
    } else {
        defs.iter().collect()
    };

    let mut seen = HashSet::new();
    let mut deduped: Vec<&SymbolDef> = pool.iter().filter(|d| seen.insert((&d.file, d.line, d.col))).copied().collect();

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
    let has_tie = deduped.get(1).is_some_and(|second| rank_definition(word, second, source_file) == best_rank);
    if has_tie {
        let tied: Vec<&SymbolDef> = deduped
            .iter()
            .copied()
            .take_while(|candidate| rank_definition(word, candidate, source_file) == best_rank)
            .collect();

        if let Some(disambiguated) = disambiguate_parameter_tie(&tied, source_file, position) {
            debug!(
                "[goto-def] TIER-6 disambiguated parameter tie '{word}' to {}:{}:{}",
                disambiguated.file, disambiguated.line, disambiguated.col
            );
            return def_to_location(disambiguated).map(NavigationTarget::Single);
        }

        debug!("[goto-def] TIER-6 ambiguous for '{word}' (top rank tie), suppressing fallback hit");
        return None;
    }

    debug!("[goto-def] TIER-6 candidate for '{word}': {}:{}:{} kind={}", best.file, best.line, best.col, best.kind);
    def_to_location(best).map(NavigationTarget::Single)
}
