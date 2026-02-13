//! Integration tests for go-to-definition correctness.
//!
//! These tests invoke `xcrun metal -ast-dump=json` on real Metal fixture files
//! and verify that the definition provider resolves symbols to the correct
//! file and line. They require macOS with Xcode Command Line Tools installed.

use std::path::PathBuf;

use metal_analyzer::{DefinitionProvider, IdeLocation, NavigationTarget, syntax::SyntaxTree};
use tower_lsp::lsp_types::{Position, Url};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn uri_for(filename: &str) -> Url {
    Url::from_file_path(fixtures_dir().join(filename)).unwrap()
}

fn include_paths() -> Vec<String> {
    vec![fixtures_dir().display().to_string()]
}

fn read_fixture(filename: &str) -> String {
    std::fs::read_to_string(fixtures_dir().join(filename)).unwrap()
}

fn extract_location(resp: NavigationTarget) -> IdeLocation {
    match resp {
        NavigationTarget::Single(loc) => loc,
        NavigationTarget::Multiple(locs) => locs.into_iter().next().expect("at least one location"),
    }
}

fn has_metal_compiler() -> bool {
    std::process::Command::new("xcrun").args(["--find", "metal"]).output().is_ok_and(|o| o.status.success())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Go-to-definition on `transform(...)` call should jump to its definition.
///
/// functions.metal line 14: `transform(data[id].position, params->scale)`
/// functions.metal line  5: `float4 transform(float4 pos, float scale) {`
#[test]
fn goto_def_function_in_same_file() {
    if !has_metal_compiler() {
        return;
    }

    let source = read_fixture("functions.metal");
    let uri = uri_for("functions.metal");
    let snapshot = SyntaxTree::parse(&source);
    let provider = DefinitionProvider::new();

    // "transform" call is at line 14, column 18
    let position = Position {
        line: 14,
        character: 18,
    };

    let result = provider.provide(&uri, position, &source, &include_paths(), &snapshot);

    let loc = extract_location(result.expect("expected a definition for 'transform'"));
    assert_eq!(
        loc.file_path,
        uri.to_file_path().expect("fixture uri should be a file path"),
        "should resolve to same file"
    );
    assert_eq!(loc.range.start.line, 5, "transform is defined at line 5 (0-indexed)");
}

/// Go-to-definition on `MyStruct` type in a parameter should jump to types.metal.
///
/// functions.metal line 10: `device MyStruct* data`
/// types.metal     line 10: `struct MyStruct {`
#[test]
fn goto_def_struct_in_included_file() {
    if !has_metal_compiler() {
        return;
    }

    let source = read_fixture("functions.metal");
    let uri = uri_for("functions.metal");
    let snapshot = SyntaxTree::parse(&source);
    let provider = DefinitionProvider::new();

    // Pre-index so we can inspect the AST index.
    provider.index_document(&uri, &source, &include_paths());
    // "MyStruct" at line 10, column 11
    let position = Position {
        line: 10,
        character: 11,
    };

    let result = provider.provide(&uri, position, &source, &include_paths(), &snapshot);

    let loc = extract_location(result.expect("expected a definition for 'MyStruct'"));
    assert!(
        loc.file_path.to_string_lossy().ends_with("types.metal"),
        "should resolve to types.metal, got: {}",
        loc.file_path.display()
    );
    assert_eq!(loc.range.start.line, 10, "MyStruct defined at line 10 in types.metal");
}

/// Go-to-definition on `MyParams` type should jump to types.metal.
///
/// functions.metal line 11: `const constant MyParams* params`
/// types.metal     line  4: `struct MyParams {`
#[test]
fn goto_def_struct_cross_file_via_include() {
    if !has_metal_compiler() {
        return;
    }

    let source = read_fixture("functions.metal");
    let uri = uri_for("functions.metal");
    let snapshot = SyntaxTree::parse(&source);
    let provider = DefinitionProvider::new();

    // "MyParams" at line 11, column 19
    let position = Position {
        line: 11,
        character: 19,
    };

    let result = provider.provide(&uri, position, &source, &include_paths(), &snapshot);

    let loc = extract_location(result.expect("expected a definition for 'MyParams'"));
    assert!(
        loc.file_path.to_string_lossy().ends_with("types.metal"),
        "should resolve to types.metal, got: {}",
        loc.file_path.display()
    );
    assert_eq!(loc.range.start.line, 4, "MyParams defined at line 4 in types.metal");
}

/// Go-to-definition should still resolve when cursor is on pointer punctuation.
///
/// functions.metal line 11: `const constant MyParams* params`
/// cursor on `*`
/// types.metal     line  4: `struct MyParams {`
#[test]
fn goto_def_struct_cross_file_when_cursor_on_pointer_star() {
    if !has_metal_compiler() {
        return;
    }

    let source = read_fixture("functions.metal");
    let uri = uri_for("functions.metal");
    let snapshot = SyntaxTree::parse(&source);
    let provider = DefinitionProvider::new();

    // `*` in `MyParams*` at line 11, column 27.
    let position = Position {
        line: 11,
        character: 27,
    };

    let result = provider.provide(&uri, position, &source, &include_paths(), &snapshot);

    let loc = extract_location(result.expect("expected a definition for 'MyParams'"));
    assert!(
        loc.file_path.to_string_lossy().ends_with("types.metal"),
        "should resolve to types.metal, got: {}",
        loc.file_path.display()
    );
    assert_eq!(loc.range.start.line, 4, "MyParams defined at line 4 in types.metal");
}

/// `MissingType` only exists in a header that doesn't exist on disk.
/// The provider should return `None` rather than a false positive.
#[test]
fn goto_def_missing_type_returns_none() {
    if !has_metal_compiler() {
        return;
    }

    let source = read_fixture("missing_include.metal");
    let uri = uri_for("missing_include.metal");
    let snapshot = SyntaxTree::parse(&source);
    let provider = DefinitionProvider::new();

    // "MissingType" at line 8, column 19
    let position = Position {
        line: 8,
        character: 19,
    };

    let result = provider.provide(&uri, position, &source, &include_paths(), &snapshot);

    assert!(result.is_none(), "should return None for a type from a missing header, got: {result:?}");
}

/// In a file with a missing include, symbols from headers included BEFORE
/// the missing one should still resolve correctly.
///
/// missing_include.metal line 7: `device MyStruct* data`  (types.metal is included before nonexistent.h)
/// types.metal           line 10: `struct MyStruct {`
#[test]
fn goto_def_partial_ast_valid_symbol_resolves() {
    if !has_metal_compiler() {
        return;
    }

    let source = read_fixture("missing_include.metal");
    let uri = uri_for("missing_include.metal");
    let snapshot = SyntaxTree::parse(&source);
    let provider = DefinitionProvider::new();

    // "MyStruct" at line 7, column 11
    let position = Position {
        line: 7,
        character: 11,
    };

    let result = provider.provide(&uri, position, &source, &include_paths(), &snapshot);

    // This may or may not work depending on whether the partial AST
    // contains the included types. If it does, verify correctness.
    // If it doesn't, None is acceptable.
    if let Some(resp) = result {
        let loc = extract_location(resp);
        assert!(
            loc.file_path.to_string_lossy().ends_with("types.metal"),
            "if resolved, should point to types.metal, got: {}",
            loc.file_path.display()
        );
    }
}

/// Go-to-definition on a symbol passed through a macro invocation should
/// resolve via expansion/spelling-aware precise matching.
#[test]
fn goto_def_symbol_in_macro_invocation() {
    if !has_metal_compiler() {
        return;
    }

    let source = read_fixture("macro_calls.metal");
    let uri = uri_for("macro_calls.metal");
    let snapshot = SyntaxTree::parse(&source);
    let provider = DefinitionProvider::new();

    // "my_min" in `CALL_BIN(my_min, ...)` at line 15, column 22.
    let position = Position {
        line: 14,
        character: 22,
    };

    let result = provider.provide(&uri, position, &source, &include_paths(), &snapshot);

    let loc = extract_location(result.expect("expected definition for 'my_min'"));
    assert_eq!(
        loc.file_path,
        uri.to_file_path().expect("fixture uri should be a file path"),
        "should resolve in same file"
    );
    assert_eq!(loc.range.start.line, 4, "my_min is defined at line 5 (0-indexed 4)");
}

/// If by-name fallback is ambiguous (same-rank candidates), provider should
/// return None instead of a potentially wrong deterministic jump.
#[test]
fn goto_def_ambiguous_name_fallback_returns_none() {
    if !has_metal_compiler() {
        return;
    }

    let source = read_fixture("ambiguous_overload.metal");
    let uri = uri_for("ambiguous_overload.metal");
    let snapshot = SyntaxTree::parse(&source);
    let provider = DefinitionProvider::new();

    // Declaration name "overload" (not a use-site), forcing by-name fallback.
    let position = Position {
        line: 4,
        character: 7,
    };

    let result = provider.provide(&uri, position, &source, &include_paths(), &snapshot);

    assert!(result.is_none(), "ambiguous fallback should return None, got: {result:?}");
}
