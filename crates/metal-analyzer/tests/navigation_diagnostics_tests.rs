mod common;

use common::{
    fixture_path, fixture_uri, has_metal_compiler, include_paths_for, line_contains, position_of, read_fixture,
};
use metal_analyzer::{
    DefinitionProvider, IdeLocation, NavigationTarget,
    metal::compiler::{MetalCompiler, MetalDiagnostic},
    syntax::SyntaxTree,
};
use tower_lsp::lsp_types::{DiagnosticSeverity, Url};

fn first_location(resp: NavigationTarget) -> IdeLocation {
    match resp {
        NavigationTarget::Single(loc) => loc,
        NavigationTarget::Multiple(mut locs) => locs.remove(0),
    }
}

fn has_source_sidecar(
    dir: &std::path::Path,
    marker: &str,
) -> bool {
    std::fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .any(|name| name.contains(marker))
}

async fn compile_fixture(
    relative_path: &str,
    compiler: &MetalCompiler,
) -> Vec<MetalDiagnostic> {
    let source = read_fixture(relative_path);
    let uri = fixture_uri(relative_path);
    let include_paths = include_paths_for(relative_path);
    compiler.compile_with_include_paths(&source, uri.as_str(), &include_paths).await
}

#[test]
fn goto_def_include_resolves_generated_header() {
    if !has_metal_compiler() {
        return;
    }

    let source = read_fixture("matmul/gemv/shaders/gemv_like.metal");
    let uri = fixture_uri("matmul/gemv/shaders/gemv_like.metal");
    let include_paths = include_paths_for("matmul/gemv/shaders/gemv_like.metal");
    let snapshot = SyntaxTree::parse(&source);
    let provider = DefinitionProvider::new();

    let position = position_of(&source, "../../../generated/matmul.h");
    let result = provider
        .provide(&uri, position, &source, &include_paths, &snapshot, || false)
        .expect("expected include go-to-definition result");
    let target = first_location(result);

    assert!(
        target.file_path.to_string_lossy().ends_with("/generated/matmul.h"),
        "expected generated header target, got {}",
        target.file_path.display()
    );
}

#[test]
fn goto_def_prefers_qualified_fixture_transform() {
    if !has_metal_compiler() {
        return;
    }

    let source = read_fixture("matmul/gemv/shaders/gemv_like.metal");
    let uri = fixture_uri("matmul/gemv/shaders/gemv_like.metal");
    let include_paths = include_paths_for("matmul/gemv/shaders/gemv_like.metal");
    let snapshot = SyntaxTree::parse(&source);
    let provider = DefinitionProvider::new();

    let position = position_of(&source, "../../common/transforms.h");
    let result = provider
        .provide(&uri, position, &source, &include_paths, &snapshot, || false)
        .expect("expected include resolution for fixture transforms header");
    let target = first_location(result);

    assert!(
        target.file_path.to_string_lossy().ends_with("common/transforms.h"),
        "expected fixture transform header, got {}",
        target.file_path.display()
    );
}

#[test]
fn goto_def_prefers_qualified_steel_transform() {
    if !has_metal_compiler() {
        return;
    }

    let source = read_fixture("matmul/gemv/shaders/gemv_like.metal");
    let uri = fixture_uri("matmul/gemv/shaders/gemv_like.metal");
    let include_paths = include_paths_for("matmul/gemv/shaders/gemv_like.metal");
    let snapshot = SyntaxTree::parse(&source);
    let provider = DefinitionProvider::new();

    let position = position_of(&source, "../../common/steel/gemm/transforms.h");
    let result = provider
        .provide(&uri, position, &source, &include_paths, &snapshot, || false)
        .expect("expected include resolution for steel transforms header");
    let target = first_location(result);

    assert!(
        target.file_path.to_string_lossy().ends_with("steel/gemm/transforms.h"),
        "expected steel transform header, got {}",
        target.file_path.display()
    );
}

