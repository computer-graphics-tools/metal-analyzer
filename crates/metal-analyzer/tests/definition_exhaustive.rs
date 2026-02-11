//! Exhaustive go-to-definition correctness tests.
//!
//! For every identifier in a Metal source file, this test:
//! 1. Runs go-to-definition via the DefinitionProvider
//! 2. If a location is returned, reads the target line and asserts it contains
//!    the symbol name (catches false positives like jumping to the wrong overload)
//!
//! Run against the built-in fixtures:
//!   cargo test --test definition_exhaustive
//!
//! Run against an external shader directory (e.g. a large corpus):
//!   METAL_TEST_DIR=/path/to/shaders cargo test --test definition_exhaustive -- --nocapture

use std::path::{Path, PathBuf};

use metal_analyzer::DefinitionProvider;
use metal_analyzer::syntax::SyntaxTree;
use tower_lsp::lsp_types::*;

#[derive(Default, Clone, Debug)]
struct FailureCategories {
    missing_file: usize,
    unreadable: usize,
    out_of_range: usize,
    wrong_target: usize,
}

#[derive(Default, Clone, Debug)]
struct FileValidation {
    total: usize,
    passed: usize,
    macro_total: usize,
    macro_passed: usize,
    categories: FailureCategories,
    failures: Vec<String>,
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn has_metal_compiler() -> bool {
    std::process::Command::new("xcrun")
        .args(["--find", "metal"])
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Extract all (line, col, word) triples for identifier tokens in the source.
fn extract_identifiers(source: &str) -> Vec<(u32, u32, String)> {
    let mut results = Vec::new();
    for (line_idx, line) in source.lines().enumerate() {
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i].is_alphabetic() || chars[i] == '_' {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                // Skip very short identifiers and pure numbers-after-prefix
                if word.len() >= 2 {
                    results.push((line_idx as u32, start as u32, word));
                }
            } else {
                i += 1;
            }
        }
    }
    results
}

/// Validate a go-to-def result: if a location is returned, the target file
/// should exist and the target line should contain the symbol name.
///
/// Returns file-level validation summary with category counts.
async fn validate_file(
    path: &Path,
    include_paths: &[String],
) -> FileValidation {
    let source = std::fs::read_to_string(path).unwrap();
    let uri = Url::from_file_path(path).unwrap();
    let snapshot = SyntaxTree::parse(&source);
    let provider = DefinitionProvider::new();

    // Pre-index to populate the AST cache.
    provider
        .index_document(&uri, &source, include_paths)
        .await;

    let identifiers = extract_identifiers(&source);
    let mut summary = FileValidation {
        total: identifiers.len(),
        ..FileValidation::default()
    };

    for (line, col, word) in &identifiers {
        let source_line = source.lines().nth(*line as usize).unwrap_or_default();
        let is_macro_ctx = is_macro_context(source_line, word);
        if is_macro_ctx {
            summary.macro_total += 1;
        }

        let position = Position {
            line: *line,
            character: *col,
        };

        let result = provider
            .provide(&uri, position, &source, include_paths, &snapshot)
            .await;

        match result {
            None => {
                // No definition found -- acceptable for keywords, builtins, etc.
                summary.passed += 1;
                if is_macro_ctx {
                    summary.macro_passed += 1;
                }
            }
            Some(resp) => {
                let loc = match &resp {
                    GotoDefinitionResponse::Scalar(l) => l.clone(),
                    GotoDefinitionResponse::Array(locs) => {
                        if let Some(l) = locs.first() {
                            l.clone()
                        } else {
                            summary.passed += 1;
                            if is_macro_ctx {
                                summary.macro_passed += 1;
                            }
                            continue;
                        }
                    }
                    GotoDefinitionResponse::Link(links) => {
                        if let Some(l) = links.first() {
                            Location {
                                uri: l.target_uri.clone(),
                                range: l.target_selection_range,
                            }
                        } else {
                            summary.passed += 1;
                            if is_macro_ctx {
                                summary.macro_passed += 1;
                            }
                            continue;
                        }
                    }
                };

                // Read the target file and check the target line.
                let target_path = loc.uri.to_file_path().unwrap_or_default();
                let target_line_idx = loc.range.start.line as usize;

                if !target_path.exists() {
                    summary.categories.missing_file += 1;
                    summary.failures.push(format!(
                        "  {word} at {}:{} → target file does not exist: {}",
                        line + 1,
                        col + 1,
                        target_path.display()
                    ));
                    continue;
                }

                let target_source = match std::fs::read_to_string(&target_path) {
                    Ok(s) => s,
                    Err(_) => {
                        summary.categories.unreadable += 1;
                        summary.failures.push(format!(
                            "  {word} at {}:{} → cannot read target: {}",
                            line + 1,
                            col + 1,
                            target_path.display()
                        ));
                        continue;
                    }
                };

                let target_lines: Vec<&str> = target_source.lines().collect();
                if target_line_idx >= target_lines.len() {
                    summary.categories.out_of_range += 1;
                    summary.failures.push(format!(
                        "  {word} at {}:{} → target line {} out of range (file has {} lines): {}",
                        line + 1,
                        col + 1,
                        target_line_idx + 1,
                        target_lines.len(),
                        target_path.display()
                    ));
                    continue;
                }

                let target_line = target_lines[target_line_idx];

                // Include-directive resolution jumps to the file at line 0.
                // Also, some definitions use #define which contains the word
                // in the macro body, or the word might be on the same line
                // as part of a larger expression. Accept as long as the word
                // appears SOMEWHERE near the target line (±2 lines).
                let near_lines: Vec<&str> = target_lines
                    .get(target_line_idx.saturating_sub(1)..=(target_line_idx + 1).min(target_lines.len() - 1))
                    .unwrap_or_default()
                    .to_vec();
                let found_near = near_lines.iter().any(|l| l.contains(word.as_str()));

                // Also accept if the target file name contains the word
                // (include-directive resolution).
                let target_name = target_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy();
                let file_name_match = target_name.contains(word.as_str());

                // Accept default range (0:0) as file-level resolution.
                let is_file_level = loc.range.start.line == 0 && loc.range.start.character == 0;

                if found_near || file_name_match || is_file_level {
                    summary.passed += 1;
                    if is_macro_ctx {
                        summary.macro_passed += 1;
                    }
                } else {
                    summary.categories.wrong_target += 1;
                    summary.failures.push(format!(
                        "  {word} at {}:{} → {target_name}:{} does not contain '{word}': \"{}\"",
                        line + 1,
                        col + 1,
                        target_line_idx + 1,
                        target_line.trim(),
                    ));
                }
            }
        }
    }

    summary
}

