use std::collections::HashMap;

use tracing::debug;

use crate::definition::{
    ast_index::AstIndex,
    clang_nodes::{Clang, DeclData, Node, RefExprData, resolve_loc},
    ref_site::{RefSite, RefSiteLocation},
    symbol_def::SymbolDef,
    utils::normalize_type_name,
};

/// Collect a declaration node into the definitions list.
fn collect_decl(
    node: &Node,
    data: &DeclData,
    kind: &str,
    defs: &mut Vec<SymbolDef>,
) {
    let name = match data.name() {
        Some(n) if !n.is_empty() => n,
        _ => return,
    };

    if data.is_implicit() {
        return;
    }

    // For declarations, prefer spelling location so macro-generated declarations
    // jump to the declaration text in the macro body instead of call-site lines.
    let bare = match data
        .loc
        .as_ref()
        .and_then(|loc| loc.spelling_loc.as_ref().or(loc.expansion_loc.as_ref()))
        .or_else(|| data.loc.as_ref().and_then(resolve_loc))
    {
        Some(bare) if bare.line > 0 => bare,
        _ => return,
    };

    let qual_type = data.qual_type().map(str::to_owned);
    let type_name = if matches!(kind, "VarDecl" | "FieldDecl" | "ParmVarDecl") {
        data.qual_type().and_then(normalize_type_name)
    } else {
        None
    };

    defs.push(SymbolDef {
        id: node.id.to_string(),
        name: name.to_owned(),
        kind: kind.to_owned(),
        file: bare.file.to_string(),
        line: bare.line as u32,
        col: bare.col as u32,
        is_definition: data.is_definition(),
        type_name,
        qual_type,
    });
}

/// Collect a reference expression (DeclRefExpr, MemberExpr).
fn collect_ref(
    _node: &Node,
    data: &RefExprData,
    refs: &mut Vec<RefSite>,
) {
    if data.is_implicit.unwrap_or(false) {
        return;
    }

    let referenced = match &data.referenced_decl {
        Some(r) => r,
        None => return,
    };

    // Prefer range.begin for precise token location, fall back to loc.
    let source_loc = data.range.as_ref().map(|r| &r.begin).or(data.loc.as_ref());
    let source_loc = match source_loc {
        Some(loc) => loc,
        None => return,
    };

    let bare = match resolve_loc(source_loc) {
        Some(b) if b.line > 0 && !b.file.is_empty() => b,
        _ => return,
    };

    let to_ref_loc = |loc: &clang_ast::BareSourceLocation| -> Option<RefSiteLocation> {
        if loc.line == 0 || loc.file.is_empty() {
            return None;
        }
        Some(RefSiteLocation {
            file: loc.file.to_string(),
            line: loc.line as u32,
            col: loc.col as u32,
            tok_len: loc.tok_len as u32,
        })
    };

    let expansion = source_loc.expansion_loc.as_ref().and_then(to_ref_loc);
    let spelling = source_loc.spelling_loc.as_ref().and_then(to_ref_loc);

    refs.push(RefSite {
        file: bare.file.to_string(),
        line: bare.line as u32,
        col: bare.col as u32,
        tok_len: bare.tok_len as u32,
        target_id: referenced.id.to_string(),
        target_name: referenced.name.clone().unwrap_or_default(),
        target_kind: referenced.kind.clone().unwrap_or_default(),
        expansion,
        spelling,
    });
}

/// Recursively walk the typed AST, collecting declarations and references.
fn walk(
    node: &Node,
    defs: &mut Vec<SymbolDef>,
    refs: &mut Vec<RefSite>,
) {
    match &node.kind {
        Clang::FunctionDecl(d) => collect_decl(node, d, "FunctionDecl", defs),
        Clang::CXXRecordDecl(d) => collect_decl(node, d, "CXXRecordDecl", defs),
        Clang::CXXMethodDecl(d) => collect_decl(node, d, "CXXMethodDecl", defs),
        Clang::TypedefDecl(d) => collect_decl(node, d, "TypedefDecl", defs),
        Clang::TypeAliasDecl(d) => collect_decl(node, d, "TypeAliasDecl", defs),
        Clang::EnumDecl(d) => collect_decl(node, d, "EnumDecl", defs),
        Clang::EnumConstantDecl(d) => collect_decl(node, d, "EnumConstantDecl", defs),
        Clang::NamespaceDecl(d) => collect_decl(node, d, "NamespaceDecl", defs),
        Clang::FunctionTemplateDecl(d) => collect_decl(node, d, "FunctionTemplateDecl", defs),
        Clang::ClassTemplateDecl(d) => collect_decl(node, d, "ClassTemplateDecl", defs),
        Clang::ClassTemplateSpecializationDecl(d) => {
            collect_decl(node, d, "ClassTemplateSpecializationDecl", defs);
        },
        Clang::UsingDecl(d) => collect_decl(node, d, "UsingDecl", defs),
        Clang::TemplateTypeParmDecl(d) => collect_decl(node, d, "TemplateTypeParmDecl", defs),
        Clang::NonTypeTemplateParmDecl(d) => {
            collect_decl(node, d, "NonTypeTemplateParmDecl", defs);
        },
        Clang::VarDecl(d) => collect_decl(node, d, "VarDecl", defs),
        Clang::FieldDecl(d) => collect_decl(node, d, "FieldDecl", defs),
        Clang::ParmVarDecl(d) => collect_decl(node, d, "ParmVarDecl", defs),

        Clang::DeclRefExpr(d) => collect_ref(node, d, refs),
        Clang::MemberExpr(d) => collect_ref(node, d, refs),

        Clang::Other {
            ..
        } => {},
    }

    for child in &node.inner {
        walk(child, defs, refs);
    }
}

