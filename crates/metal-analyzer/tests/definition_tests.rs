use metal_analyzer::{SymbolDef, def_to_location, is_system_header, normalize_type_name, paths_match};

#[test]
fn normalize_type_name_strips_qualifiers() {
    assert_eq!(normalize_type_name("Foo"), Some("Foo".to_string()));
    assert_eq!(
        normalize_type_name("const struct Foo *"),
        Some("Foo".to_string())
    );
    assert_eq!(
        normalize_type_name("threadgroup metal::Foo &"),
        Some("Foo".to_string())
    );
}

#[test]
fn system_header_detection() {
    assert!(is_system_header(
        "/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/metal/include/metal_stdlib"
    ));
    assert!(is_system_header(
        "/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk/usr/include/stdio.h"
    ));
    assert!(!is_system_header("/Users/dev/project/shaders/utils.h"));
    assert!(!is_system_header("/tmp/test.metal"));
}

#[test]
fn paths_match_identical() {
    assert!(paths_match("/tmp/foo.metal", "/tmp/foo.metal"));
}

#[test]
fn paths_match_different() {
    assert!(!paths_match("/tmp/foo.metal", "/tmp/bar.metal"));
}

#[test]
fn def_to_location_converts_to_zero_based() {
    let def = SymbolDef {
        id: "0x1".into(),
        name: "Foo".into(),
        kind: "CXXRecordDecl".into(),
        file: "/tmp/test.metal".into(),
        line: 3,
        col: 8,
        is_definition: true,
        type_name: None,
        qual_type: None,
    };

    let loc = def_to_location(&def).expect("expected location");
    assert_eq!(loc.range.start.line, 2);
    assert_eq!(loc.range.start.character, 7);
    assert_eq!(loc.range.end.character, 10); // 7 + len("Foo")
    assert_eq!(loc.uri.path(), "/tmp/test.metal");
}