#[test]
fn goto_def_prefers_qualified_fixture_loader() {
    if !has_metal_compiler() {
        return;
    }

    let source = read_fixture("matmul/gemv/shaders/gemv_like.metal");
    let uri = fixture_uri("matmul/gemv/shaders/gemv_like.metal");
    let include_paths = include_paths_for("matmul/gemv/shaders/gemv_like.metal");
    let snapshot = SyntaxTree::parse(&source);
    let provider = DefinitionProvider::new();

    let position = position_of(&source, "../../common/loader.h");
    let result = provider
        .provide(&uri, position, &source, &include_paths, &snapshot, || false)
        .expect("expected include resolution for fixture loader header");
    let target = first_location(result);

    assert!(
        target.file_path.to_string_lossy().ends_with("common/loader.h")
            && !target.file_path.to_string_lossy().ends_with("steel/gemm/loader.h"),
        "expected fixture loader header, got {}",
        target.file_path.display()
    );
}

#[test]
fn goto_def_prefers_qualified_steel_loader() {
    if !has_metal_compiler() {
        return;
    }

    let source = read_fixture("matmul/gemv/shaders/gemv_like.metal");
    let uri = fixture_uri("matmul/gemv/shaders/gemv_like.metal");
    let include_paths = include_paths_for("matmul/gemv/shaders/gemv_like.metal");
    let snapshot = SyntaxTree::parse(&source);
    let provider = DefinitionProvider::new();

    let position = position_of(&source, "../../common/steel/gemm/loader.h");
    let result = provider
        .provide(&uri, position, &source, &include_paths, &snapshot, || false)
        .expect("expected include resolution for steel loader header");
    let target = first_location(result);

    assert!(
        target.file_path.to_string_lossy().ends_with("steel/gemm/loader.h"),
        "expected steel loader header, got {}",
        target.file_path.display()
    );
}

#[test]
fn goto_def_unqualified_loader_resolves_local_header() {
    if !has_metal_compiler() {
        return;
    }

    let rel = "matmul/gemv/shaders/ambiguous_loader_include.metal";
    let source = read_fixture(rel);
    let uri = fixture_uri(rel);
    let include_paths = include_paths_for(rel);
    let snapshot = SyntaxTree::parse(&source);
    let provider = DefinitionProvider::new();

    let position = position_of(&source, "\"loader.h\"");
    let result = provider
        .provide(&uri, position, &source, &include_paths, &snapshot, || false)
        .expect("expected include resolution for unqualified loader.h");
    let target = first_location(result);

    assert!(
        target.file_path.to_string_lossy().ends_with("matmul/gemv/shaders/loader.h"),
        "expected local loader.h to win for unqualified include, got {}",
        target.file_path.display()
    );
}

#[test]
fn goto_def_overloaded_symbol_resolves_function_definition() {
    if !has_metal_compiler() {
        return;
    }

    let source = read_fixture("matmul/gemv/shaders/gemv_like.metal");
    let uri = fixture_uri("matmul/gemv/shaders/gemv_like.metal");
    let include_paths = include_paths_for("matmul/gemv/shaders/gemv_like.metal");
    let snapshot = SyntaxTree::parse(&source);
    let provider = DefinitionProvider::new();

    let position = position_of(&source, "local_template(sum.re)");
    let result = provider
        .provide(&uri, position, &source, &include_paths, &snapshot, || false)
        .expect("expected definition for local_template call");
    let target = first_location(result);

    let target_path = target.file_path.clone();
    assert_eq!(
        target.file_path,
        uri.to_file_path().expect("fixture URI is file path"),
        "expected local template definition in same file"
    );
    assert!(line_contains(&target_path, "local_template("), "target file should contain local_template definition",);
}

