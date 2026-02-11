use tower_lsp::lsp_types::{Hover, Position};

use crate::metal::builtins;
use crate::syntax::cst::SyntaxNode;
use crate::syntax::helpers;

use super::builtins::make_hover_from_entry;

pub(crate) fn try_attribute_hover_from_tree(
    root: &SyntaxNode,
    source: &str,
    position: Position,
) -> Option<Hover> {
    let attr_text = helpers::attribute_at_position(root, source, position)?;

    if let Some(entry) = builtins::lookup(&attr_text) {
        return Some(make_hover_from_entry(entry));
    }

    let normalized = normalize_attribute(&attr_text);
    if normalized != attr_text
        && let Some(entry) = builtins::lookup(&normalized)
    {
        return Some(make_hover_from_entry(entry));
    }

    None
}

pub(crate) fn try_attribute_hover(text: &str, position: Position) -> Option<Hover> {
    let lines: Vec<&str> = text.lines().collect();
    let line_idx = position.line as usize;
    if line_idx >= lines.len() {
        return None;
    }
    let line = lines[line_idx];
    let col = position.character as usize;
    let bytes = line.as_bytes();

    if bytes.is_empty() || col >= bytes.len() {
        return None;
    }

    // Walk backwards from the cursor to find `[[`.
    let mut start = None;
    for i in (0..=col.min(bytes.len().saturating_sub(1))).rev() {
        if i + 1 < bytes.len() && bytes[i] == b'[' && bytes[i + 1] == b'[' {
            start = Some(i);
            break;
        }
    }
    let start = start?;

    // Walk forward to find `]]`.
    let search_start = col.min(bytes.len().saturating_sub(1));
    let mut end = None;
    for i in search_start..bytes.len().saturating_sub(1) {
        if bytes[i] == b']' && bytes[i + 1] == b']' {
            end = Some(i + 2);
            break;
        }
    }
    let end = end?;

    let attr_text = &line[start..end];

    if let Some(entry) = builtins::lookup(attr_text) {
        return Some(make_hover_from_entry(entry));
    }

    let normalized = normalize_attribute(attr_text);
    if normalized != attr_text
        && let Some(entry) = builtins::lookup(&normalized)
    {
        return Some(make_hover_from_entry(entry));
    }

    None
}

/// Normalize a parameterized attribute for lookup.
///
/// `[[buffer(2)]]` → `[[buffer(n)]]`,  `[[color(0)]]` → `[[color(n)]]`.
pub(crate) fn normalize_attribute(attr: &str) -> String {
    if let Some(open) = attr.find('(')
        && let Some(rel_close) = attr[open..].find(')')
    {
        let close = open + rel_close;
        let mut normalized = String::with_capacity(attr.len());
        normalized.push_str(&attr[..open + 1]);
        normalized.push('n');
        normalized.push_str(&attr[close..]);
        return normalized;
    }
    attr.to_string()
}
