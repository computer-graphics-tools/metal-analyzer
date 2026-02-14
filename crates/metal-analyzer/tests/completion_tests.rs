use metal_analyzer::CompletionProvider;
use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, Position};

fn has_label(
    items: &[CompletionItem],
    label: &str,
) -> bool {
    items.iter().any(|item| item.label == label)
}

fn has_insert_text(
    items: &[CompletionItem],
    insert_text: &str,
) -> bool {
    items.iter().any(|item| item.insert_text.as_deref() == Some(insert_text))
}

#[test]
fn include_context_returns_metal_headers() {
    let provider = CompletionProvider::new();
    let text = "#include ";
    let items = provider.provide(
        Some(text),
        Position {
            line: 0,
            character: text.len() as u32,
        },
        None,
    );

    assert!(has_insert_text(&items, "<metal_stdlib>"), "expected metal_stdlib include suggestion");
}

#[test]
fn preprocessor_context_offers_directives() {
    let provider = CompletionProvider::new();
    let text = "#inc";
    let items = provider.provide(
        Some(text),
        Position {
            line: 0,
            character: text.len() as u32,
        },
        None,
    );

    assert!(has_label(&items, "include"), "expected preprocessor directive completion");
}

#[test]
fn member_access_context_offers_swizzles() {
    let provider = CompletionProvider::new();
    let text = "position.";
    let items = provider.provide(
        Some(text),
        Position {
            line: 0,
            character: text.len() as u32,
        },
        None,
    );

    let has_swizzle = items.iter().any(|item| item.label == "xyz" && item.kind == Some(CompletionItemKind::FIELD));

    assert!(has_swizzle, "expected vector swizzle completion");
}

#[test]
fn provide_with_none_text_returns_items() {
    let provider = CompletionProvider::new();
    let items = provider.provide(
        None,
        Position {
            line: 0,
            character: 0,
        },
        None,
    );

    assert!(!items.is_empty(), "expected non-empty completions");
}
