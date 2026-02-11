    use super::*;

    static AST_DUMP_COUNTER_TEST_LOCK: once_cell::sync::Lazy<std::sync::Mutex<()>> =
        once_cell::sync::Lazy::new(|| std::sync::Mutex::new(()));

    fn has_metal_compiler() -> bool {
        std::process::Command::new("xcrun")
            .args(["--find", "metal"])
            .output()
            .is_ok_and(|o| o.status.success())
    }

    fn position_of(source: &str, needle: &str) -> Position {
        let idx = source.find(needle).expect("needle must exist");
        let before = &source[..idx];
        let line = before.as_bytes().iter().filter(|&&b| b == b'\n').count() as u32;
        let col = before
            .rsplit_once('\n')
            .map(|(_, tail)| tail.chars().count() as u32)
            .unwrap_or_else(|| before.chars().count() as u32);
        Position::new(line, col)
    }

    #[test]
    fn local_template_parameter_fast_path_resolves_usage() {
        let source = r#"
template <typename T, const int BN, const int TM>
struct Kernel {
  int value = BN * TM;
};
"#;
        let snapshot = SyntaxTree::parse(source);
        let uri = Url::parse("file:///tmp/kernel.metal").expect("valid uri");
        let usage = position_of(source, "BN * TM");

        let result =
            resolve_local_template_parameter(&uri, &snapshot, source, usage, "BN")
                .expect("template param should resolve");
        let GotoDefinitionResponse::Scalar(location) = result else {
            panic!("expected scalar response");
        };

        let definition_line = source
            .lines()
            .nth(location.range.start.line as usize)
            .expect("definition line");
        assert!(
            definition_line.contains("const int BN"),
            "expected BN template parameter definition, got line: {definition_line}"
        );
    }

    #[test]
    fn by_name_field_tie_uses_member_receiver_type_to_disambiguate() {
        let source = r#"
static METAL_FUNC void run(constant PrimaryParams* state) {
  int iteration_count = state->iteration_limit;
}
"#;
        let position = position_of(source, "iteration_limit");
        let source_file = "/tmp/member_field_tie.metal";
        let member_file = "/tmp/member_field_defs.h";

        let defs = vec![
            SymbolDef {
                id: "record-primary".into(),
                name: "PrimaryParams".into(),
                kind: "CXXRecordDecl".into(),
                file: member_file.into(),
                line: 10,
                col: 8,
                is_definition: true,
                type_name: None,
                qual_type: None,
            },
            SymbolDef {
                id: "field-primary".into(),
                name: "iteration_limit".into(),
                kind: "FieldDecl".into(),
                file: member_file.into(),
                line: 28,
                col: 13,
                is_definition: true,
                type_name: Some("int".into()),
                qual_type: Some("const int".into()),
            },
            SymbolDef {
                id: "record-secondary".into(),
                name: "SecondaryParams".into(),
                kind: "CXXRecordDecl".into(),
                file: member_file.into(),
                line: 33,
                col: 8,
                is_definition: true,
                type_name: None,
                qual_type: None,
            },
            SymbolDef {
                id: "field-secondary".into(),
                name: "iteration_limit".into(),
                kind: "FieldDecl".into(),
                file: member_file.into(),
                line: 49,
                col: 13,
                is_definition: true,
                type_name: Some("int".into()),
                qual_type: Some("const int".into()),
            },
            SymbolDef {
                id: "parm-state".into(),
                name: "state".into(),
                kind: "ParmVarDecl".into(),
                file: source_file.into(),
                line: 2,
                col: 49,
                is_definition: true,
                type_name: Some("PrimaryParams".into()),
                qual_type: Some("constant PrimaryParams *".into()),
            },
        ];

        let mut name_to_defs = std::collections::HashMap::new();
        name_to_defs.insert("iteration_limit".to_string(), vec![1, 3]);
        name_to_defs.insert("state".to_string(), vec![4]);

        let index = AstIndex {
            defs,
            refs: Vec::new(),
            id_to_def: std::collections::HashMap::new(),
            name_to_defs,
            target_id_to_refs: std::collections::HashMap::new(),
            file_to_defs: std::collections::HashMap::new(),
            file_to_refs: std::collections::HashMap::new(),
        };

        let result = resolve_by_name(
            &index,
            source_file,
            source,
            position,
            "iteration_limit",
        )
        .expect("tie should be disambiguated to the primary owner field");
        let GotoDefinitionResponse::Scalar(location) = result else {
            panic!("expected scalar response");
        };

        assert!(
            location.uri.path().ends_with("/tmp/member_field_defs.h"),
            "expected member_field_defs.h target, got {}",
            location.uri.path()
        );
        assert_eq!(location.range.start.line, 27);
    }

    #[test]
    fn by_name_field_tie_without_receiver_type_remains_ambiguous() {
        let source = "int v = ptr->iteration_limit;";
        let position = position_of(source, "iteration_limit");
        let source_file = "/tmp/member_field_tie.metal";
        let member_file = "/tmp/member_field_defs.h";

        let defs = vec![
            SymbolDef {
                id: "record-primary".into(),
                name: "PrimaryParams".into(),
                kind: "CXXRecordDecl".into(),
                file: member_file.into(),
                line: 10,
                col: 8,
                is_definition: true,
                type_name: None,
                qual_type: None,
            },
            SymbolDef {
                id: "field-primary".into(),
                name: "iteration_limit".into(),
                kind: "FieldDecl".into(),
                file: member_file.into(),
                line: 28,
                col: 13,
                is_definition: true,
                type_name: Some("int".into()),
                qual_type: Some("const int".into()),
            },
            SymbolDef {
                id: "record-secondary".into(),
                name: "SecondaryParams".into(),
                kind: "CXXRecordDecl".into(),
                file: member_file.into(),
                line: 33,
                col: 8,
                is_definition: true,
                type_name: None,
                qual_type: None,
            },
            SymbolDef {
                id: "field-secondary".into(),
                name: "iteration_limit".into(),
                kind: "FieldDecl".into(),
                file: member_file.into(),
                line: 49,
                col: 13,
                is_definition: true,
                type_name: Some("int".into()),
                qual_type: Some("const int".into()),
            },
        ];

        let mut name_to_defs = std::collections::HashMap::new();
        name_to_defs.insert("iteration_limit".to_string(), vec![1, 3]);

        let index = AstIndex {
            defs,
            refs: Vec::new(),
            id_to_def: std::collections::HashMap::new(),
            name_to_defs,
            target_id_to_refs: std::collections::HashMap::new(),
            file_to_defs: std::collections::HashMap::new(),
            file_to_refs: std::collections::HashMap::new(),
        };

        let result = resolve_by_name(
            &index,
            source_file,
            source,
            position,
            "iteration_limit",
        );
        assert!(
            result.is_none(),
            "without receiver type info, tie should remain ambiguous"
        );
    }

    #[test]
    fn by_name_method_tie_uses_receiver_type_and_prefers_non_const_overload() {
        let source = r#"
static METAL_FUNC void run(thread auto& tile, short lane) {
  thread auto& fragment = tile.element_at(0, lane);
}
"#;
        let position = position_of(source, "element_at(0, lane)");
        let source_file = "/tmp/member_method_tie.metal";
        let member_file = "/tmp/member_owner.h";

        let defs = vec![
            SymbolDef {
                id: "record-tile".into(),
                name: "TileOwner".into(),
                kind: "CXXRecordDecl".into(),
                file: member_file.into(),
                line: 10,
                col: 8,
                is_definition: true,
                type_name: None,
                qual_type: None,
            },
            SymbolDef {
                id: "method-element-at-mutable".into(),
                name: "element_at".into(),
                kind: "CXXMethodDecl".into(),
                file: member_file.into(),
                line: 20,
                col: 20,
                is_definition: true,
                type_name: None,
                qual_type: Some("thread element_type &(const short, const short)".into()),
            },
            SymbolDef {
                id: "method-element-at-const".into(),
                name: "element_at".into(),
                kind: "CXXMethodDecl".into(),
                file: member_file.into(),
                line: 24,
                col: 26,
                is_definition: true,
                type_name: None,
                qual_type: Some(
                    "const thread element_type &(const short, const short) const".into(),
                ),
            },
            SymbolDef {
                id: "parm-tile".into(),
                name: "tile".into(),
                kind: "ParmVarDecl".into(),
                file: source_file.into(),
                line: 2,
                col: 41,
                is_definition: true,
                type_name: Some("TileOwner".into()),
                qual_type: Some("thread TileOwner &".into()),
            },
        ];

        let mut name_to_defs = std::collections::HashMap::new();
        name_to_defs.insert("element_at".to_string(), vec![1, 2]);
        name_to_defs.insert("tile".to_string(), vec![3]);

        let index = AstIndex {
            defs,
            refs: Vec::new(),
            id_to_def: std::collections::HashMap::new(),
            name_to_defs,
            target_id_to_refs: std::collections::HashMap::new(),
            file_to_defs: std::collections::HashMap::new(),
            file_to_refs: std::collections::HashMap::new(),
        };

        let result = resolve_by_name(&index, source_file, source, position, "element_at")
            .expect("method tie should resolve to the mutable overload");
        let GotoDefinitionResponse::Scalar(location) = result else {
            panic!("expected scalar response");
        };

        assert!(
            location.uri.path().ends_with("/tmp/member_owner.h"),
            "expected member_owner.h target, got {}",
            location.uri.path()
        );
        assert_eq!(location.range.start.line, 19);
    }

    #[test]
    fn by_name_method_tie_without_receiver_type_keeps_unique_owner() {
        let source = "thread auto& fragment = tile.element_at(0, lane);";
        let position = position_of(source, "element_at(0, lane)");
        let source_file = "/tmp/member_method_tie.metal";
        let member_file = "/tmp/member_owner.h";

        let defs = vec![
            SymbolDef {
                id: "record-tile".into(),
                name: "TileOwner".into(),
                kind: "CXXRecordDecl".into(),
                file: member_file.into(),
                line: 10,
                col: 8,
                is_definition: true,
                type_name: None,
                qual_type: None,
            },
            SymbolDef {
                id: "method-element-at-mutable".into(),
                name: "element_at".into(),
                kind: "CXXMethodDecl".into(),
                file: member_file.into(),
                line: 20,
                col: 20,
                is_definition: true,
                type_name: None,
                qual_type: Some("thread element_type &(const short, const short)".into()),
            },
            SymbolDef {
                id: "method-element-at-const".into(),
                name: "element_at".into(),
                kind: "CXXMethodDecl".into(),
                file: member_file.into(),
                line: 24,
                col: 26,
                is_definition: true,
                type_name: None,
                qual_type: Some(
                    "const thread element_type &(const short, const short) const".into(),
                ),
            },
        ];

        let mut name_to_defs = std::collections::HashMap::new();
        name_to_defs.insert("element_at".to_string(), vec![1, 2]);

        let index = AstIndex {
            defs,
            refs: Vec::new(),
            id_to_def: std::collections::HashMap::new(),
            name_to_defs,
            target_id_to_refs: std::collections::HashMap::new(),
            file_to_defs: std::collections::HashMap::new(),
            file_to_refs: std::collections::HashMap::new(),
        };

        let result = resolve_by_name(&index, source_file, source, position, "element_at")
            .expect("method tie with a unique owner should resolve");
        let GotoDefinitionResponse::Scalar(location) = result else {
            panic!("expected scalar response");
        };
        assert_eq!(location.range.start.line, 19);
    }

    #[test]
    fn by_name_method_tie_without_receiver_type_stays_ambiguous_for_multiple_owners() {
        let source = "thread auto& fragment = tile.element_at(0, lane);";
        let position = position_of(source, "element_at(0, lane)");
        let source_file = "/tmp/member_method_tie.metal";

        let defs = vec![
            SymbolDef {
                id: "record-a".into(),
                name: "OwnerA".into(),
                kind: "CXXRecordDecl".into(),
                file: "/tmp/a.h".into(),
                line: 10,
                col: 8,
                is_definition: true,
                type_name: None,
                qual_type: None,
            },
            SymbolDef {
                id: "method-a".into(),
                name: "element_at".into(),
                kind: "CXXMethodDecl".into(),
                file: "/tmp/a.h".into(),
                line: 20,
                col: 20,
                is_definition: true,
                type_name: None,
                qual_type: Some("thread element_type &(const short, const short)".into()),
            },
            SymbolDef {
                id: "record-b".into(),
                name: "OwnerB".into(),
                kind: "CXXRecordDecl".into(),
                file: "/tmp/b.h".into(),
                line: 10,
                col: 8,
                is_definition: true,
                type_name: None,
                qual_type: None,
            },
            SymbolDef {
                id: "method-b".into(),
                name: "element_at".into(),
                kind: "CXXMethodDecl".into(),
                file: "/tmp/b.h".into(),
                line: 20,
                col: 20,
                is_definition: true,
                type_name: None,
                qual_type: Some("thread element_type &(const short, const short)".into()),
            },
        ];

        let mut name_to_defs = std::collections::HashMap::new();
        name_to_defs.insert("element_at".to_string(), vec![1, 3]);

        let index = AstIndex {
            defs,
            refs: Vec::new(),
            id_to_def: std::collections::HashMap::new(),
            name_to_defs,
            target_id_to_refs: std::collections::HashMap::new(),
            file_to_defs: std::collections::HashMap::new(),
            file_to_refs: std::collections::HashMap::new(),
        };

        let result = resolve_by_name(&index, source_file, source, position, "element_at");
        assert!(
            result.is_none(),
            "method tie across multiple owners should remain ambiguous without receiver type",
        );
    }

    #[test]
    fn cast_operators_are_non_navigable_symbols() {
        assert!(is_non_navigable_symbol("static_cast"));
        assert!(is_non_navigable_symbol("dynamic_cast"));
        assert!(is_non_navigable_symbol("reinterpret_cast"));
        assert!(is_non_navigable_symbol("const_cast"));
        assert!(!is_non_navigable_symbol("AccumType"));
    }

    #[test]
    fn builtin_navigation_candidates_skip_language_keywords() {
        assert!(is_builtin_navigation_candidate("threadgroup_barrier"));
        assert!(is_builtin_navigation_candidate("simd_sum"));
        assert!(is_builtin_navigation_candidate("mem_flags"));
        assert!(is_builtin_navigation_candidate("mem_threadgroup"));
        assert!(!is_builtin_navigation_candidate("if"));
        assert!(!is_builtin_navigation_candidate("while"));
    }

    #[test]
    fn extract_namespace_qualifier_before_word_detects_scoped_access() {
        let source = "const auto mode = address::repeat;";
        let position = position_of(source, "repeat");
        let qualifier = extract_namespace_qualifier_before_word(source, position, "repeat")
            .expect("qualified symbol should expose namespace");
        assert_eq!(qualifier, "address");
    }

    #[test]
    fn should_fast_lookup_system_symbol_detects_prefixes_and_namespaces() {
        let src_prefixed = "threadgroup_barrier(mem_flags::mem_threadgroup);";
        assert!(
            should_fast_lookup_system_symbol(
                src_prefixed,
                position_of(src_prefixed, "threadgroup_barrier"),
                "threadgroup_barrier",
            ),
            "known system prefix symbols should take fast system-header path"
        );

        let src_scoped = "const auto mode = compare_func::greater_equal;";
        assert!(
            should_fast_lookup_system_symbol(
                src_scoped,
                position_of(src_scoped, "greater_equal"),
                "greater_equal",
            ),
            "recognized system namespaces should take fast system-header path"
        );

        let src_project = "const int custom_helper = 1;";
        assert!(
            !should_fast_lookup_system_symbol(
                src_project,
                position_of(src_project, "custom_helper"),
                "custom_helper",
            ),
            "project-local identifiers should not trigger system-header fast path"
        );
    }

    #[test]
    fn find_scoped_enum_member_offset_locates_member_in_enum_body() {
        let src = r#"
enum class address {
  clamp_to_edge,
  repeat,
  mirrored_repeat
};
"#;
        let offset = find_scoped_enum_member_offset(src, "address", "repeat")
            .expect("scoped enum member should be found");
        assert_eq!(&src[offset..offset + "repeat".len()], "repeat");
    }

    #[test]
    fn resolve_fast_system_symbol_location_uses_scoped_enum_fast_case() {
        let temp_dir = std::env::temp_dir().join(format!(
            "metal-analyzer-fast-system-lookup-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock drift")
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_dir).expect("create fake include root");
        let header_path = temp_dir.join("metal_fake");
        std::fs::write(
            &header_path,
            r#"
enum class address {
  clamp_to_edge,
  repeat
};
"#,
        )
        .expect("write fake metal header");

        let source = "const auto mode = address::repeat;";
        let position = position_of(source, "repeat");
        let include_paths = vec![temp_dir.display().to_string()];
        let result =
            resolve_fast_system_symbol_location(source, position, "repeat", &include_paths)
                .expect("fast system lookup should resolve namespaced enum symbol");
        let GotoDefinitionResponse::Scalar(location) = result else {
            panic!("expected scalar location for fast system lookup");
        };
        assert!(
            location.uri.path().ends_with("metal_fake"),
            "expected synthetic metal header location, got {}",
            location.uri.path()
        );

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn system_builtin_header_candidates_include_dynamic_toolchain_headers() {
        let temp_dir = std::env::temp_dir().join(format!(
            "metal-analyzer-builtin-header-candidates-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock drift")
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_dir).expect("create fake include root");
        let dynamic_header = temp_dir.join("metal_experimental");
        let nested_dynamic_header = temp_dir.join("metal").join("metal_future");
        std::fs::create_dir_all(
            nested_dynamic_header
                .parent()
                .expect("nested fake header should have parent directory"),
        )
        .expect("create nested include root");
        std::fs::write(&dynamic_header, "// synthetic metal header").expect("write dynamic header");
        std::fs::write(&nested_dynamic_header, "// synthetic nested metal header")
            .expect("write nested dynamic header");

        let include_paths = vec![temp_dir.display().to_string()];
        let candidates = system_builtin_header_candidates(&include_paths);
        assert!(
            candidates.contains(&dynamic_header.canonicalize().expect("canonical dynamic header")),
            "dynamic headers in include roots should be considered as builtin symbol candidates"
        );
        assert!(
            candidates.contains(
                &nested_dynamic_header
                    .canonicalize()
                    .expect("canonical nested dynamic header")
            ),
            "dynamic headers under include_root/metal should be considered as builtin symbol candidates"
        );

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn find_word_boundary_offset_avoids_partial_identifier_matches() {
        let src = "mem_threadgroup mem_threadgroup2 xmem_threadgroup";
        let first = find_word_boundary_offset(src, "mem_threadgroup")
            .expect("exact token should be found");
        assert_eq!(&src[first..first + "mem_threadgroup".len()], "mem_threadgroup");

        let second_search = find_word_boundary_offset(&src[first + 1..], "mem_threadgroup");
        assert!(
            second_search.is_none(),
            "partial identifier matches should be rejected",
        );
    }

    #[tokio::test]
    async fn provide_resolves_metal_system_sync_symbols() {
        if !has_metal_compiler() {
            return;
        }
        let _guard = AST_DUMP_COUNTER_TEST_LOCK
            .lock()
            .expect("AST dump test lock should not be poisoned");

        let source = r#"
#include <metal_stdlib>
using namespace metal;

kernel void k(device float* sinks [[buffer(0)]], uint tid [[thread_position_in_grid]]) {
  if (tid == 0) {
    threadgroup_barrier(mem_flags::mem_threadgroup);
    sinks[0] = 0.0f;
  }
}
"#;
        let uri = Url::parse("file:///tmp/sync_symbols_jump.metal").expect("valid uri");
        let snapshot = SyntaxTree::parse(source);
        let provider = DefinitionProvider::new();

        let include_paths = {
            let compiler = crate::metal::compiler::MetalCompiler::new();
            compiler.discover_system_includes().await;
            compiler
                .get_system_include_paths()
                .into_iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
        };

        for symbol in ["threadgroup_barrier", "mem_flags", "mem_threadgroup"] {
            let position = position_of(source, symbol);
            let result = provider
                .provide(&uri, position, source, &include_paths, &snapshot)
                .await;

            let Some(GotoDefinitionResponse::Scalar(location)) = result else {
                panic!("expected goto-definition target for builtin symbol: {symbol}");
            };
            assert!(
                location.uri.path().contains("/metal/"),
                "expected symbol `{symbol}` to resolve into a Metal SDK header, got {}",
                location.uri.path()
            );
        }
    }

    #[tokio::test]
    async fn provide_returns_none_for_static_cast_keyword() {
        let source = r#"
kernel void k(device float* sinks [[buffer(0)]], uint tid [[thread_position_in_grid]]) {
  float value = static_cast<float>(sinks[tid]);
}
"#;
        let uri = Url::parse("file:///tmp/static_cast_keyword.metal").expect("valid uri");
        let position = position_of(source, "static_cast<float>");
        let snapshot = SyntaxTree::parse(source);
        let provider = DefinitionProvider::new();

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            provider.provide(&uri, position, source, &Vec::new(), &snapshot),
        )
        .await
        .expect("static_cast lookup should not block");

        assert!(
            result.is_none(),
            "goto-definition on static_cast should return no symbol location"
        );
    }

    #[tokio::test]
    async fn provide_returns_none_for_control_flow_keyword_without_ast_dump() {
        let _guard = AST_DUMP_COUNTER_TEST_LOCK
            .lock()
            .expect("AST dump test lock should not be poisoned");

        let source = r#"
kernel void k(device float* sinks [[buffer(0)]], uint tid [[thread_position_in_grid]]) {
  if (tid > 0) {
    sinks[tid] = 0.0f;
  }
}
"#;
        let uri = Url::parse("file:///tmp/if_keyword.metal").expect("valid uri");
        let position = position_of(source, "if (tid > 0)");
        let snapshot = SyntaxTree::parse(source);
        let provider = DefinitionProvider::new();
        let before = super::super::compiler::ast_dump_counter();

        let result = provider
            .provide(&uri, position, source, &Vec::new(), &snapshot)
            .await;

        let after = super::super::compiler::ast_dump_counter();
        assert!(
            result.is_none(),
            "goto-definition on `if` should return no symbol location"
        );
        assert_eq!(
            after - before,
            0,
            "language keyword lookup should not trigger AST dump work"
        );
    }

    #[tokio::test]
    async fn concurrent_goto_definition_for_same_file_runs_single_ast_dump() {
        if !has_metal_compiler() {
            return;
        }
        let _guard = AST_DUMP_COUNTER_TEST_LOCK
            .lock()
            .expect("AST dump test lock should not be poisoned");

        let fixtures_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let source_path = fixtures_dir.join("functions.metal");
        let uri = Url::from_file_path(&source_path).expect("fixture URI");
        let unique_suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("valid clock")
            .as_nanos();
        let source = format!(
            "{}\n// concurrent-goto-def-test-{unique_suffix}\n",
            std::fs::read_to_string(&source_path).expect("fixture source")
        );
        let snapshot = Arc::new(SyntaxTree::parse(&source));
        let include_paths = Arc::new(vec![fixtures_dir.display().to_string()]);
        let source = Arc::new(source);
        let provider = Arc::new(DefinitionProvider::new());

        let cursor = position_of(source.as_str(), "transform(data[id].position");
        let workers = 8usize;
        let barrier = Arc::new(tokio::sync::Barrier::new(workers));
        let before = super::super::compiler::ast_dump_counter();

        let responses = futures::future::join_all((0..workers).map(|_| {
            let provider = Arc::clone(&provider);
            let uri = uri.clone();
            let source = Arc::clone(&source);
            let include_paths = Arc::clone(&include_paths);
            let snapshot = Arc::clone(&snapshot);
            let barrier = Arc::clone(&barrier);
            async move {
                barrier.wait().await;
                provider
                    .provide(
                        &uri,
                        cursor,
                        source.as_str(),
                        include_paths.as_slice(),
                        snapshot.as_ref(),
                    )
                    .await
            }
        }))
        .await;

        for response in responses {
            assert!(
                response.is_some(),
                "all rapid navigation requests should resolve definition"
            );
        }

        let after = super::super::compiler::ast_dump_counter();
        assert_eq!(
            after - before,
            1,
            "concurrent jumps on the same document should share one AST dump build"
        );
    }

    #[tokio::test]
    async fn concurrent_index_document_and_provide_share_single_ast_dump() {
        if !has_metal_compiler() {
            return;
        }
        let _guard = AST_DUMP_COUNTER_TEST_LOCK
            .lock()
            .expect("AST dump test lock should not be poisoned");

        let fixtures_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let source_path = fixtures_dir.join("functions.metal");
        let uri = Url::from_file_path(&source_path).expect("fixture URI");
        let unique_suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("valid clock")
            .as_nanos();
        let source = format!(
            "{}\n// concurrent-index-and-provide-test-{unique_suffix}\n",
            std::fs::read_to_string(&source_path).expect("fixture source")
        );
        let snapshot = Arc::new(SyntaxTree::parse(&source));
        let include_paths = Arc::new(vec![fixtures_dir.display().to_string()]);
        let source = Arc::new(source);
        let provider = Arc::new(DefinitionProvider::new());
        let cursor = position_of(source.as_str(), "transform(data[id].position");
        let barrier = Arc::new(tokio::sync::Barrier::new(2));
        let before = super::super::compiler::ast_dump_counter();

        let index_task = {
            let provider = Arc::clone(&provider);
            let uri = uri.clone();
            let source = Arc::clone(&source);
            let include_paths = Arc::clone(&include_paths);
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                barrier.wait().await;
                provider
                    .index_document(&uri, source.as_str(), include_paths.as_slice())
                    .await;
            })
        };

        let provide_task = {
            let provider = Arc::clone(&provider);
            let uri = uri.clone();
            let source = Arc::clone(&source);
            let include_paths = Arc::clone(&include_paths);
            let snapshot = Arc::clone(&snapshot);
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                barrier.wait().await;
                provider
                    .provide(
                        &uri,
                        cursor,
                        source.as_str(),
                        include_paths.as_slice(),
                        snapshot.as_ref(),
                    )
                    .await
            })
        };

        let _ = index_task.await.expect("index task should not panic");
        let provide_result = provide_task.await.expect("provide task should not panic");
        assert!(
            provide_result.is_some(),
            "navigation request should still resolve while concurrent indexing runs"
        );

        let after = super::super::compiler::ast_dump_counter();
        assert_eq!(
            after - before,
            1,
            "index_document and provide should share one AST dump build for same source hash"
        );
    }

    #[test]
    fn matches_position_accepts_cursor_at_token_end_boundary() {
        assert!(matches_position(
            "/tmp/member_method_tie.metal",
            71,
            20,
            10,
            "/tmp/member_method_tie.metal",
            71,
            30,
        ));
    }

    #[test]
    fn matches_position_rejects_cursor_past_token_end_boundary() {
        assert!(!matches_position(
            "/tmp/member_method_tie.metal",
            71,
            20,
            10,
            "/tmp/member_method_tie.metal",
            71,
            31,
        ));
    }
