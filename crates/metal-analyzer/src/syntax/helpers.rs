/// Coordinate conversion and node utilities for rowan and LSP interop.
use rowan::{TextRange, TextSize, TokenAtOffset};
use tower_lsp::lsp_types::{Position, Range};

use crate::syntax::cst::{SyntaxNode, SyntaxToken};
use crate::syntax::kind::SyntaxKind;

pub fn range_to_lsp(range: TextRange, source: &str) -> Range {
    Range {
        start: offset_to_position(source, range.start()),
        end: offset_to_position(source, range.end()),
    }
}

pub fn token_text<'a>(token: &SyntaxToken, source: &'a str) -> &'a str {
    let range = token.text_range();
    let start = range.start().into();
    let end = range.end().into();
    &source[start..end]
}

pub fn node_text<'a>(node: &SyntaxNode, source: &'a str) -> &'a str {
    let range = node.text_range();
    let start = range.start().into();
    let end = range.end().into();
    &source[start..end]
}

/// Find the deepest node at the given LSP position.
pub fn node_at_position(root: &SyntaxNode, source: &str, position: Position) -> Option<SyntaxNode> {
    let offset = position_to_offset(source, position);
    let token = pick_token(root.token_at_offset(offset))?;
    token.parent()
}

/// Walk ancestors until a node with the given kind is found.
pub fn find_ancestor(node: SyntaxNode, kind: SyntaxKind) -> Option<SyntaxNode> {
    let mut current = node;
    loop {
        if current.kind() == kind {
            return Some(current);
        }
        current = current.parent()?;
    }
}

/// Extract the identifier word at a position using the syntax tree.
pub fn word_at_position(root: &SyntaxNode, source: &str, position: Position) -> Option<String> {
    let offset = position_to_offset(source, position);
    let token = pick_token(root.token_at_offset(offset))?;
    if token.kind() == SyntaxKind::Ident {
        return Some(token_text(&token, source).to_string());
    }
    None
}

/// Return the syntax-token kind under the cursor, if any.
pub fn token_kind_at_position(
    root: &SyntaxNode,
    source: &str,
    position: Position,
) -> Option<SyntaxKind> {
    let offset = position_to_offset(source, position);
    let token = pick_token(root.token_at_offset(offset))?;
    Some(token.kind())
}

/// Extract a navigable symbol at a position with parser-aware fallback rules.
///
/// We only use text fallback when the parser cannot classify the token (`Error`)
/// or no token exists at this position. For pointer/reference declarators
/// (`Type*`, `Type&`, `Type&&`) we allow text fallback so the type name remains
/// jumpable when the cursor lands on the punctuation token.
pub fn navigation_word_at_position(
    root: &SyntaxNode,
    source: &str,
    position: Position,
) -> Option<String> {
    if let Some(word) = word_at_position(root, source, position) {
        return Some(word);
    }

    if let Some(kind) = token_kind_at_position(root, source, position)
        && !matches!(kind, SyntaxKind::Error)
        && !allows_navigation_text_fallback(kind)
    {
        return None;
    }

    word_at_position_text_fallback(source, position)
}

fn allows_navigation_text_fallback(kind: SyntaxKind) -> bool {
    matches!(kind, SyntaxKind::Star | SyntaxKind::Amp | SyntaxKind::AndAnd)
}

/// Extract the identifier word at a position using plain text scanning.
pub fn word_at_position_text_fallback(source: &str, position: Position) -> Option<String> {
    let line = source.lines().nth(position.line as usize)?;
    let chars: Vec<char> = line.chars().collect();
    let col = position.character as usize;

    if col > chars.len() {
        return None;
    }

    let is_identifier_char = |c: char| c.is_alphanumeric() || c == '_';

    let mut start = col;
    while start > 0 && is_identifier_char(chars[start - 1]) {
        start -= 1;
    }

    let mut end = col;
    while end < chars.len() && is_identifier_char(chars[end]) {
        end += 1;
    }

    if start == end {
        return None;
    }

    Some(chars[start..end].iter().collect())
}

