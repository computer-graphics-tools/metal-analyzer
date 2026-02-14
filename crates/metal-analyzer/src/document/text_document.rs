use tower_lsp::lsp_types::*;

// ── Document ────────────────────────────────────────────────────────────────

/// Snapshot of a single open text document.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Document {
    /// The document URI.
    pub uri: Url,
    /// Full source text (always kept up-to-date).
    pub text: String,
    /// Document version as reported by the client.
    pub version: i32,
    /// Pre-computed line start byte offsets (rebuilt on every mutation).
    line_offsets: Vec<usize>,
}

impl Document {
    pub fn new(
        uri: Url,
        text: String,
        version: i32,
    ) -> Self {
        let line_offsets = Self::compute_line_offsets(&text);
        Self {
            uri,
            text,
            version,
            line_offsets,
        }
    }

    // ── queries ─────────────────────────────────────────────────────────

    /// Number of lines in the document.
    #[allow(dead_code)]
    pub fn line_count(&self) -> usize {
        self.line_offsets.len()
    }

    /// Return the full text of a given 0-based line (without the trailing newline).
    #[allow(dead_code)]
    pub fn line_text(
        &self,
        line: usize,
    ) -> Option<&str> {
        let start = *self.line_offsets.get(line)?;
        let end = self.line_offsets.get(line + 1).copied().unwrap_or(self.text.len());
        let slice = &self.text[start..end];
        Some(slice.trim_end_matches('\n').trim_end_matches('\r'))
    }

    /// Convert an LSP `Position` (line/character, 0-based) to a byte offset.
    pub fn offset_of(
        &self,
        pos: Position,
    ) -> Option<usize> {
        let line = pos.line as usize;
        let line_start = *self.line_offsets.get(line)?;
        let line_end = self.line_offsets.get(line + 1).copied().unwrap_or(self.text.len());
        let line_text = &self.text[line_start..line_end];

        // LSP character offsets are UTF-16 code-unit counts.
        let mut utf16_offset: u32 = 0;
        let mut byte_offset = line_start;
        for ch in line_text.chars() {
            if utf16_offset >= pos.character {
                break;
            }
            utf16_offset += ch.len_utf16() as u32;
            byte_offset += ch.len_utf8();
        }
        Some(byte_offset)
    }

    /// Convert a byte offset to an LSP `Position`.
    #[allow(dead_code)]
    pub fn position_of(
        &self,
        offset: usize,
    ) -> Position {
        let offset = offset.min(self.text.len());
        let line = match self.line_offsets.binary_search(&offset) {
            Ok(exact) => exact,
            Err(ins) => ins.saturating_sub(1),
        };
        let line_start = self.line_offsets[line];
        let character = self.text[line_start..offset].chars().map(|c| c.len_utf16() as u32).sum::<u32>();
        Position {
            line: line as u32,
            character,
        }
    }

    /// Extract the word (identifier) surrounding the given position.
    /// Returns `(word, Range)`.
    #[allow(dead_code)]
    pub fn word_at(
        &self,
        pos: Position,
    ) -> Option<(String, Range)> {
        let line_text = self.line_text(pos.line as usize)?;
        let chars: Vec<char> = line_text.chars().collect();

        // Translate UTF-16 character offset to char index in the line.
        let mut char_idx: usize = 0;
        let mut utf16_count: u32 = 0;
        for (i, &ch) in chars.iter().enumerate() {
            if utf16_count >= pos.character {
                char_idx = i;
                break;
            }
            utf16_count += ch.len_utf16() as u32;
            char_idx = i + 1;
        }

        if char_idx >= chars.len() {
            if char_idx > 0 && is_word_char(chars[char_idx - 1]) {
                char_idx = chars.len() - 1;
            } else {
                return None;
            }
        }

        if !is_word_char(chars[char_idx]) {
            if char_idx > 0 && is_word_char(chars[char_idx - 1]) {
                char_idx -= 1;
            } else {
                return None;
            }
        }

        // Expand left.
        let mut start = char_idx;
        while start > 0 && is_word_char(chars[start - 1]) {
            start -= 1;
        }

        // Expand right.
        let mut end = char_idx;
        while end + 1 < chars.len() && is_word_char(chars[end + 1]) {
            end += 1;
        }

        let word: String = chars[start..=end].iter().collect();
        if word.is_empty() {
            return None;
        }

        let start_utf16: u32 = chars[..start].iter().map(|c| c.len_utf16() as u32).sum();
        let end_utf16: u32 = chars[..=end].iter().map(|c| c.len_utf16() as u32).sum();

        let range = Range {
            start: Position {
                line: pos.line,
                character: start_utf16,
            },
            end: Position {
                line: pos.line,
                character: end_utf16,
            },
        };

        Some((word, range))
    }

    // ── mutations ───────────────────────────────────────────────────────

    /// Replace the full content and bump version.
    pub fn set_content(
        &mut self,
        text: String,
        version: i32,
    ) {
        self.text = text;
        self.version = version;
        self.line_offsets = Self::compute_line_offsets(&self.text);
    }

    /// Apply a list of incremental or full-content changes and bump version.
    pub fn apply_changes(
        &mut self,
        changes: Vec<TextDocumentContentChangeEvent>,
        version: i32,
    ) {
        for change in changes {
            if let Some(range) = change.range {
                if let (Some(start), Some(end)) = (self.offset_of(range.start), self.offset_of(range.end)) {
                    self.text.replace_range(start..end, &change.text);
                    self.line_offsets = Self::compute_line_offsets(&self.text);
                }
            } else {
                self.text = change.text;
                self.line_offsets = Self::compute_line_offsets(&self.text);
            }
        }
        self.version = version;
    }

    // ── internal helpers ────────────────────────────────────────────────

    fn compute_line_offsets(text: &str) -> Vec<usize> {
        let mut offsets = vec![0usize];
        for (i, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                offsets.push(i + 1);
            }
        }
        offsets
    }
}

// ── helpers ─────────────────────────────────────────────────────────────────

#[allow(dead_code)]
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

#[cfg(test)]
#[path = "../../tests/src/document/text_document_tests.rs"]
mod tests;
