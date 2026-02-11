use std::path::Path;
use tower_lsp::lsp_types::{Location, Position, Range, Url};

use super::symbol_def::SymbolDef;

/// Normalize a Clang type name by stripping qualifiers and pointers.
///
/// E.g. `const float *` -> `float`, `device atomic_int &` -> `atomic_int`.
/// This is used to resolve `type_name` from a variable declaration to its
/// type definition.
pub fn normalize_type_name(qual_type: &str) -> Option<String> {
    let mut s = qual_type.trim();
    if s.is_empty() {
        return None;
    }

    // Drop template arguments to get the base type name.
    if let Some((base, _)) = s.split_once('<') {
        s = base.trim();
    }

    // Strip leading qualifiers / elaborations.
    loop {
        let before = s;
        for prefix in [
            "const ",
            "volatile ",
            "struct ",
            "class ",
            "enum ",
            "thread ",
            "device ",
            "threadgroup ",
            "constant ",
        ] {
            if let Some(rest) = before.strip_prefix(prefix) {
                s = rest.trim_start();
                break;
            }
        }
        if before == s {
            break;
        }
    }

    // Strip trailing pointer/reference markers.
    s = s.trim_end_matches(['*', '&', ' ', '\t']);

    // Take the last whitespace-delimited token.
    let token = s.split_whitespace().last().unwrap_or(s);
    let token = token.trim_end_matches(['*', '&']);

    // Drop namespaces.
    let token = token.rsplit("::").next().unwrap_or(token).trim();

    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

pub fn def_to_location(def: &SymbolDef) -> Option<Location> {
    let uri = Url::from_file_path(Path::new(&def.file)).ok()?;

    // AST locations are 1-based; LSP is 0-based.
    // Be careful with underflow if the AST reports 0 (which shouldn't happen for valid locs).
    let line = def.line.saturating_sub(1);
    let col = def.col.saturating_sub(1);
    let end_col = col + def.name.len() as u32;

    Some(Location {
        uri,
        range: Range {
            start: Position {
                line,
                character: col,
            },
            end: Position {
                line,
                character: end_col,
            },
        },
    })
}

/// Returns `true` if a file path looks like a system / SDK header.
pub fn is_system_header(path: &str) -> bool {
    path.contains("/Toolchains/")
        || path.contains("/SDKs/")
        || path.contains("/usr/include/")
        || path.contains("/lib/clang/")
        || path.contains("/metal/include/")
        || path.is_empty()
}

/// Compare two file paths for equality, tolerating symlinks and the temp-file
/// indirection we use for the AST dump.
pub fn paths_match(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    // Try canonical comparison (resolves symlinks).
    let pa = Path::new(a);
    let pb = Path::new(b);
    if let (Ok(ca), Ok(cb)) = (pa.canonicalize(), pb.canonicalize())
        && ca == cb {
            return true;
        }
    // Last resort: compare file names only (handles the temp file case).
    if let (Some(fa), Some(fb)) = (pa.file_name(), pb.file_name()) {
        return fa == fb;
    }
    false
}