fn collect_metal_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if dir.is_file() && dir.extension().is_some_and(|e| e == "metal") {
        files.push(dir.to_path_buf());
    } else if dir.is_dir() {
        for entry in walkdir::WalkDir::new(dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let p = entry.path();
            if p.extension().is_some_and(|e| e == "metal" || e == "h") {
                files.push(p.to_path_buf());
            }
        }
    }
    files
}

fn include_paths_for(file: &Path) -> Vec<String> {
    let mut paths = Vec::new();
    let mut dir = file.parent();
    while let Some(d) = dir {
        paths.push(d.display().to_string());
        dir = d.parent();
        if dir.is_none_or(|p| p == d) {
            break;
        }
    }
    paths
}

fn is_macro_context(line: &str, word: &str) -> bool {
    line.trim_start().starts_with("#")
        || word.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn exhaustive_fixtures() {
    if !has_metal_compiler() {
        return;
    }

    let dir = fixtures_dir();
    let files = collect_metal_files(&dir);
    assert!(!files.is_empty(), "no fixture files found");

    let mut total_failures = Vec::new();
    let mut grand_total = 0;
    let mut grand_passed = 0;
    let mut grand_macro_total = 0;
    let mut grand_macro_passed = 0;
    let mut grand_categories = FailureCategories::default();

    for file in &files {
        if !file.extension().is_some_and(|e| e == "metal") {
            continue;
        }
        let includes = include_paths_for(file);
        let summary = validate_file(file, &includes).await;
        grand_total += summary.total;
        grand_passed += summary.passed;
        grand_macro_total += summary.macro_total;
        grand_macro_passed += summary.macro_passed;
        grand_categories.missing_file += summary.categories.missing_file;
        grand_categories.unreadable += summary.categories.unreadable;
        grand_categories.out_of_range += summary.categories.out_of_range;
        grand_categories.wrong_target += summary.categories.wrong_target;

        let name = file.file_name().unwrap_or_default().to_string_lossy();
        if summary.failures.is_empty() {
            eprintln!(
                "{name}: {}/{} identifiers OK (macro {}/{})",
                summary.passed, summary.total, summary.macro_passed, summary.macro_total
            );
        } else {
            eprintln!(
                "{name}: {}/{} OK, {} FAILED (macro {}/{}):",
                summary.passed,
                summary.total,
                summary.failures.len(),
                summary.macro_passed,
                summary.macro_total
            );
            eprintln!(
                "  categories: missing_file={}, unreadable={}, out_of_range={}, wrong_target={}",
                summary.categories.missing_file,
                summary.categories.unreadable,
                summary.categories.out_of_range,
                summary.categories.wrong_target
            );
            for f in &summary.failures {
                eprintln!("{f}");
            }
            total_failures.extend(summary.failures);
        }
    }

    eprintln!(
        "\nTotal: {grand_passed}/{grand_total} identifiers OK, {} failures",
        total_failures.len()
    );
    eprintln!(
        "Macro-context: {grand_macro_passed}/{grand_macro_total} identifiers OK"
    );
    eprintln!(
        "Failure categories: missing_file={}, unreadable={}, out_of_range={}, wrong_target={}",
        grand_categories.missing_file,
        grand_categories.unreadable,
        grand_categories.out_of_range,
        grand_categories.wrong_target
    );

    assert!(
        total_failures.is_empty(),
        "{} false-positive definition(s) found:\n{}",
        total_failures.len(),
        total_failures.join("\n"),
    );
}

async fn run_external_audit(dir: &Path, label: &str) {
    let files = collect_metal_files(dir);
    if files.is_empty() {
        eprintln!("No .metal/.h files found for {label} at {}", dir.display());
        return;
    }

    eprintln!("Testing {} files for {label} from {}", files.len(), dir.display());

    let mut total_failures = Vec::new();
    let mut grand_total = 0;
    let mut grand_passed = 0;
    let mut grand_macro_total = 0;
    let mut grand_macro_passed = 0;
    let mut grand_categories = FailureCategories::default();

    for file in &files {
        if !file.extension().is_some_and(|e| e == "metal") {
            continue;
        }
        let includes = include_paths_for(file);
        let summary = validate_file(file, &includes).await;
        grand_total += summary.total;
        grand_passed += summary.passed;
        grand_macro_total += summary.macro_total;
        grand_macro_passed += summary.macro_passed;
        grand_categories.missing_file += summary.categories.missing_file;
        grand_categories.unreadable += summary.categories.unreadable;
        grand_categories.out_of_range += summary.categories.out_of_range;
        grand_categories.wrong_target += summary.categories.wrong_target;

        let name = file.strip_prefix(dir).unwrap_or(file).display();
        if summary.failures.is_empty() {
            eprintln!(
                "{name}: {}/{} OK (macro {}/{})",
                summary.passed, summary.total, summary.macro_passed, summary.macro_total
            );
        } else {
            eprintln!(
                "{name}: {}/{} OK, {} FAILED (macro {}/{}):",
                summary.passed,
                summary.total,
                summary.failures.len(),
                summary.macro_passed,
                summary.macro_total
            );
            eprintln!(
                "  categories: missing_file={}, unreadable={}, out_of_range={}, wrong_target={}",
                summary.categories.missing_file,
                summary.categories.unreadable,
                summary.categories.out_of_range,
                summary.categories.wrong_target
            );
            for f in &summary.failures {
                eprintln!("{f}");
            }
            total_failures.extend(summary.failures);
        }
    }

    eprintln!(
        "\nTotal: {grand_passed}/{grand_total} identifiers OK, {} failures",
        total_failures.len()
    );
    eprintln!(
        "Macro-context: {grand_macro_passed}/{grand_macro_total} identifiers OK"
    );
    eprintln!(
        "Failure categories: missing_file={}, unreadable={}, out_of_range={}, wrong_target={}",
        grand_categories.missing_file,
        grand_categories.unreadable,
        grand_categories.out_of_range,
        grand_categories.wrong_target
    );

    // Non-blocking audit: report failures without failing the test.
    if !total_failures.is_empty() {
        eprintln!(
            "\n{} false-positive definition(s) in {label} files (non-fatal).",
            total_failures.len()
        );
    }
}

/// Run against an external shader directory specified by METAL_TEST_DIR.
/// Skipped if the env var is not set.
#[tokio::test]
async fn exhaustive_external() {
    if !has_metal_compiler() {
        return;
    }

    let dir = match std::env::var("METAL_TEST_DIR") {
        Ok(d) => PathBuf::from(d),
        Err(_) => {
            eprintln!("METAL_TEST_DIR not set, skipping external test");
            return;
        }
    };

    run_external_audit(&dir, "METAL_TEST_DIR").await;
}

/// Auto-detect the repository's external kernel corpus and run the same
/// non-blocking exhaustive audit when present.
#[tokio::test]
#[ignore = "developer-only audit against external kernel corpus"]
async fn exhaustive_external_kernel_corpus_if_present() {
    if !has_metal_compiler() {
        return;
    }

    let external_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../external");
    let dir = std::fs::read_dir(&external_root)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .flat_map(|project_root| {
            let crates_dir = project_root.join("crates");
            std::fs::read_dir(crates_dir)
                .ok()
                .into_iter()
                .flatten()
                .filter_map(|entry| entry.ok().map(|e| e.path()))
                .collect::<Vec<_>>()
        })
        .map(|crate_root| crate_root.join("src/backends/metal/kernel"))
        .find(|candidate| candidate.exists());

    let Some(dir) = dir else {
        eprintln!(
            "external kernel corpus directory not found under {}, skipping",
            external_root.display()
        );
        return;
    };

    run_external_audit(&dir, "external-kernel-corpus").await;
}
