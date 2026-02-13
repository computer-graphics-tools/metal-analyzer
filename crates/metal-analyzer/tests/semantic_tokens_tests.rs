use std::sync::Arc;

use metal_analyzer::{
    DefinitionProvider,
    semantic_tokens::{LEGEND_TYPES, SemanticTokenProvider, get_legend},
    syntax::SyntaxTree,
};
use tower_lsp::lsp_types::{SemanticTokenType, Url};

#[test]
fn legend_matches_declared_types() {
    let legend = get_legend();
    assert_eq!(legend.token_types.len(), LEGEND_TYPES.len());
    assert!(LEGEND_TYPES.contains(&SemanticTokenType::KEYWORD));
    assert!(LEGEND_TYPES.contains(&SemanticTokenType::TYPE));
}

#[test]
fn provider_returns_tokens_for_basic_source() {
    let source = "struct S { int x; }; kernel void f() { return; }";
    let tree = SyntaxTree::parse(source);
    let uri = Url::parse("file:///tmp/test.metal").expect("valid uri");

    let provider = SemanticTokenProvider::new(Arc::new(DefinitionProvider::new()));
    let tokens = provider.provide(&uri, Some(&tree));

    assert!(!tokens.is_empty(), "expected semantic tokens for basic source");
}
