use std::{
    fmt::{Display, Formatter},
    process::Stdio,
};

use tokio::{io::AsyncWriteExt, process::Command};
use tower_lsp::lsp_types::{FormattingOptions, Position, Range, TextEdit};

use crate::{document::Document, server::settings::FormattingSettings};

pub(crate) async fn format_document(
    document: &Document,
    _options: &FormattingOptions,
    formatting_settings: &FormattingSettings,
) -> Result<Option<TextEdit>, FormattingError> {
    let assume_filename =
        document.uri.to_file_path().ok().map(|p| p.display().to_string()).unwrap_or_else(|| "shader.metal".to_string());

    let args = clang_format_args(&formatting_settings.args, assume_filename);

    let formatted = match run_clang_format(&formatting_settings.command, &args, &document.text).await {
        Ok(output) => output,
        Err(FormattingError::CommandNotFound(_)) if formatting_settings.command == "clang-format" => {
            let mut xcrun_args = vec!["clang-format".to_string()];
            xcrun_args.extend(args);
            run_clang_format("xcrun", &xcrun_args, &document.text).await?
        },
        Err(error) => return Err(error),
    };

    if formatted == document.text {
        return Ok(None);
    }

    Ok(Some(TextEdit {
        range: full_document_range(document),
        new_text: formatted,
    }))
}

fn full_document_range(document: &Document) -> Range {
    let end = document.position_of(document.text.len());
    Range {
        start: Position::new(0, 0),
        end,
    }
}

fn clang_format_args(
    extra_args: &[String],
    assume_filename: String,
) -> Vec<String> {
    let mut args = extra_args.to_vec();
    args.extend([
        "--assume-filename".to_string(),
        assume_filename,
        "--style".to_string(),
        "file".to_string(),
        "--fallback-style".to_string(),
        "none".to_string(),
    ]);
    args
}

async fn run_clang_format(
    executable: &str,
    args: &[String],
    input: &str,
) -> Result<String, FormattingError> {
    let mut child = Command::new(executable)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => FormattingError::CommandNotFound(executable.to_string()),
            _ => FormattingError::LaunchFailed {
                command: executable.to_string(),
                reason: error.to_string(),
            },
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input.as_bytes()).await.map_err(|error| FormattingError::LaunchFailed {
            command: executable.to_string(),
            reason: format!("failed to stream source to formatter: {error}"),
        })?;
    }

    let output = child.wait_with_output().await.map_err(|error| FormattingError::LaunchFailed {
        command: executable.to_string(),
        reason: error.to_string(),
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(FormattingError::FormattingFailed {
            command: executable.to_string(),
            reason: if stderr.is_empty() {
                format!("process exited with status {}", output.status)
            } else {
                stderr
            },
        });
    }

    String::from_utf8(output.stdout).map_err(|error| FormattingError::FormattingFailed {
        command: executable.to_string(),
        reason: format!("formatter produced invalid UTF-8 output: {error}"),
    })
}

#[derive(Debug)]
pub(crate) enum FormattingError {
    CommandNotFound(String),
    LaunchFailed {
        command: String,
        reason: String,
    },
    FormattingFailed {
        command: String,
        reason: String,
    },
}

impl Display for FormattingError {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            Self::CommandNotFound(command) => write!(f, "{command} is not available"),
            Self::LaunchFailed {
                command,
                reason,
            } => {
                write!(f, "failed to launch {command}: {reason}")
            },
            Self::FormattingFailed {
                command,
                reason,
            } => {
                write!(f, "{command} failed: {reason}")
            },
        }
    }
}

impl std::error::Error for FormattingError {}

#[cfg(test)]
#[path = "../../tests/src/server/formatting_tests.rs"]
mod tests;
