    use super::*;

    #[test]
    fn diagnostics_generation_drops_stale_results() {
        let generations = DashMap::new();
        let uri = Url::parse("file:///tmp/shader.metal").expect("valid url");

        let first = next_diagnostic_generation(&generations, &uri);
        assert_eq!(first, 1);
        assert!(is_latest_diagnostic_generation(&generations, &uri, first));

        let second = next_diagnostic_generation(&generations, &uri);
        assert_eq!(second, 2);
        assert!(!is_latest_diagnostic_generation(&generations, &uri, first));
        assert!(is_latest_diagnostic_generation(&generations, &uri, second));
    }

    #[test]
    fn diagnostics_generation_is_per_file() {
        let generations = DashMap::new();
        let a = Url::parse("file:///tmp/a.metal").expect("valid url");
        let b = Url::parse("file:///tmp/b.metal").expect("valid url");

        let a1 = next_diagnostic_generation(&generations, &a);
        let b1 = next_diagnostic_generation(&generations, &b);
        assert_eq!(a1, 1);
        assert_eq!(b1, 1);

        let a2 = next_diagnostic_generation(&generations, &a);
        assert_eq!(a2, 2);
        assert!(is_latest_diagnostic_generation(&generations, &a, a2));
        assert!(is_latest_diagnostic_generation(&generations, &b, b1));
    }

    #[test]
    fn filter_target_diagnostics_drops_non_target_in_strict_mode() {
        let target = std::path::Path::new("/tmp/header.h");
        let diagnostics = vec![
            MetalDiagnostic {
                file: Some("/tmp/header.h".to_string()),
                line: 1,
                column: 1,
                severity: DiagnosticSeverity::ERROR,
                message: "header error".to_string(),
            },
            MetalDiagnostic {
                file: Some("/tmp/owner.metal".to_string()),
                line: 2,
                column: 1,
                severity: DiagnosticSeverity::ERROR,
                message: "owner error".to_string(),
            },
        ];

        let filtered = filter_target_diagnostics(diagnostics, Some(target), true);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].message, "header error");
    }

    #[test]
    fn filter_target_diagnostics_keeps_unknown_file_in_non_strict_mode() {
        let target = std::path::Path::new("/tmp/file.metal");
        let diagnostics = vec![MetalDiagnostic {
            file: None,
            line: 0,
            column: 0,
            severity: DiagnosticSeverity::ERROR,
            message: "compiler failed".to_string(),
        }];

        let filtered = filter_target_diagnostics(diagnostics, Some(target), false);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].message, "compiler failed");
    }

    #[test]
    fn filter_target_diagnostics_drops_ambiguous_relative_header_path() {
        let target = std::path::Path::new("/tmp/project/include/utils.h");
        let diagnostics = vec![MetalDiagnostic {
            file: Some("utils.h".to_string()),
            line: 1,
            column: 1,
            severity: DiagnosticSeverity::ERROR,
            message: "unknown type name 'METAL_FUNC'".to_string(),
        }];

        let filtered = filter_target_diagnostics(diagnostics, Some(target), true);
        assert!(
            filtered.is_empty(),
            "relative basename-only file paths must not be attributed to an arbitrary header",
        );
    }

    #[test]
    fn diagnostic_paths_match_accepts_equivalent_absolute_paths() {
        assert!(diagnostic_paths_match(
            "/tmp/project/dir/../include/utils.h",
            "/tmp/project/include/utils.h",
        ));
    }

    #[test]
    fn filter_attaches_cross_file_note_as_related_info() {
        let target = std::path::Path::new("/tmp/gemv.metal");
        let diagnostics = vec![
            MetalDiagnostic {
                file: Some("/tmp/gemv.metal".to_string()),
                line: 13,
                column: 8,
                severity: DiagnosticSeverity::WARNING,
                message: "warning from primary file".to_string(),
            },
            MetalDiagnostic {
                file: Some("/tmp/defines.h".to_string()),
                line: 3,
                column: 8,
                severity: DiagnosticSeverity::INFORMATION,
                message: "related note".to_string(),
            },
        ];

        let filtered = filter_target_diagnostics(diagnostics, Some(target), false);
        assert_eq!(filtered.len(), 1, "only the primary warning should appear");
        assert_eq!(filtered[0].message, "warning from primary file");

        let related = filtered[0]
            .related_information
            .as_ref()
            .expect("note should be attached as related_information");
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].message, "related note");
        assert!(
            related[0].location.uri.path().ends_with("defines.h"),
            "related info should point to the note's file"
        );
    }

    #[test]
    fn filter_suppresses_macro_redefinition_warning_and_following_note() {
        let target = std::path::Path::new("/tmp/gemv.metal");
        let diagnostics = vec![
            MetalDiagnostic {
                file: Some("/tmp/gemv.metal".to_string()),
                line: 13,
                column: 8,
                severity: DiagnosticSeverity::WARNING,
                message: "'MTL_CONST' macro redefined [-Wmacro-redefined]".to_string(),
            },
            MetalDiagnostic {
                file: Some("/tmp/defines.h".to_string()),
                line: 3,
                column: 8,
                severity: DiagnosticSeverity::INFORMATION,
                message: "previous definition is here".to_string(),
            },
        ];

        let filtered = filter_target_diagnostics(diagnostics, Some(target), false);
        assert!(
            filtered.is_empty(),
            "macro redefinition warning and trailing note should be suppressed"
        );
    }

    #[test]
    fn filter_drops_orphan_note_without_primary() {
        let target = std::path::Path::new("/tmp/shader.metal");
        let diagnostics = vec![MetalDiagnostic {
            file: Some("/tmp/other.h".to_string()),
            line: 5,
            column: 1,
            severity: DiagnosticSeverity::INFORMATION,
            message: "expanded from macro".to_string(),
        }];

        let filtered = filter_target_diagnostics(diagnostics, Some(target), false);
        assert!(
            filtered.is_empty(),
            "orphan note with no preceding primary should not appear"
        );
    }

    #[test]
    fn filter_keeps_primary_when_note_has_relative_path() {
        let target = std::path::Path::new("/tmp/shader.metal");
        let diagnostics = vec![
            MetalDiagnostic {
                file: Some("/tmp/shader.metal".to_string()),
                line: 10,
                column: 1,
                severity: DiagnosticSeverity::WARNING,
                message: "some warning".to_string(),
            },
            MetalDiagnostic {
                file: Some("relative.h".to_string()),
                line: 1,
                column: 1,
                severity: DiagnosticSeverity::INFORMATION,
                message: "note about it".to_string(),
            },
        ];

        let filtered = filter_target_diagnostics(diagnostics, Some(target), false);
        assert_eq!(filtered.len(), 1, "warning should be kept");
        assert_eq!(filtered[0].message, "some warning");
        // Relative path cannot be converted to a file:// URI, so no related info.
        assert!(
            filtered[0].related_information.is_none(),
            "relative note path should be silently skipped"
        );
    }

    #[test]
    fn build_workspace_scan_exclude_prefixes_expands_relative_paths_per_workspace_root() {
        let workspace_roots = vec![PathBuf::from("/tmp/ws-a"), PathBuf::from("/tmp/ws-b")];
        let exclude_paths = vec![
            "external/vendor-shaders".to_string(),
            "/opt/generated".to_string(),
        ];

        let prefixes = build_workspace_scan_exclude_prefixes(&workspace_roots, &exclude_paths);
        assert!(
            prefixes.contains(&PathBuf::from("/tmp/ws-a/external/vendor-shaders")),
            "relative excludes should expand under the first workspace root"
        );
        assert!(
            prefixes.contains(&PathBuf::from("/tmp/ws-b/external/vendor-shaders")),
            "relative excludes should expand under the second workspace root"
        );
        assert!(
            prefixes.contains(&PathBuf::from("/opt/generated")),
            "absolute excludes should be kept as absolute prefixes"
        );
    }

    #[test]
    fn is_path_excluded_matches_prefix_descendants() {
        let excluded = vec![PathBuf::from("/tmp/ws/external/vendor-shaders")];
        assert!(is_path_excluded(
            Path::new("/tmp/ws/external/vendor-shaders/shaders/kernel.metal"),
            &excluded
        ));
        assert!(!is_path_excluded(
            Path::new("/tmp/ws/crates/app/kernel.metal"),
            &excluded
        ));
    }