/// Extract include path from a `#include` directive using the syntax tree.
/// Returns `(path, is_system)` where `is_system` is true for `<...>` includes.
pub fn include_at_position(
    root: &SyntaxNode,
    source: &str,
    position: Position,
) -> Option<(String, bool)> {
    let node = node_at_position(root, source, position)?;
    let include_node = find_ancestor(node, SyntaxKind::PreprocInclude)?;
    let include_text = node_text(&include_node, source);
    parse_include_text(include_text)
}

/// Extract include path from a `#include` directive using text scanning.
pub fn include_at_position_text_fallback(
    source: &str,
    position: Position,
) -> Option<(String, bool)> {
    let line = source.lines().nth(position.line as usize)?;
    let trimmed = line.trim_start();
    if !trimmed.starts_with("#include") {
        return None;
    }

    let start_index = line.find('<').or_else(|| line.find('"'))?;
    let is_system = line.as_bytes()[start_index] == b'<';
    let end_char = if is_system { '>' } else { '"' };
    let end_index = line[start_index + 1..].find(end_char)? + start_index + 1;

    let col = position.character as usize;
    if col < line.len() {
        return Some((line[start_index + 1..end_index].to_string(), is_system));
    }

    None
}

/// Detect a `[[...]]` attribute at the cursor position.
/// Returns the full attribute text (e.g. `[[position]]` or `[[buffer(0)]]`).
pub fn attribute_at_position(
    root: &SyntaxNode,
    source: &str,
    position: Position,
) -> Option<String> {
    let node = node_at_position(root, source, position)?;
    if let Some(attr_node) = find_ancestor(node, SyntaxKind::Attribute) {
        return Some(node_text(&attr_node, source).to_string());
    }
    attribute_at_position_text_fallback(source, position)
}

pub fn attribute_at_position_text_fallback(source: &str, position: Position) -> Option<String> {
    let line = source.lines().nth(position.line as usize)?;
    let start = line.find("[[")?;
    let end = line[start + 2..].find("]]")? + start + 2;
    let col = position.character as usize;
    if col >= start && col <= end + 2 {
        return Some(line[start..end + 2].to_string());
    }
    None
}

fn parse_include_text(text: &str) -> Option<(String, bool)> {
    if let Some(start) = text.find('<')
        && let Some(end) = text[start + 1..].find('>')
    {
        return Some((text[start + 1..start + 1 + end].to_string(), true));
    }
    if let Some(start) = text.find('"')
        && let Some(end) = text[start + 1..].find('"')
    {
        return Some((text[start + 1..start + 1 + end].to_string(), false));
    }
    None
}

fn pick_token(tokens: TokenAtOffset<SyntaxToken>) -> Option<SyntaxToken> {
    tokens.max_by_key(|token| match token.kind() {
        SyntaxKind::Ident => 2,
        SyntaxKind::Integer | SyntaxKind::Float | SyntaxKind::String => 1,
        _ => 0,
    })
}

fn position_to_offset(source: &str, position: Position) -> TextSize {
    let mut byte_offset = 0usize;
    let mut lines = source.split('\n');

    for _ in 0..position.line {
        if let Some(line) = lines.next() {
            byte_offset += line.len() + 1;
        } else {
            return TextSize::from(source.len() as u32);
        }
    }

    if let Some(line) = lines.next() {
        let mut utf16_col = 0u32;
        let mut char_offset = 0usize;
        for ch in line.chars() {
            if utf16_col >= position.character {
                break;
            }
            utf16_col += ch.len_utf16() as u32;
            char_offset += ch.len_utf8();
        }
        byte_offset += char_offset;
    }

    TextSize::from((byte_offset as u32).min(source.len() as u32))
}

fn offset_to_position(source: &str, offset: TextSize) -> Position {
    let mut remaining = offset.into();
    for (line_index, line) in source.split('\n').enumerate() {
        let line_len = line.len();
        if remaining <= line_len {
            let mut utf16_col = 0u32;
            let mut byte_count = 0usize;
            for ch in line.chars() {
                if byte_count >= remaining {
                    break;
                }
                utf16_col += ch.len_utf16() as u32;
                byte_count += ch.len_utf8();
            }
            return Position {
                line: line_index as u32,
                character: utf16_col,
            };
        }
        remaining = remaining.saturating_sub(line_len + 1);
    }
    Position {
        line: 0,
        character: 0,
    }
}

#[cfg(test)]
#[path = "../../tests/src/syntax/helpers_tests.rs"]
mod tests;
