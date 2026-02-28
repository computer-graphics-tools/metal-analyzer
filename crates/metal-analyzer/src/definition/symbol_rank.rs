use std::collections::HashSet;

use tower_lsp::lsp_types::Position;

use crate::{
    definition::{
        ast_index::AstIndex,
        symbol_def::SymbolDef,
        symbol_text::{extract_call_argument_count, extract_member_receiver_identifier},
        utils::{is_system_header, paths_match},
    },
    metal::builtins::lookup as lookup_builtin,
};

pub(super) fn rank_definition(
    word: &str,
    def: &SymbolDef,
    source_file: &str,
) -> (u8, u8, u8, u8) {
    let same_file = if paths_match(&def.file, source_file) {
        0
    } else {
        1
    };
    let is_definition = if def.is_definition {
        0
    } else {
        1
    };
    let is_parm_var = if matches!(def.kind.as_str(), "ParmVarDecl") {
        1
    } else {
        0
    };

    let system_rank = if looks_like_builtin_symbol(word) {
        if is_system_header(&def.file) {
            0
        } else {
            1
        }
    } else if is_system_header(&def.file) {
        1
    } else {
        0
    };

    (same_file, is_definition, is_parm_var, system_rank)
}

pub(super) fn disambiguate_member_tie<'a>(
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
    let receiver_type = infer_local_identifier_type_name(index, source_file, cursor_line, cursor_col, &receiver)
        .map(|type_name| short_type_name(&type_name).to_string());

    let mut matches: Vec<&SymbolDef> = tied_candidates.iter().copied().filter(is_member_candidate).collect();
    if matches.is_empty() {
        return None;
    }

    if let Some(receiver_type) = receiver_type.as_deref() {
        let owner_matched: Vec<&SymbolDef> = matches
            .iter()
            .copied()
            .filter(|candidate| {
                enclosing_record_name_for_member(index, candidate)
                    .is_some_and(|owner_name| short_type_name(owner_name) == receiver_type)
            })
            .collect();
        if !owner_matched.is_empty() {
            matches = owner_matched;
        }
    } else if matches.iter().all(|candidate| candidate.kind == "CXXMethodDecl") {
        let owner_names: HashSet<&str> = matches
            .iter()
            .filter_map(|candidate| enclosing_record_name_for_member(index, candidate))
            .map(short_type_name)
            .collect();
        if owner_names.len() > 1 {
            return None;
        }
    } else {
        return None;
    }

    if matches.len() == 1 {
        return matches.first().copied();
    }

    if matches.iter().all(|candidate| candidate.kind == "CXXMethodDecl") {
        return select_method_overload_for_member_call(matches, source, position, word);
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

fn is_member_candidate(def: &&SymbolDef) -> bool {
    matches!(def.kind.as_str(), "FieldDecl" | "CXXMethodDecl")
}

fn enclosing_record_name_for_member<'a>(
    index: &'a AstIndex,
    member: &SymbolDef,
) -> Option<&'a str> {
    if !matches!(member.kind.as_str(), "FieldDecl" | "CXXMethodDecl") {
        return None;
    }

    index
        .defs
        .iter()
        .filter(|def| paths_match(&def.file, &member.file))
        .filter(|def| matches!(def.kind.as_str(), "CXXRecordDecl" | "ClassTemplateSpecializationDecl"))
        .filter(|def| def.line <= member.line)
        .max_by_key(|def| def.line)
        .map(|def| def.name.as_str())
}

fn select_method_overload_for_member_call<'a>(
    mut methods: Vec<&'a SymbolDef>,
    source: &str,
    position: Position,
    word: &str,
) -> Option<&'a SymbolDef> {
    methods.retain(|candidate| candidate.kind == "CXXMethodDecl");
    if methods.is_empty() {
        return None;
    }

    if let Some(argument_count) = extract_call_argument_count(source, position, word) {
        let arity_matched: Vec<&SymbolDef> = methods
            .iter()
            .copied()
            .filter(|candidate| {
                method_parameter_count(candidate).map(|param_count| param_count == argument_count).unwrap_or(true)
            })
            .collect();
        if !arity_matched.is_empty() {
            methods = arity_matched;
        }
    }

    methods.sort_by(|a, b| {
        method_constness_rank(a)
            .cmp(&method_constness_rank(b))
            .then_with(|| a.file.cmp(&b.file))
            .then_with(|| a.line.cmp(&b.line))
            .then_with(|| a.col.cmp(&b.col))
    });
    methods.first().copied()
}

fn method_constness_rank(def: &SymbolDef) -> u8 {
    if def.kind != "CXXMethodDecl" {
        return 2;
    }

    let signature = def.qual_type.as_deref().unwrap_or_default();
    let normalized = signature.trim_end();
    if normalized.ends_with("const") || normalized.contains(") const") || normalized.contains(" const noexcept") {
        1
    } else {
        0
    }
}

fn method_parameter_count(def: &SymbolDef) -> Option<usize> {
    let signature = def.qual_type.as_deref()?;
    let start = signature.find('(')?;
    let mut depth = 0usize;
    let mut end = None;
    for (idx, ch) in signature[start..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    end = Some(start + idx);
                    break;
                }
            },
            _ => {},
        }
    }
    let end = end?;
    let params = signature[start + 1..end].trim();
    if params.is_empty() || params == "void" {
        return Some(0);
    }

    let mut count = 1usize;
    let mut nested = 0usize;
    for ch in params.chars() {
        match ch {
            '<' | '(' | '[' => nested += 1,
            '>' | ')' | ']' => nested = nested.saturating_sub(1),
            ',' if nested == 0 => count += 1,
            _ => {},
        }
    }
    Some(count)
}

fn short_type_name(type_name: &str) -> &str {
    let without_namespace = type_name.rsplit("::").next().unwrap_or(type_name);
    without_namespace.split('<').next().unwrap_or(without_namespace)
}

fn looks_like_builtin_symbol(word: &str) -> bool {
    word.starts_with("simd_") || word.starts_with("metal::") || lookup_builtin(word).is_some()
}
