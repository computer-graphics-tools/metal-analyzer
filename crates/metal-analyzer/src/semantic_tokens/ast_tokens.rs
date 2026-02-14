use crate::{
    definition::AstIndex,
    semantic_tokens::{RawToken, mapping::map_ast_kind_to_token_type},
};

pub(crate) fn tokens_from_ast_index(
    index: &AstIndex,
    path: &str,
) -> Vec<RawToken> {
    let mut tokens = Vec::new();

    for def in &index.defs {
        if def.file == path
            && let Some(token_type) = map_ast_kind_to_token_type(&def.kind)
        {
            tokens.push(RawToken {
                line: def.line.saturating_sub(1),
                col: def.col.saturating_sub(1),
                length: def.name.len() as u32,
                token_type,
            });
        }
    }

    for r in &index.refs {
        if r.file == path
            && let Some(token_type) = map_ast_kind_to_token_type(&r.target_kind)
        {
            tokens.push(RawToken {
                line: r.line.saturating_sub(1),
                col: r.col.saturating_sub(1),
                length: r.tok_len,
                token_type,
            });
        }
    }

    tokens
}
