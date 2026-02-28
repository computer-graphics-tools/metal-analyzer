#![allow(dead_code)]

use std::path::{Path, PathBuf};

use metal_analyzer::metal::compiler::compute_include_paths;
use tower_lsp::lsp_types::{Position, Url};

pub fn has_metal_compiler() -> bool {
    std::process::Command::new("xcrun").args(["--find", "metal"]).output().is_ok_and(|output| output.status.success())
}

pub fn fixture_cases_root() -> PathBuf {
    let fixtures_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    if let Ok(entries) = std::fs::read_dir(&fixtures_root) {
        for entry in entries.flatten() {
            let candidate = entry.path();
            if !candidate.is_dir() {
                continue;
            }
            if candidate.join("common/types.h").exists()
                && candidate.join("generated/matmul.h").exists()
                && candidate.join("matmul/gemv/shaders/gemv_like.metal").exists()
            {
                return candidate;
            }
        }
    }
    fixtures_root.join("cases")
}

pub fn fixture_path(relative_path: &str) -> PathBuf {
    fixture_cases_root().join(relative_path)
}

pub fn fixture_uri(relative_path: &str) -> Url {
    Url::from_file_path(fixture_path(relative_path)).expect("fixture path is valid file:// URI")
}

pub fn read_fixture(relative_path: &str) -> String {
    std::fs::read_to_string(fixture_path(relative_path)).expect("fixture must exist")
}

pub fn include_paths_for(relative_path: &str) -> Vec<String> {
    let file = fixture_path(relative_path);
    let root = fixture_cases_root();
    compute_include_paths(&file, Some(&[root]))
}

pub fn position_of(
    source: &str,
    needle: &str,
) -> Position {
    position_of_nth(source, needle, 0)
}

pub fn position_of_nth(
    source: &str,
    needle: &str,
    nth: usize,
) -> Position {
    assert!(!needle.is_empty(), "needle must not be empty");
    let mut from = 0usize;
    let mut current = 0usize;

    loop {
        let Some(idx) = source[from..].find(needle) else {
            panic!("needle not found: {needle}");
        };
        let absolute = from + idx;
        if current == nth {
            let before = &source[..absolute];
            let line = before.as_bytes().iter().filter(|&&b| b == b'\n').count() as u32;
            let col = before
                .rsplit_once('\n')
                .map(|(_, tail)| tail.chars().count() as u32)
                .unwrap_or_else(|| before.chars().count() as u32);
            return Position::new(line, col);
        }
        current += 1;
        from = absolute + needle.len();
    }
}

pub fn line_contains(
    path: &Path,
    needle: &str,
) -> bool {
    std::fs::read_to_string(path).ok().is_some_and(|src| src.lines().any(|line| line.contains(needle)))
}