#[test]
fn goto_def_template_parameter_resolves_without_compiler_fallback() {
    let source = r#"
template <typename T, const int BM, const int BN, const int TM>
struct Kernel {
  static constexpr int tgp_mem_size = BN > 1 ? BN * (BM + TM) : 0;
};
"#
    .to_string();
    let uri = Url::parse("file:///tmp/gemv_like_bn_fast_path.metal").expect("valid URI");
    let include_paths = Vec::new();
    let snapshot = SyntaxTree::parse(&source);
    let provider = DefinitionProvider::new();

    let position = position_of(&source, "BN > 1 ? BN * (BM + TM) : 0");
    let result = provider
        .provide(&uri, position, &source, &include_paths, &snapshot, || false)
        .expect("expected definition for template parameter BN");
    let target = first_location(result);

    assert_eq!(
        target.file_path,
        uri.to_file_path().expect("fixture URI is file path"),
        "BN should resolve in same file"
    );
    let line_text = source.lines().nth(target.range.start.line as usize).expect("definition line should exist");
    assert!(line_text.contains("const int BN"), "expected BN template parameter definition, got line: {line_text}");
}

#[tokio::test]
async fn diagnostics_include_macro_redefinition_note_pair() {
    if !has_metal_compiler() {
        return;
    }

    let compiler = MetalCompiler::new();
    compiler.ensure_system_includes_ready().await;
    let diagnostics = compile_fixture("matmul/gemv/shaders/gemv_like.metal", &compiler).await;

    let has_redefine_warning = diagnostics.iter().any(|d| {
        d.severity == DiagnosticSeverity::WARNING
            && d.message.to_lowercase().contains("redefine")
            && d.file.as_deref().is_some_and(|f| f.ends_with("gemv_like.metal"))
    });
    let has_previous_definition_note = diagnostics.iter().any(|d| {
        d.severity == DiagnosticSeverity::INFORMATION
            && d.message.to_lowercase().contains("previous definition")
            && d.file.as_deref().is_some_and(|f| f.ends_with("common/defines.h"))
    });

    assert!(has_redefine_warning, "expected macro-redefined warning in gemv_like.metal diagnostics: {:?}", diagnostics);
    assert!(
        has_previous_definition_note,
        "expected cross-file previous-definition note in common/defines.h diagnostics: {:?}",
        diagnostics
    );
}

#[tokio::test]
async fn diagnostics_report_deep_header_error_on_header_file() {
    if !has_metal_compiler() {
        return;
    }

    let compiler = MetalCompiler::new();
    compiler.ensure_system_includes_ready().await;
    let diagnostics = compile_fixture("matmul/gemv/shaders/deep_include_error.metal", &compiler).await;

    let header_error = diagnostics.iter().find(|d| {
        d.severity == DiagnosticSeverity::ERROR
            && d.file.as_deref().is_some_and(|f| f.ends_with("common/broken_header.h"))
    });

    assert!(header_error.is_some(), "expected header-attributed error for deep include chain, got: {:?}", diagnostics);
}

#[tokio::test]
async fn diagnostics_compile_does_not_create_source_sidecar_files() {
    if !has_metal_compiler() {
        return;
    }

    let compiler = MetalCompiler::new();
    compiler.ensure_system_includes_ready().await;
    let rel = "matmul/gemv/shaders/gemv_like.metal";
    let _ = compile_fixture(rel, &compiler).await;
    let source_dir = fixture_path(rel).parent().expect("fixture should have parent directory").to_path_buf();

    assert!(
        !has_source_sidecar(&source_dir, ".lsp-diag-"),
        "diagnostics compile should not create sidecar .lsp-diag files in source dir"
    );
}

#[test]
fn ast_dump_does_not_create_source_sidecar_files() {
    if !has_metal_compiler() {
        return;
    }

    let provider = DefinitionProvider::new();
    let rel = "matmul/gemv/shaders/gemv_like.metal";
    let uri = fixture_uri(rel);
    let source = read_fixture(rel);
    let include_paths = include_paths_for(rel);
    provider.index_document(&uri, &source, &include_paths);
    let source_dir = fixture_path(rel).parent().expect("fixture should have parent directory").to_path_buf();

    assert!(
        !has_source_sidecar(&source_dir, ".lsp-tmp.metal"),
        "AST dump should not create sidecar .lsp-tmp files in source dir"
    );
}
