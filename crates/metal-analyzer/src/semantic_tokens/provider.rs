use tower_lsp::lsp_types::{SemanticToken, Url};

use crate::{
    definition::DefinitionProvider,
    semantic_tokens::{
        RawToken, ast_tokens::tokens_from_ast_index, encode_delta, merge_tokens, syntactic::syntactic_tokens,
    },
    syntax::SyntaxTree,
};

/// Semantic token provider with two tiers: instant rowan syntactic tokens
/// and deferred Clang AST semantic tokens that overlay for higher precision.
pub struct SemanticTokenProvider {
    definition_provider: std::sync::Arc<DefinitionProvider>,
}

impl SemanticTokenProvider {
    pub fn new(definition_provider: std::sync::Arc<DefinitionProvider>) -> Self {
        Self {
            definition_provider,
        }
    }

    /// Provide merged tokens: rowan syntactic base + Clang AST overlay.
    pub fn provide(
        &self,
        uri: &Url,
        snapshot: Option<&SyntaxTree>,
    ) -> Vec<SemanticToken> {
        let mut raw_tokens: Vec<RawToken> = match snapshot {
            Some(snapshot) => syntactic_tokens(snapshot),
            None => Vec::new(),
        };

        if let Some(index) = self.definition_provider.get_cached_index(uri) {
            let path = uri.to_file_path().ok().map(|p| p.display().to_string()).unwrap_or_default();
            let ast_tokens = tokens_from_ast_index(&index, &path);
            merge_tokens(&mut raw_tokens, &ast_tokens);
        }

        encode_delta(raw_tokens)
    }
}
