use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use tower_lsp::lsp_types::Position;

use crate::{
    definition::symbol_text::{extract_namespace_qualifier_before_word, is_ident_char},
    ide::{
        lsp::lsp_position_to_ide,
        navigation::{IdeLocation, IdePosition, IdeRange, NavigationTarget},
    },
    text_pos::position_from_byte_offset,
};

pub(super) fn resolve_fast_system_symbol_location(
    source: &str,
    position: Position,
    word: &str,
    include_paths: &[String],
) -> Option<NavigationTarget> {
    if let Some(qualifier) = extract_namespace_qualifier_before_word(source, position, word)
        && is_likely_system_namespace(&qualifier)
        && let Some(result) = resolve_qualified_system_symbol_location(include_paths, &qualifier, word)
    {
        return Some(result);
    }

    if !should_fast_lookup_system_symbol(source, position, word) {
        return None;
    }

    resolve_system_header_symbol_location(word, include_paths)
}

pub(super) fn should_fast_lookup_system_symbol(
    source: &str,
    position: Position,
    word: &str,
) -> bool {
    if is_likely_system_symbol_family(word) {
        return true;
    }

    if let Some(qualifier) = extract_namespace_qualifier_before_word(source, position, word) {
        return is_likely_system_namespace(&qualifier);
    }

    false
}

fn is_likely_system_symbol_family(word: &str) -> bool {
    if matches!(
        word,
        "mem_flags"
            | "thread_scope"
            | "memory_order"
            | "memory_scope"
            | "threadgroup_barrier"
            | "simdgroup_barrier"
            | "simd_sum"
    ) {
        return true;
    }

    ["simd_", "simdgroup_", "threadgroup_", "quad_", "atomic_", "mem_", "thread_", "intersection_", "visible_"]
        .iter()
        .any(|prefix| word.starts_with(prefix))
}

fn is_likely_system_namespace(qualifier: &str) -> bool {
    matches!(
        qualifier,
        "metal"
            | "address"
            | "coord"
            | "filter"
            | "mip_filter"
            | "compare_func"
            | "access"
            | "mem_flags"
            | "thread_scope"
            | "memory_order"
            | "memory_scope"
    )
}

pub(super) fn system_builtin_header_candidates(include_paths: &[String]) -> Vec<PathBuf> {
    const METAL_HEADER_BASENAMES: &[&str] = &[
        "metal_stdlib",
        "metal_compute",
        "metal_simdgroup",
        "metal_atomic",
        "metal_math",
        "metal_geometric",
        "metal_types",
        "metal_common",
    ];

    let mut seen = HashSet::new();
    let mut out = Vec::new();

    for include_path in include_paths {
        let include_root = PathBuf::from(include_path);
        let roots = [include_root.clone(), include_root.join("metal")];
        for root in roots {
            for basename in METAL_HEADER_BASENAMES {
                let candidate = normalize_candidate_path(&root.join(basename));
                if candidate.is_file() && seen.insert(candidate.clone()) {
                    out.push(candidate);
                }
            }

            if let Ok(entries) = std::fs::read_dir(&root) {
                for entry in entries.flatten() {
                    let candidate = normalize_candidate_path(&entry.path());
                    if !candidate.is_file() {
                        continue;
                    }
                    let Some(file_name) = candidate.file_name().and_then(|name| name.to_str()) else {
                        continue;
                    };
                    if !file_name.starts_with("metal") {
                        continue;
                    }
                    if seen.insert(candidate.clone()) {
                        out.push(candidate);
                    }
                }
            }
        }
    }

    out
}

pub(super) fn resolve_system_header_symbol_location(
    symbol: &str,
    include_paths: &[String],
) -> Option<NavigationTarget> {
    for header_path in system_builtin_header_candidates(include_paths) {
        let Some(range) = find_word_range_in_file(&header_path, symbol) else {
            continue;
        };
        return Some(NavigationTarget::Single(IdeLocation::new(header_path, range)));
    }

    None
}

fn resolve_qualified_system_symbol_location(
    include_paths: &[String],
    qualifier: &str,
    symbol: &str,
) -> Option<NavigationTarget> {
    for header_path in system_builtin_header_candidates(include_paths) {
        let Some(range) = find_scoped_enum_member_range_in_file(&header_path, qualifier, symbol) else {
            continue;
        };
        return Some(NavigationTarget::Single(IdeLocation::new(header_path, range)));
    }

    None
}

fn normalize_candidate_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn find_word_range_in_file(
    path: &Path,
    word: &str,
) -> Option<IdeRange> {
    let source = std::fs::read_to_string(path).ok()?;
    let start = find_word_boundary_offset(&source, word)?;
    let start_pos = byte_offset_to_position(&source, start);
    let end_pos = byte_offset_to_position(&source, start + word.len());
    Some(IdeRange::new(start_pos, end_pos))
}

pub(super) fn find_word_boundary_offset(
    source: &str,
    word: &str,
) -> Option<usize> {
    if word.is_empty() {
        return None;
    }

    let mut search_from = 0usize;
    while let Some(local_idx) = source[search_from..].find(word) {
        let start = search_from + local_idx;
        let end = start + word.len();

        let prev = source[..start].chars().next_back();
        let next = source[end..].chars().next();
        let prev_is_ident = prev.is_some_and(is_ident_char);
        let next_is_ident = next.is_some_and(is_ident_char);
        if !prev_is_ident && !next_is_ident {
            return Some(start);
        }

        search_from = end;
    }

    None
}

fn find_scoped_enum_member_range_in_file(
    path: &Path,
    qualifier: &str,
    symbol: &str,
) -> Option<IdeRange> {
    let source = std::fs::read_to_string(path).ok()?;
    let start = find_scoped_enum_member_offset(&source, qualifier, symbol)?;
    let start_pos = byte_offset_to_position(&source, start);
    let end_pos = byte_offset_to_position(&source, start + symbol.len());
    Some(IdeRange::new(start_pos, end_pos))
}

pub(super) fn find_scoped_enum_member_offset(
    source: &str,
    qualifier: &str,
    symbol: &str,
) -> Option<usize> {
    if qualifier.is_empty() || symbol.is_empty() {
        return None;
    }

    let enum_markers = [format!("enum class {qualifier}"), format!("enum {qualifier}")];
    for marker in enum_markers {
        let mut search_from = 0usize;
        while let Some(local_marker_start) = source[search_from..].find(&marker) {
            let marker_start = search_from + local_marker_start;
            let after_marker = &source[marker_start + marker.len()..];
            let Some(open_brace_rel) = after_marker.find('{') else {
                search_from = marker_start + marker.len();
                continue;
            };
            let body_start = marker_start + marker.len() + open_brace_rel + 1;
            let Some(body_end) = find_matching_brace(source, body_start - 1) else {
                search_from = body_start;
                continue;
            };
            let body = &source[body_start..body_end];
            if let Some(body_offset) = find_word_boundary_offset(body, symbol) {
                return Some(body_start + body_offset);
            }

            search_from = body_end + 1;
        }
    }

    None
}

fn find_matching_brace(
    source: &str,
    open_brace_offset: usize,
) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, ch) in source[open_brace_offset..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(open_brace_offset + idx);
                }
            },
            _ => {},
        }
    }
    None
}

fn byte_offset_to_position(
    source: &str,
    byte_offset: usize,
) -> IdePosition {
    lsp_position_to_ide(position_from_byte_offset(source, byte_offset))
}
