use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use super::*;

/// Create a unique temporary directory for each test.
fn test_dir() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("metalfmt_test_{}_{id}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn find_metalfmt_toml_discovers_in_same_dir() {
    let dir = test_dir();
    let toml_path = dir.join("metalfmt.toml");
    fs::write(&toml_path, "indent_width = 4\n").unwrap();

    let source = dir.join("shader.metal");
    fs::write(&source, "").unwrap();

    let found = find_metalfmt_toml(&source);
    assert_eq!(found, Some(toml_path));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn find_metalfmt_toml_discovers_in_parent_dir() {
    let dir = test_dir();
    let toml_path = dir.join("metalfmt.toml");
    fs::write(&toml_path, "indent_width = 4\n").unwrap();

    let sub = dir.join("src");
    fs::create_dir_all(&sub).unwrap();
    let source = sub.join("shader.metal");
    fs::write(&source, "").unwrap();

    let found = find_metalfmt_toml(&source);
    assert_eq!(found, Some(toml_path));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn find_metalfmt_toml_returns_none_when_missing() {
    let dir = test_dir();
    let source = dir.join("shader.metal");
    fs::write(&source, "").unwrap();

    // No metalfmt.toml anywhere in dir — the walk will eventually hit
    // the filesystem root and return None.
    let found = find_metalfmt_toml(&source);
    assert!(found.is_none());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn load_inline_style_basic() {
    let dir = test_dir();
    let toml_path = dir.join("metalfmt.toml");
    fs::write(
        &toml_path,
        r#"
based_on_style = "LLVM"
indent_width = 4
column_limit = 120
"#,
    )
    .unwrap();

    let style = load_inline_style(&toml_path).unwrap();
    assert!(style.contains("BasedOnStyle: LLVM"));
    assert!(style.contains("IndentWidth: 4"));
    assert!(style.contains("ColumnLimit: 120"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn load_inline_style_bool_keys() {
    let dir = test_dir();
    let toml_path = dir.join("metalfmt.toml");
    fs::write(
        &toml_path,
        r#"
bin_pack_arguments = false
bin_pack_parameters = true
sort_includes = false
use_tab = true
"#,
    )
    .unwrap();

    let style = load_inline_style(&toml_path).unwrap();
    assert!(style.contains("BinPackArguments: false"));
    assert!(style.contains("BinPackParameters: true"));
    assert!(style.contains("SortIncludes: Never"));
    assert!(style.contains("UseTab: ForIndentation"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn load_inline_style_extra_keys_passthrough() {
    let dir = test_dir();
    let toml_path = dir.join("metalfmt.toml");
    fs::write(
        &toml_path,
        r#"
based_on_style = "LLVM"
continuation_indent_width = 8
derive_pointer_alignment = false
"#,
    )
    .unwrap();

    let style = load_inline_style(&toml_path).unwrap();
    assert!(style.contains("BasedOnStyle: LLVM"));
    assert!(style.contains("ContinuationIndentWidth: 8"));
    assert!(style.contains("DerivePointerAlignment: false"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn load_inline_style_empty_file_returns_none() {
    let dir = test_dir();
    let toml_path = dir.join("metalfmt.toml");
    fs::write(&toml_path, "").unwrap();

    assert!(load_inline_style(&toml_path).is_none());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn resolve_inline_style_full_chain() {
    let dir = test_dir();
    let toml_path = dir.join("metalfmt.toml");
    fs::write(
        &toml_path,
        r#"
based_on_style = "Google"
indent_width = 2
"#,
    )
    .unwrap();

    let sub = dir.join("shaders");
    fs::create_dir_all(&sub).unwrap();
    let source = sub.join("test.metal");
    fs::write(&source, "").unwrap();

    let style = resolve_inline_style(&source).unwrap();
    assert!(style.contains("BasedOnStyle: Google"));
    assert!(style.contains("IndentWidth: 2"));

    let _ = fs::remove_dir_all(&dir);
}
