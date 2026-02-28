use tower_lsp::lsp_types::Position;

use crate::text_pos::line_and_byte_column_at_position;

pub(super) fn extract_call_argument_count(
    source: &str,
    position: Position,
    word: &str,
) -> Option<usize> {
    let (chars, cursor) = line_chars_and_cursor(source, position)?;

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

    let mut idx = word_end;
    while idx < chars.len() && chars[idx].is_whitespace() {
        idx += 1;
    }
    if idx >= chars.len() || chars[idx] != '(' {
        return None;
    }

    let mut depth = 0usize;
    let mut saw_any_argument_token = false;
    let mut commas = 0usize;
    for ch in chars[idx..].iter().copied() {
        match ch {
            '(' => depth += 1,
            ')' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
                if depth == 0 {
                    return if saw_any_argument_token {
                        Some(commas + 1)
                    } else {
                        Some(0)
                    };
                }
            },
            ',' if depth == 1 => commas += 1,
            c if depth == 1 && !c.is_whitespace() => {
                saw_any_argument_token = true;
            },
            _ => {},
        }
    }

    None
}

pub(super) fn extract_member_receiver_identifier(
    source: &str,
    position: Position,
    word: &str,
) -> Option<String> {
    let (chars, cursor) = line_chars_and_cursor(source, position)?;

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

pub(super) fn extract_namespace_qualifier_before_word(
    source: &str,
    position: Position,
    word: &str,
) -> Option<String> {
    let (chars, cursor) = line_chars_and_cursor(source, position)?;

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
    if idx < 2 || chars[idx - 1] != ':' || chars[idx - 2] != ':' {
        return None;
    }

    let mut qualifier_end = idx - 2;
    while qualifier_end > 0 && chars[qualifier_end - 1].is_whitespace() {
        qualifier_end -= 1;
    }
    if qualifier_end == 0 {
        return None;
    }

    let mut qualifier_start = qualifier_end;
    while qualifier_start > 0 && is_ident_char(chars[qualifier_start - 1]) {
        qualifier_start -= 1;
    }
    if qualifier_start == qualifier_end {
        return None;
    }

    Some(chars[qualifier_start..qualifier_end].iter().collect())
}

pub(super) fn is_ident_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

pub(super) fn line_chars_and_cursor(
    source: &str,
    position: Position,
) -> Option<(Vec<char>, usize)> {
    let (line, byte_col) = line_and_byte_column_at_position(source, position)?;
    let chars: Vec<char> = line.chars().collect();
    if chars.is_empty() {
        return None;
    }

    let mut cursor = line[..byte_col].chars().count();
    if cursor >= chars.len() {
        cursor = chars.len().saturating_sub(1);
    }

    Some((chars, cursor))
}
