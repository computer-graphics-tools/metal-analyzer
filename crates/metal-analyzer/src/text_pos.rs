use tower_lsp::lsp_types::Position;

#[allow(dead_code)]
pub fn byte_offset_from_position(
    source: &str,
    position: Position,
) -> Option<usize> {
    let line_idx = position.line as usize;
    let mut lines = source.split('\n');
    let mut byte_offset = 0usize;

    for _ in 0..line_idx {
        let line = lines.next()?;
        byte_offset += line.len() + 1;
    }

    let line = lines.next()?;
    let mut utf16_offset = 0u32;
    let mut char_offset = 0usize;
    for ch in line.chars() {
        if utf16_offset >= position.character {
            break;
        }
        utf16_offset += ch.len_utf16() as u32;
        char_offset += ch.len_utf8();
    }

    Some(byte_offset + char_offset)
}

#[allow(dead_code)]
pub fn position_from_byte_offset(
    source: &str,
    byte_offset: usize,
) -> Position {
    let mut remaining = byte_offset.min(source.len());

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
            return Position::new(line_index as u32, utf16_col);
        }
        remaining = remaining.saturating_sub(line_len + 1);
    }

    Position::new(0, 0)
}

#[allow(dead_code)]
pub fn line_and_byte_column_at_position(
    source: &str,
    position: Position,
) -> Option<(&str, usize)> {
    let byte_offset = byte_offset_from_position(source, position)?;
    let line_start = source[..byte_offset].rfind('\n').map(|idx| idx + 1).unwrap_or(0);
    let line_end = source[byte_offset..].find('\n').map(|idx| byte_offset + idx).unwrap_or(source.len());
    Some((&source[line_start..line_end], byte_offset - line_start))
}

#[allow(dead_code)]
pub fn utf16_column_of_byte_offset(
    line: &str,
    byte_offset: usize,
) -> u32 {
    line[..byte_offset.min(line.len())].encode_utf16().count() as u32
}
