use std::path::PathBuf;

use super::*;

fn unique_temp_dir(name: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("valid clock").as_nanos();
    std::env::temp_dir().join(format!("metal-analyzer-rewrite-includes-{name}-{}-{nonce}", std::process::id(),))
}

#[test]
fn rewrite_includes_rewrites_existing_local_quote_include() {
    let temp_dir = unique_temp_dir("local");
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let local_header = temp_dir.join("local.h");
    std::fs::write(&local_header, "// header\n").expect("write local header");

    let source = "#include \"local.h\"\n";
    let rewritten = rewrite_includes(source, &temp_dir, &[]);

    assert!(
        rewritten.contains(&local_header.display().to_string()),
        "expected include to be rewritten to existing local absolute path, got: {rewritten}"
    );

    let _ = std::fs::remove_file(local_header);
    let _ = std::fs::remove_dir(temp_dir);
}

#[test]
fn rewrite_includes_keeps_missing_local_quote_include_relative() {
    let temp_dir = unique_temp_dir("generated");
    std::fs::create_dir_all(&temp_dir).expect("temp dir");

    let source = "#include \"attention.h\"\n";
    let rewritten = rewrite_includes(source, &temp_dir, &[]);
    assert_eq!(rewritten, source, "missing local include must stay relative so include paths can resolve it");

    let _ = std::fs::remove_dir(temp_dir);
}

#[test]
fn rewrite_includes_resolves_via_include_paths_when_local_missing() {
    let temp_dir = unique_temp_dir("inc-paths");
    let base_dir = temp_dir.join("src");
    let include_root = temp_dir.join("include-root");
    std::fs::create_dir_all(&base_dir).expect("base dir");
    std::fs::create_dir_all(&include_root).expect("include root");
    let header = include_root.join("attention.h");
    std::fs::write(&header, "// header\n").expect("write header");

    let source = "#include \"attention.h\"\n";
    let include_paths = vec![include_root.display().to_string()];
    let rewritten = rewrite_includes(source, &base_dir, &include_paths);

    assert!(
        rewritten.contains(&header.display().to_string()),
        "expected include to be rewritten via -I path, got: {rewritten}"
    );

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn rewrite_includes_resolves_dotdot_via_include_paths() {
    let temp_dir = unique_temp_dir("dotdot");
    let base_dir = temp_dir.join("src").join("kernels");
    let include_root = temp_dir.join("project");
    let common_dir = include_root.join("common");
    std::fs::create_dir_all(&base_dir).expect("base dir");
    std::fs::create_dir_all(&common_dir).expect("common dir");
    let header = common_dir.join("matmul.h");
    std::fs::write(&header, "// header\n").expect("write header");

    let source = "#include \"../common/matmul.h\"\n";
    let include_paths = vec![include_root.display().to_string()];
    let rewritten = rewrite_includes(source, &base_dir, &include_paths);

    assert!(
        rewritten.contains(&header.display().to_string()) || rewritten.contains("common/matmul.h"),
        "expected dotdot include to be rewritten via -I path, got: {rewritten}"
    );

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn rewrite_includes_skips_framework_include_paths() {
    let temp_dir = unique_temp_dir("framework");
    std::fs::create_dir_all(&temp_dir).expect("temp dir");

    let source = "#include \"missing.h\"\n";
    let include_paths = vec![format!("{}/Frameworks", crate::metal::compiler::FRAMEWORK_DIR_PREFIX)];
    let rewritten = rewrite_includes(source, &temp_dir, &include_paths);

    assert_eq!(rewritten, source, "framework entries must not be treated as -I roots for quoted includes");

    let _ = std::fs::remove_dir(temp_dir);
}
