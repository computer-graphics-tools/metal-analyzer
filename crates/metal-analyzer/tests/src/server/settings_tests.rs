use serde_json::json;

use super::*;

#[test]
fn parses_namespaced_payload() {
    let payload = json!({
        "metal-analyzer": {
            "formatting": {
                "enable": false,
                "command": "xcrun",
                "args": ["clang-format"]
            },
            "diagnostics": {
                "debounceMs": 1200,
                "scope": "workspace"
            },
            "indexing": {
                "concurrency": 4,
                "maxFileSizeKb": 256,
                "excludePaths": ["external/vendor-shaders", " /tmp/generated "]
            },
            "compiler": {
                "includePaths": ["/tmp/includes"],
                "extraFlags": ["-DMETAL"],
                "platform": "ios"
            },
            "logging": {
                "level": "debug"
            }
        }
    });

    let settings = ServerSettings::from_lsp_payload(Some(&payload));
    assert!(!settings.formatting.enable);
    assert_eq!(settings.formatting.command, "xcrun");
    assert_eq!(settings.formatting.args, vec!["clang-format"]);
    assert_eq!(settings.diagnostics.debounce_ms, 1200);
    assert_eq!(settings.diagnostics.scope, DiagnosticsScope::Workspace);
    assert_eq!(settings.indexing.concurrency, 4);
    assert_eq!(settings.indexing.max_file_size_kb, 256);
    assert_eq!(
        settings.indexing.exclude_paths,
        vec!["external/vendor-shaders".to_string(), "/tmp/generated".to_string(),]
    );
    assert_eq!(settings.compiler.include_paths, vec!["/tmp/includes"]);
    assert_eq!(settings.compiler.extra_flags, vec!["-DMETAL"]);
    assert_eq!(settings.compiler.platform, CompilerPlatform::Ios);
    assert_eq!(settings.logging.level, LogLevel::Debug);
}

#[test]
fn parses_direct_payload() {
    let payload = json!({
        "diagnostics": {
            "onType": false,
            "onSave": true,
            "scope": "openFiles"
        },
        "indexing": {
            "enable": false
        }
    });

    let settings = ServerSettings::from_lsp_payload(Some(&payload));
    assert!(!settings.diagnostics.on_type);
    assert!(settings.diagnostics.on_save);
    assert_eq!(settings.diagnostics.scope, DiagnosticsScope::OpenFiles);
    assert!(!settings.indexing.enable);
}

#[test]
fn clamps_numeric_values() {
    let payload = json!({
        "diagnostics": { "debounceMs": 1 },
        "indexing": { "concurrency": 0, "maxFileSizeKb": 1 }
    });

    let settings = ServerSettings::from_lsp_payload(Some(&payload));
    assert_eq!(settings.diagnostics.debounce_ms, MIN_DIAGNOSTIC_DEBOUNCE_MS);
    assert_eq!(settings.indexing.concurrency, MIN_INDEXING_CONCURRENCY);
    assert_eq!(settings.indexing.max_file_size_kb, MIN_MAX_FILE_SIZE_KB);
}

#[test]
fn preserves_existing_values_when_payload_is_partial() {
    let base = ServerSettings {
        formatting: FormattingSettings {
            command: "custom-format".to_string(),
            ..FormattingSettings::default()
        },
        ..ServerSettings::default()
    };
    let payload = json!({
        "diagnostics": {
            "debounceMs": 900
        }
    });

    let merged = base.merged_with_payload(&payload);
    assert_eq!(merged.formatting.command, "custom-format");
    assert_eq!(merged.diagnostics.debounce_ms, 900);
}

#[test]
fn diagnostics_scope_defaults_to_open_files() {
    let settings = ServerSettings::from_lsp_payload(None);
    assert_eq!(settings.diagnostics.scope, DiagnosticsScope::OpenFiles);
}

#[test]
fn compiler_platform_normalizes_case_and_whitespace() {
    let payload = json!({
        "compiler": {
            "platform": "  MaCoS  "
        }
    });

    let settings = ServerSettings::from_lsp_payload(Some(&payload));
    assert_eq!(settings.compiler.platform, CompilerPlatform::Macos);
}

#[test]
fn compiler_platform_falls_back_to_macos_for_invalid_values() {
    let payload = json!({
        "compiler": {
            "platform": "nonexistent"
        }
    });

    let settings = ServerSettings::from_lsp_payload(Some(&payload));
    assert_eq!(settings.compiler.platform, CompilerPlatform::Macos);
}

#[test]
fn indexing_exclude_paths_are_trimmed_and_deduplicated() {
    let payload = json!({
        "indexing": {
            "excludePaths": [
                "",
                "  external/vendor-shaders  ",
                "external/vendor-shaders",
                "/tmp/generated"
            ]
        }
    });

    let settings = ServerSettings::from_lsp_payload(Some(&payload));
    assert_eq!(
        settings.indexing.exclude_paths,
        vec!["external/vendor-shaders".to_string(), "/tmp/generated".to_string(),]
    );
}
