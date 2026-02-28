use std::collections::HashMap;

use metal_analyzer::{AstIndex, RefSite, SymbolDef};

fn build_index(defs: Vec<SymbolDef>, refs: Vec<RefSite>) -> AstIndex {
    let mut id_to_def = HashMap::new();
    let mut name_to_defs: HashMap<String, Vec<usize>> = HashMap::new();
    let mut target_id_to_refs: HashMap<String, Vec<usize>> = HashMap::new();
    let mut file_to_defs: HashMap<String, Vec<usize>> = HashMap::new();
    let mut file_to_refs: HashMap<String, Vec<usize>> = HashMap::new();

    for (i, def) in defs.iter().enumerate() {
        id_to_def.entry(def.id.clone()).or_insert(i);
        name_to_defs.entry(def.name.clone()).or_default().push(i);
        file_to_defs.entry(def.file.clone()).or_default().push(i);
    }

    for (i, r) in refs.iter().enumerate() {
        target_id_to_refs
            .entry(r.target_id.clone())
            .or_default()
            .push(i);
        file_to_refs.entry(r.file.clone()).or_default().push(i);
    }

    AstIndex {
        defs,
        refs,
        id_to_def,
        name_to_defs,
        target_id_to_refs,
        file_to_defs,
        file_to_refs,
    }
}

#[test]
fn get_declarations_returns_only_non_definitions() {
    let defs = vec![
        SymbolDef {
            id: "0x1".into(),
            name: "Foo".into(),
            kind: "CXXRecordDecl".into(),
            file: "/tmp/decl.h".into(),
            line: 1,
            col: 1,
            is_definition: false,
            type_name: None,
            qual_type: None,
        },
        SymbolDef {
            id: "0x2".into(),
            name: "Foo".into(),
            kind: "CXXRecordDecl".into(),
            file: "/tmp/def.h".into(),
            line: 10,
            col: 2,
            is_definition: true,
            type_name: None,
            qual_type: None,
        },
    ];

    let index = build_index(defs, vec![]);
    let decls = index.get_declarations("Foo");

    assert_eq!(decls.len(), 1);
    assert_eq!(decls[0].file, "/tmp/decl.h");
}

#[test]
fn get_type_definition_prefers_user_over_system() {
    let user_def = SymbolDef {
        id: "0xU".into(),
        name: "MyType".into(),
        kind: "CXXRecordDecl".into(),
        file: "/project/include/my_type.h".into(),
        line: 5,
        col: 1,
        is_definition: true,
        type_name: None,
        qual_type: None,
    };
    let system_def = SymbolDef {
        id: "0xS".into(),
        name: "MyType".into(),
        kind: "CXXRecordDecl".into(),
        file: "/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/metal/include/my_type".into(),
        line: 42,
        col: 1,
        is_definition: true,
        type_name: None,
        qual_type: None,
    };
    let var_def = SymbolDef {
        id: "0xV".into(),
        name: "value".into(),
        kind: "VarDecl".into(),
        file: "/project/src/main.metal".into(),
        line: 12,
        col: 3,
        is_definition: true,
        type_name: Some("MyType".into()),
        qual_type: Some("MyType".into()),
    };

    let index = build_index(vec![user_def.clone(), system_def, var_def.clone()], vec![]);
    let ty = index
        .get_type_definition(&var_def)
        .expect("type definition");

    assert_eq!(ty.file, user_def.file);
    assert_eq!(ty.name, "MyType");
}