/// Build an [`AstIndex`] from a deserialized Clang AST root node.
///
/// `tmp_files` are the possible paths of the temp file that was compiled.
/// `original_file` is the real document path — any definition whose file
/// matches one of `tmp_files` will be rewritten to `original_file`.
pub(crate) fn build_index(
    root: &Node,
    tmp_files: &[String],
    original_file: Option<&str>,
) -> AstIndex {
    let mut defs = Vec::new();
    let mut refs = Vec::new();
    walk(root, &mut defs, &mut refs);

    debug!("[build-index] collected {} defs, {} refs (original_file={:?})", defs.len(), refs.len(), original_file,);

    if let Some(orig) = original_file {
        for def in &mut defs {
            if tmp_files.iter().any(|tmp| paths_equivalent(&def.file, tmp)) {
                def.file = orig.to_owned();
            }
        }
        for r in &mut refs {
            if tmp_files.iter().any(|tmp| paths_equivalent(&r.file, tmp)) {
                r.file = orig.to_owned();
            }
            if let Some(loc) = r.expansion.as_mut()
                && tmp_files.iter().any(|tmp| paths_equivalent(&loc.file, tmp))
            {
                loc.file = orig.to_owned();
            }
            if let Some(loc) = r.spelling.as_mut()
                && tmp_files.iter().any(|tmp| paths_equivalent(&loc.file, tmp))
            {
                loc.file = orig.to_owned();
            }
        }
    }

    let mut id_to_def = HashMap::with_capacity(defs.len());
    let mut name_to_defs: HashMap<String, Vec<usize>> = HashMap::with_capacity(defs.len());
    let mut target_id_to_refs: HashMap<String, Vec<usize>> = HashMap::new();
    let mut file_to_defs: HashMap<String, Vec<usize>> = HashMap::new();
    let mut file_to_refs: HashMap<String, Vec<usize>> = HashMap::new();

    for (i, def) in defs.iter().enumerate() {
        id_to_def
            .entry(def.id.clone())
            .and_modify(|existing_idx: &mut usize| {
                let existing_def = &defs[*existing_idx];
                if def.is_definition && !existing_def.is_definition {
                    *existing_idx = i;
                }
            })
            .or_insert(i);
        name_to_defs.entry(def.name.clone()).or_default().push(i);
        file_to_defs.entry(def.file.clone()).or_default().push(i);
    }

    for (i, ref_site) in refs.iter().enumerate() {
        target_id_to_refs.entry(ref_site.target_id.clone()).or_default().push(i);
        file_to_refs.entry(ref_site.file.clone()).or_default().push(i);
    }

    AstIndex {
        defs,
        refs,
        id_to_def,
        name_to_defs,
        target_id_to_refs,
        file_to_defs,
        file_to_refs,
    }
}

/// Check if two file paths refer to the same file.
///
/// Handles the common case where the AST dump reports a canonicalized path
/// while the temp file list has the original path (or vice versa).
fn paths_equivalent(
    a: &str,
    b: &str,
) -> bool {
    if a == b {
        return true;
    }
    let pa = std::path::Path::new(a);
    let pb = std::path::Path::new(b);
    if let (Ok(ca), Ok(cb)) = (pa.canonicalize(), pb.canonicalize()) {
        return ca == cb;
    }
    // Last resort: compare file names only.
    matches!((pa.file_name(), pb.file_name()), (Some(fa), Some(fb)) if fa == fb)
}
