mod common;

use std::sync::Arc;

use common::{fixture_uri, has_metal_compiler, include_paths_for, read_fixture};
use metal_analyzer::metal::compiler::MetalCompiler;
use metal_analyzer::syntax::SyntaxTree;
use metal_analyzer::{CompletionProvider, DefinitionProvider, SemanticTokenProvider};
use tower_lsp::lsp_types::{CompletionItemKind, DiagnosticSeverity, Position};

#[test]
fn completion_handles_realistic_include_and_member_contexts() {
    let provider = CompletionProvider::new();

    let include_text = "#include ";
    let include_items = provider.provide(
        Some(include_text),
        Position::new(0, include_text.len() as u32),
        None,
    );
    assert!(
        include_items
            .iter()
            .any(|item| item.insert_text.as_deref() == Some("<metal_stdlib>")),
        "expected include-header suggestion in include context"
    );

    let member_text = "position.";
    let member_items = provider.provide(
        Some(member_text),
        Position::new(0, member_text.len() as u32),
        None,
    );
    let has_swizzle = member_items
        .iter()
        .any(|item| item.label == "xyz" && item.kind == Some(CompletionItemKind::FIELD));
    assert!(
        has_swizzle,
        "expected member/swizzle suggestions for realistic member-access context"
    );
}

#[test]
fn semantic_tokens_cover_realistic_fixture_case() {
    let source = read_fixture("matmul/gemv/shaders/gemv_like.metal");
    let tree = SyntaxTree::parse(&source);
    let uri = fixture_uri("matmul/gemv/shaders/gemv_like.metal");
    let provider = SemanticTokenProvider::new(Arc::new(DefinitionProvider::new()));
    let tokens = provider.provide(&uri, Some(&tree));

    assert!(
        !tokens.is_empty(),
        "expected semantic tokens for fixture-like corpus"
    );
    assert!(
        tokens.len() > 8,
        "expected rich token stream for realistic fixture, got {}",
        tokens.len()
    );
}

#[tokio::test]
async fn edit_workflow_missing_symbol_error_disappears_after_fix() {
    if !has_metal_compiler() {
        return;
    }

    let compiler = MetalCompiler::new();
    compiler.ensure_system_includes_ready().await;

    let rel = "matmul/gemv/shaders/ref_user_a.metal";
    let fixed_source = read_fixture(rel);
    let broken_source = fixed_source.replace("params->scale", "params->missing_scale");
    let uri = fixture_uri(rel);
    let include_paths = include_paths_for(rel);

    let broken_diags = compiler
        .compile_with_include_paths(&broken_source, uri.as_str(), &include_paths)
        .await;
    assert!(
        broken_diags.iter().any(|d| {
            d.severity == DiagnosticSeverity::ERROR
                && (d.message.contains("missing_scale")
                    || d.message.to_lowercase().contains("no member"))
        }),
        "expected broken source to emit missing member error, got: {:?}",
        broken_diags
    );

    let fixed_diags = compiler
        .compile_with_include_paths(&fixed_source, uri.as_str(), &include_paths)
        .await;
    assert!(
        !fixed_diags.iter().any(|d| {
            d.severity == DiagnosticSeverity::ERROR
                && (d.message.contains("missing_scale")
                    || d.message.to_lowercase().contains("no member"))
        }),
        "fixed source should clear missing-member errors, got: {:?}",
        fixed_diags
    );
}

#[tokio::test]
async fn edit_workflow_macro_warning_disappears_after_fix() {
    if !has_metal_compiler() {
        return;
    }

    let compiler = MetalCompiler::new();
    compiler.ensure_system_includes_ready().await;

    let rel = "matmul/gemv/shaders/gemv_like.metal";
    let warning_source = read_fixture(rel);
    let fixed_source = warning_source
        .lines()
        .filter(|line| !line.contains("#define MTL_CONST static constant constexpr const"))
        .collect::<Vec<_>>()
        .join("\n");
    let uri = fixture_uri(rel);
    let include_paths = include_paths_for(rel);

    let warning_diags = compiler
        .compile_with_include_paths(&warning_source, uri.as_str(), &include_paths)
        .await;
    assert!(
        warning_diags
            .iter()
            .any(|d| d.message.to_lowercase().contains("redefine")),
        "expected macro redefinition warning before fix, got: {:?}",
        warning_diags
    );

    let fixed_diags = compiler
        .compile_with_include_paths(&fixed_source, uri.as_str(), &include_paths)
        .await;
    assert!(
        !fixed_diags
            .iter()
            .any(|d| d.message.to_lowercase().contains("redefine")),
        "expected macro redefinition warning to disappear after fix, got: {:?}",
        fixed_diags
    );
}
