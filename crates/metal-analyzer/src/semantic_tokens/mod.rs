//! Semantic token provider with two tiers: instant rowan syntactic tokens
//! and deferred Clang AST semantic tokens that overlay for higher precision.

pub(crate) mod ast_tokens;
pub(crate) mod mapping;
pub(crate) mod provider;
pub(crate) mod syntactic;

use tower_lsp::lsp_types::{SemanticToken, SemanticTokenType, SemanticTokensLegend};

pub use self::provider::SemanticTokenProvider;

pub const LEGEND_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::NAMESPACE,
    SemanticTokenType::TYPE,
    SemanticTokenType::CLASS,
    SemanticTokenType::ENUM,
    SemanticTokenType::INTERFACE,
    SemanticTokenType::STRUCT,
    SemanticTokenType::TYPE_PARAMETER,
    SemanticTokenType::PARAMETER,
    SemanticTokenType::VARIABLE,
    SemanticTokenType::PROPERTY,
    SemanticTokenType::ENUM_MEMBER,
    SemanticTokenType::EVENT,
    SemanticTokenType::FUNCTION,
    SemanticTokenType::METHOD,
    SemanticTokenType::MACRO,
    SemanticTokenType::KEYWORD,
    SemanticTokenType::MODIFIER,
    SemanticTokenType::COMMENT,
    SemanticTokenType::STRING,
    SemanticTokenType::NUMBER,
    SemanticTokenType::REGEXP,
    SemanticTokenType::OPERATOR,
];

pub fn get_legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: LEGEND_TYPES.into(),
        token_modifiers: vec![],
    }
}

/// A raw token before delta encoding.
#[derive(Clone)]
pub(crate) struct RawToken {
    pub(crate) line: u32,
    pub(crate) col: u32,
    pub(crate) length: u32,
    pub(crate) token_type: SemanticTokenType,
}

/// Fast mapping from byte offsets to (line, column).
///
/// Note: columns are byte-based (sufficient for typical Metal code).
pub(crate) struct LineIndex {
    line_starts: Box<[usize]>,
}

impl LineIndex {
    pub(crate) fn new(source: &str) -> Self {
        let mut starts = Vec::with_capacity(source.len() / 40);
        starts.push(0usize);
        for (i, b) in source.bytes().enumerate() {
            if b == b'\n' {
                starts.push(i + 1);
            }
        }
        Self {
            line_starts: starts.into_boxed_slice(),
        }
    }

    pub(crate) fn line_col(
        &self,
        byte_offset: usize,
    ) -> (u32, u32) {
        let off = byte_offset;
        let line = match self.line_starts.binary_search(&off) {
            Ok(exact) => exact,
            Err(ins) => ins.saturating_sub(1),
        };
        let col = off.saturating_sub(self.line_starts[line]);
        (line as u32, col as u32)
    }
}

/// Merge AST tokens into syntactic tokens. AST tokens override syntactic
/// tokens at the same position (they have higher semantic precision).
pub(crate) fn merge_tokens(
    syntactic: &mut Vec<RawToken>,
    ast: &[RawToken],
) {
    for ast_tok in ast {
        syntactic.retain(|t| !(t.line == ast_tok.line && t.col == ast_tok.col));
        syntactic.push(ast_tok.clone());
    }
}

/// Sort tokens and encode as LSP delta format.
pub(crate) fn encode_delta(mut tokens: Vec<RawToken>) -> Vec<SemanticToken> {
    tokens.sort_by(|a, b| a.line.cmp(&b.line).then(a.col.cmp(&b.col)));

    // Deduplicate tokens at the same position (keep last = highest priority).
    tokens.dedup_by(|a, b| a.line == b.line && a.col == b.col);

    let mut result = Vec::with_capacity(tokens.len());
    let mut prev_line = 0u32;
    let mut prev_col = 0u32;

    for tok in tokens {
        let delta_line = tok.line - prev_line;
        let delta_col = if delta_line == 0 {
            tok.col - prev_col
        } else {
            tok.col
        };

        result.push(SemanticToken {
            delta_line,
            delta_start: delta_col,
            length: tok.length,
            token_type: mapping::get_token_type_index(tok.token_type),
            token_modifiers_bitset: 0,
        });

        prev_line = tok.line;
        prev_col = tok.col;
    }

    result
}
