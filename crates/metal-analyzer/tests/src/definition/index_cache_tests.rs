    use std::collections::HashMap;

    use super::*;
    use crate::definition::ref_site::RefSite;
    use crate::definition::symbol_def::SymbolDef;

    #[tokio::test]
    async fn disk_cache_roundtrip_and_invalidation() {
        let root = std::env::temp_dir().join(format!(
            "metal-analyzer-index-cache-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock drift")
                .as_nanos()
        ));
        let file = root.join("shader.metal");
        let include_paths = vec!["/tmp/include".to_owned(), "/tmp/include2".to_owned()];
        let index = AstIndex {
            defs: vec![SymbolDef {
                id: "0x1".to_owned(),
                name: "foo".to_owned(),
                kind: "FunctionDecl".to_owned(),
                file: "/tmp/shader.metal".to_owned(),
                line: 1,
                col: 1,
                is_definition: true,
                type_name: None,
                qual_type: None,
            }],
            refs: vec![RefSite {
                file: "/tmp/shader.metal".to_owned(),
                line: 2,
                col: 3,
                tok_len: 3,
                target_id: "0x1".to_owned(),
                target_name: "foo".to_owned(),
                target_kind: "FunctionDecl".to_owned(),
                expansion: None,
                spelling: None,
            }],
            id_to_def: HashMap::from([("0x1".to_owned(), 0)]),
            name_to_defs: HashMap::from([("foo".to_owned(), vec![0])]),
            target_id_to_refs: HashMap::from([("0x1".to_owned(), vec![0])]),
            file_to_defs: HashMap::from([("/tmp/shader.metal".to_owned(), vec![0])]),
            file_to_refs: HashMap::from([("/tmp/shader.metal".to_owned(), vec![0])]),
        };

        save_to_root(&root, &file, "source-hash-1", &include_paths, &index).await;

        let loaded = load_from_root(&root, &file, "source-hash-1", &include_paths).await;
        assert!(loaded.is_some(), "cache should load for matching key");
        assert_eq!(loaded.expect("cache payload").defs.len(), 1);

        let stale_source = load_from_root(&root, &file, "source-hash-2", &include_paths).await;
        assert!(stale_source.is_none(), "cache must invalidate by source hash");

        let stale_include =
            load_from_root(&root, &file, "source-hash-1", &["/tmp/other".to_owned()]).await;
        assert!(
            stale_include.is_none(),
            "cache must invalidate by include path fingerprint"
        );

        let _ = tokio::fs::remove_dir_all(root).await;
    }
