mod common;

use std::sync::Arc;

use common::{fixture_path, fixture_uri, has_metal_compiler, include_paths_for, position_of, read_fixture};
use metal_analyzer::{DefinitionProvider, HoverProvider, SymbolProvider, syntax::SyntaxTree};
use tower_lsp::lsp_types::{HoverContents, MarkedString};

fn marked_string_text(marked: &MarkedString) -> String {
    match marked {
        MarkedString::String(s) => s.clone(),
        MarkedString::LanguageString(s) => s.value.clone(),
    }
}

fn hover_text(contents: &HoverContents) -> String {
    match contents {
        HoverContents::Markup(markup) => markup.value.clone(),
        HoverContents::Scalar(s) => marked_string_text(s),
        HoverContents::Array(items) => items.iter().map(marked_string_text).collect::<Vec<_>>().join("\n"),
    }
}

#[test]
fn references_include_cross_file_uses_for_shared_symbol() {
    if !has_metal_compiler() {
        return;
    }

    let provider = DefinitionProvider::new();
    let file_a = "matmul/gemv/shaders/ref_user_a.metal";
    let file_b = "matmul/gemv/shaders/ref_user_b.metal";

    assert!(
        provider.index_workspace_file(&fixture_path(file_a), &include_paths_for(file_a)),
        "expected indexing success for {file_a}"
    );
    assert!(
        provider.index_workspace_file(&fixture_path(file_b), &include_paths_for(file_b)),
        "expected indexing success for {file_b}"
    );

    let source = read_fixture(file_a);
    let uri = fixture_uri(file_a);
    let include_paths = include_paths_for(file_a);
    let snapshot = SyntaxTree::parse(&source);
    let mut position = position_of(&source, "fixture::shared_mul");
    position.character += "fixture::".len() as u32;

    let refs = provider
        .provide_references(&uri, position, &source, &include_paths, &snapshot, true)
        .expect("expected references for shared_mul");
    let paths: Vec<String> = refs.iter().map(|loc| loc.file_path.to_string_lossy().to_string()).collect();

    assert!(
        paths.iter().any(|p| p.ends_with("/common/math_ops.h")),
        "expected declaration in common/math_ops.h, got {paths:?}"
    );
    assert!(
        paths.iter().any(|p| p.ends_with("/matmul/gemv/shaders/ref_user_a.metal")),
        "expected same-file reference for ref_user_a, got {paths:?}"
    );
    assert!(
        paths.iter().any(|p| p.ends_with("/matmul/gemv/shaders/ref_user_b.metal")),
        "expected cross-file reference for ref_user_b, got {paths:?}"
    );
}

#[test]
fn prepare_rename_allows_project_symbol() {
    if !has_metal_compiler() {
        return;
    }

    let provider = DefinitionProvider::new();
    let rel = "matmul/gemv/shaders/ref_user_a.metal";
    let source = read_fixture(rel);
    let uri = fixture_uri(rel);
    let snapshot = SyntaxTree::parse(&source);
    let include_paths = include_paths_for(rel);
    provider.index_document(&uri, &source, &include_paths);

    let mut position = position_of(&source, "fixture::shared_mul");
    position.character += "fixture::".len() as u32;
    let rename_range = provider.prepare_rename(&uri, position, &source, &include_paths, &snapshot);

    assert!(rename_range.is_some(), "expected rename range for shared_mul");
}

#[tokio::test]
async fn hover_handles_attribute_in_realistic_fixture() {
    let definition_provider = Arc::new(DefinitionProvider::new());
    let symbol_provider = Arc::new(SymbolProvider::new());
    let hover_provider = HoverProvider::new(symbol_provider, definition_provider);

    let rel = "matmul/gemv/shaders/gemv_like.metal";
    let source = read_fixture(rel);
    let uri = fixture_uri(rel);
    let snapshot = SyntaxTree::parse(&source);

    let position = position_of(&source, "buffer(0)");
    let hover = hover_provider
        .provide(&uri, &source, position, Some(&snapshot))
        .await
        .expect("expected hover for buffer attribute");
    let text = hover_text(&hover.contents);

    assert!(text.contains("buffer"), "hover should include buffer attribute help, got: {text}");
}

#[test]
fn symbol_extraction_keeps_kernel_and_template_symbols() {
    let source = read_fixture("matmul/gemv/shaders/gemv_like.metal");
    let provider = SymbolProvider::new();
    let symbols = provider.extract_symbols(&source);
    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();

    assert!(names.contains(&"gemv_like"), "expected kernel symbol gemv_like, got: {names:?}");
    assert!(
        names.contains(&"MTL_CONST"),
        "expected macro symbol MTL_CONST in realistic shader fixture, got: {names:?}"
    );
}
