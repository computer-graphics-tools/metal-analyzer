use metal_analyzer::HoverProvider;
use metal_analyzer::DefinitionProvider;
use metal_analyzer::symbols::SymbolProvider;
use std::sync::Arc;
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Url};

fn marked_string_text(ms: &MarkedString) -> String {
    match ms {
        MarkedString::String(s) => s.clone(),
        MarkedString::LanguageString(ls) => ls.value.clone(),
    }
}

fn hover_text(hover: &HoverContents) -> String {
    match hover {
        HoverContents::Markup(markup) => markup.value.clone(),
        HoverContents::Scalar(s) => marked_string_text(s),
        HoverContents::Array(items) => items
            .iter()
            .map(marked_string_text)
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

fn test_provider() -> HoverProvider {
    HoverProvider::new(
        Arc::new(SymbolProvider::new()),
        Arc::new(DefinitionProvider::new()),
    )
}

fn test_uri() -> Url {
    Url::parse("file:///tmp/test.metal").unwrap()
}

// ──────────────────────────── tests ─────────────────────────────────────────

#[tokio::test]
async fn hover_known_symbol_returns_markup() {
    let provider = test_provider();
    let text = "float4 x;";
    let hover = provider
        .provide(
            &test_uri(),
            text,
            Position {
                line: 0,
                character: 2,
            },
            None,
        )
        .await;

    let contents = hover_text(&hover.expect("expected hover").contents);
    // The hover text formatting includes code blocks, so we search for "float4" inside a block
    assert!(contents.contains("float4") || contents.contains("Vector type"));
}

#[tokio::test]
async fn hover_keyword_returns_markup() {
    let provider = test_provider();
    let text = "kernel void f() {}";
    let hover = provider
        .provide(
            &test_uri(),
            text,
            Position {
                line: 0,
                character: 2,
            },
            None,
        )
        .await;

    let contents = hover_text(&hover.expect("expected hover").contents);
    assert!(contents.contains("Metal keyword"));
}

#[tokio::test]
async fn hover_unknown_symbol_returns_none() {
    let provider = test_provider();
    let text = "myCustomType x;";
    let hover = provider
        .provide(
            &test_uri(),
            text,
            Position {
                line: 0,
                character: 5,
            },
            None,
        )
        .await;

    assert!(hover.is_none());
}

#[tokio::test]
async fn hover_attribute_normalizes_parameter() {
    let provider = test_provider();
    let text = "constant float x [[buffer(2)]];";
    let hover = provider
        .provide(
            &test_uri(),
            text,
            Position {
                line: 0,
                character: 26,
            },
            None,
        )
        .await;

    let contents = hover_text(&hover.expect("expected hover").contents);
    assert!(contents.contains("buffer"));
}
