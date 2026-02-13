use std::{
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

use tower_lsp::lsp_types::Url;
use tracing::{debug, warn};

static NEXT_AST_DUMP_ID: AtomicU64 = AtomicU64::new(1);

#[cfg(test)]
pub(crate) fn ast_dump_counter() -> u64 {
    NEXT_AST_DUMP_ID.load(Ordering::Relaxed)
}

fn xcrun_command(args: &[String]) -> Command {
    let mut command = Command::new("xcrun");
    command.args(args);
    command
}

pub(crate) fn run_ast_dump(
    source: &str,
    uri: &Url,
    include_paths: &[String],
) -> Option<(String, Vec<String>)> {
    let tmp_dir = std::env::temp_dir().join(format!("metal-analyzer-def-{}", std::process::id()));
    if std::fs::create_dir_all(&tmp_dir).is_err() {
        warn!("Failed to create temp dir for AST dump");
        return None;
    }

    let compilation_id = NEXT_AST_DUMP_ID.fetch_add(1, Ordering::Relaxed);
    let src_file = tmp_dir.join(format!("shader-{compilation_id}.metal"));

    let content = if let Ok(original_path) = uri.to_file_path() {
        if let Some(parent) = original_path.parent() {
            rewrite_includes(source, parent)
        } else {
            source.to_string()
        }
    } else {
        source.to_string()
    };

    if std::fs::write(&src_file, content).is_err() {
        warn!("Failed to write temp file for AST dump");
        let _ = std::fs::remove_dir(&tmp_dir);
        return None;
    }

    let mut args = vec![
        "metal".to_string(),
        "-Xclang".to_string(),
        "-ast-dump=json".to_string(),
        "-fsyntax-only".to_string(),
        "-fno-color-diagnostics".to_string(),
        src_file.display().to_string(),
    ];

    let mut seen_includes = std::collections::HashSet::with_capacity(include_paths.len() + 16);

    for path in include_paths {
        if seen_includes.insert(path.clone()) {
            args.push("-I".to_string());
            args.push(path.clone());
        }
    }

    if let Ok(file_path) = uri.to_file_path() {
        let mut directory = file_path.parent();
        while let Some(dir) = directory {
            let path_string = dir.display().to_string();
            if seen_includes.insert(path_string.clone()) {
                args.push("-I".to_string());
                args.push(path_string);
            }
            directory = dir.parent();
            if directory.is_none_or(|parent| parent == dir) {
                break;
            }
        }
    }

    debug!("AST dump: xcrun {}", args.join(" "));

    let mut command = xcrun_command(&args);
    let output = match command.output() {
        Ok(output) => output,
        Err(error) => {
            warn!("Failed to run AST dump: {error}");
            return None;
        },
    };

    let raw_tmp_file = src_file.display().to_string();
    let canonical_tmp_file = std::fs::canonicalize(&src_file).ok().map(|path| path.display().to_string());
    let mut tmp_files = vec![raw_tmp_file];
    if let Some(canonical) = canonical_tmp_file
        && !tmp_files.contains(&canonical)
    {
        tmp_files.push(canonical);
    }

    let _ = std::fs::remove_file(&src_file);
    let _ = std::fs::remove_dir(&tmp_dir);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        for line in stderr.lines() {
            if line.contains("error:") {
                warn!("[ast-dump] compiler error: {line}");
            }
        }
        debug!("[ast-dump] exited with non-zero status (partial AST may still be usable)");
    }

    let stdout = String::from_utf8(output.stdout).ok()?;
    if stdout.is_empty() || !stdout.starts_with('{') {
        warn!("[ast-dump] produced no usable JSON for {uri}");
        return None;
    }

    debug!("[ast-dump] produced {} bytes of JSON for {uri}", stdout.len());

    Some((stdout, tmp_files))
}

fn rewrite_includes(
    source: &str,
    base_dir: &std::path::Path,
) -> String {
    let mut output = String::with_capacity(source.len());
    for line in source.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("#include")
            && let Some(start) = line.find('"')
            && let Some(end) = line[start + 1..].find('"')
        {
            let rel_path = &line[start + 1..start + 1 + end];
            if !std::path::Path::new(rel_path).is_absolute() {
                let abs_path = base_dir.join(rel_path);
                if abs_path.exists() {
                    output.push_str(&line[..start + 1]);
                    output.push_str(&abs_path.display().to_string());
                    output.push_str(&line[start + 1 + end..]);
                    output.push('\n');
                    continue;
                }
            }
        }
        output.push_str(line);
        output.push('\n');
    }
    output
}

#[cfg(test)]
#[path = "../../tests/src/definition/compiler_tests.rs"]
mod tests;
