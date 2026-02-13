use tower_lsp::lsp_types::{Position, Url};
use tracing::debug;

use crate::{
    definition::{
        ast_index::AstIndex,
        ref_site::RefSite,
        symbol_def::SymbolDef,
        utils::{def_to_location, paths_match},
    },
    ide::{
        lsp::lsp_range_to_ide,
        navigation::{IdeLocation, NavigationTarget},
    },
    syntax::{
        SyntaxTree,
        ast::{self, AstNode},
        cst::SyntaxNode,
        helpers,
        kind::SyntaxKind,
    },
};

pub(super) fn resolve_local_template_parameter(
    uri: &Url,
    snapshot: &SyntaxTree,
    source: &str,
    position: Position,
    word: &str,
) -> Option<NavigationTarget> {
    let root = snapshot.root();
    let node = helpers::node_at_position(&root, source, position)?;
    let file_path = uri.to_file_path().ok()?;

    for declaration_node in node.ancestors().filter(|ancestor| {
        matches!(ancestor.kind(), SyntaxKind::FunctionDef | SyntaxKind::StructDef | SyntaxKind::ClassDef)
    }) {
        let Some(template_def) = nearest_preceding_template_def(&root, &declaration_node) else {
            continue;
        };
        let Some(template_def) = ast::TemplateDef::cast(template_def) else {
            continue;
        };

        let mut matches =
            template_def.parameters().filter_map(|param| param.name_token()).filter(|name| name.text() == word);
        let first = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        let range = lsp_range_to_ide(helpers::range_to_lsp(first.text_range(), source));
        return Some(NavigationTarget::Single(IdeLocation::new(file_path.clone(), range)));
    }

    None
}

fn nearest_preceding_template_def(
    root: &SyntaxNode,
    declaration_node: &SyntaxNode,
) -> Option<SyntaxNode> {
    let declaration_start = declaration_node.text_range().start();

    root.descendants()
        .filter(|node| node.kind() == SyntaxKind::TemplateDef)
        .filter(|node| node.text_range().end() <= declaration_start)
        .max_by_key(|node| node.text_range().end())
}

pub(super) fn resolve_precise(
    index: &AstIndex,
    source_file: &str,
    position: Position,
    word: &str,
) -> Option<NavigationTarget> {
    resolve_precise_def(index, source_file, position, word)
        .and_then(|def| def_to_location(def).map(NavigationTarget::Single))
}

pub(super) fn resolve_precise_def<'a>(
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
            if !matches!(matched_site, MatchSite::Primary) && matches!(def.kind.as_str(), "ParmVarDecl") {
                continue;
            }
            debug!("Precise ({matched_site}): {} → {}:{}:{}", word, def.file, def.line, def.col);
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
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        let name = match self {
            Self::Primary => "primary",
            Self::Expansion => "expansion",
            Self::Spelling => "spelling",
        };
        f.write_str(name)
    }
}

pub(super) fn matches_position(
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
