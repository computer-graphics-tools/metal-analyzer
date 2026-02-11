    use tower_lsp::lsp_types::Url;

    use super::*;

    #[test]
    fn clang_format_args_enforces_project_config_file() {
        let args = clang_format_args(&Vec::new(), "/tmp/shader.metal".to_string());
        assert_eq!(
            args,
            vec![
                "--assume-filename".to_string(),
                "/tmp/shader.metal".to_string(),
                "--style".to_string(),
                "file".to_string(),
                "--fallback-style".to_string(),
                "none".to_string(),
            ]
        );
    }

    #[test]
    fn clang_format_args_preserves_custom_arguments() {
        let args = clang_format_args(
            &vec!["--sort-includes=false".to_string()],
            "/tmp/shader.metal".to_string(),
        );
        assert_eq!(
            args[0],
            "--sort-includes=false".to_string(),
            "custom args should be preserved as prefix"
        );
    }

    #[test]
    fn full_document_range_handles_multiline_text() {
        let uri = Url::parse("file:///tmp/shader.metal").expect("valid uri");
        let document = Document::new(uri, "line1\nline2\n".to_string(), 1);
        let range = full_document_range(&document);
        assert_eq!(range.start, Position::new(0, 0));
        assert_eq!(range.end, Position::new(2, 0));
    }