#[test]
fn get_type_definition_prefers_definitions() {
    let decl = SymbolDef {
        id: "0xD".into(),
        name: "Vec2".into(),
        kind: "CXXRecordDecl".into(),
        file: "/project/include/vec2.h".into(),
        line: 1,
        col: 1,
        is_definition: false,
        type_name: None,
        qual_type: None,
    };
    let def = SymbolDef {
        id: "0xF".into(),
        name: "Vec2".into(),
        kind: "CXXRecordDecl".into(),
        file: "/project/include/vec2.h".into(),
        line: 20,
        col: 1,
        is_definition: true,
        type_name: None,
        qual_type: None,
    };
    let var_def = SymbolDef {
        id: "0xV".into(),
        name: "value".into(),
        kind: "VarDecl".into(),
        file: "/project/src/main.metal".into(),
        line: 12,
        col: 3,
        is_definition: true,
        type_name: Some("Vec2".into()),
        qual_type: Some("Vec2".into()),
    };

    let index = build_index(vec![decl, def.clone(), var_def.clone()], vec![]);
    let ty = index
        .get_type_definition(&var_def)
        .expect("type definition");

    assert!(ty.is_definition);
    assert_eq!(ty.line, def.line);
}

#[test]
fn get_references_returns_refs_for_target_id() {
    let refs = vec![
        RefSite {
            file: "/tmp/a.metal".into(),
            line: 2,
            col: 5,
            tok_len: 1,
            target_id: "0xA".into(),
            target_name: "Foo".into(),
            target_kind: "CXXRecordDecl".into(),
            expansion: None,
            spelling: None,
        },
        RefSite {
            file: "/tmp/b.metal".into(),
            line: 3,
            col: 7,
            tok_len: 1,
            target_id: "0xB".into(),
            target_name: "Bar".into(),
            target_kind: "CXXRecordDecl".into(),
            expansion: None,
            spelling: None,
        },
    ];

    let index = build_index(vec![], refs);
    let refs_for_a = index.get_references("0xA");

    assert_eq!(refs_for_a.len(), 1);
    assert_eq!(refs_for_a[0].file, "/tmp/a.metal");
}

#[test]
fn get_references_in_file_filters_by_path() {
    let refs = vec![
        RefSite {
            file: "/tmp/a.metal".into(),
            line: 2,
            col: 5,
            tok_len: 1,
            target_id: "0xA".into(),
            target_name: "Foo".into(),
            target_kind: "CXXRecordDecl".into(),
            expansion: None,
            spelling: None,
        },
        RefSite {
            file: "/tmp/a.metal".into(),
            line: 4,
            col: 8,
            tok_len: 1,
            target_id: "0xB".into(),
            target_name: "Bar".into(),
            target_kind: "CXXRecordDecl".into(),
            expansion: None,
            spelling: None,
        },
        RefSite {
            file: "/tmp/b.metal".into(),
            line: 1,
            col: 2,
            tok_len: 1,
            target_id: "0xC".into(),
            target_name: "Baz".into(),
            target_kind: "CXXRecordDecl".into(),
            expansion: None,
            spelling: None,
        },
    ];

    let index = build_index(vec![], refs);
    let refs_in_a = index.get_references_in_file("/tmp/a.metal");

    assert_eq!(refs_in_a.len(), 2);
    assert!(refs_in_a.iter().all(|r| r.file == "/tmp/a.metal"));
}

#[test]
fn get_implementations_returns_only_definitions() {
    let defs = vec![
        SymbolDef {
            id: "0x1".into(),
            name: "helper".into(),
            kind: "FunctionDecl".into(),
            file: "/project/include/helpers.h".into(),
            line: 1,
            col: 1,
            is_definition: false,
            type_name: None,
            qual_type: None,
        },
        SymbolDef {
            id: "0x2".into(),
            name: "helper".into(),
            kind: "FunctionDecl".into(),
            file: "/project/src/helpers.metal".into(),
            line: 10,
            col: 1,
            is_definition: true,
            type_name: None,
            qual_type: None,
        },
    ];

    let index = build_index(defs, vec![]);
    let impls = index.get_implementations("helper");

    assert_eq!(impls.len(), 1);
    assert!(impls[0].is_definition);
    assert_eq!(impls[0].file, "/project/src/helpers.metal");
}
