    use super::*;
    use std::path::PathBuf;

    #[test]
    fn ast_dump_xcrun_command_is_kill_on_drop() {
        let command = xcrun_command(&[]);
        assert!(
            command.get_kill_on_drop(),
            "AST dump command must kill subprocess when request future is dropped"
        );
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("valid clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "metal-analyzer-rewrite-includes-{name}-{}-{nonce}",
            std::process::id(),
        ))
    }

    #[test]
    fn rewrite_includes_rewrites_existing_local_quote_include() {
        let temp_dir = unique_temp_dir("local");
        std::fs::create_dir_all(&temp_dir).expect("temp dir");
        let local_header = temp_dir.join("local.h");
        std::fs::write(&local_header, "// header\n").expect("write local header");

        let source = "#include \"local.h\"\n";
        let rewritten = rewrite_includes(source, &temp_dir);

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
        let rewritten = rewrite_includes(source, &temp_dir);
        assert_eq!(
            rewritten, source,
            "missing local include must stay relative so include paths can resolve it"
        );

        let _ = std::fs::remove_dir(temp_dir);
    }
