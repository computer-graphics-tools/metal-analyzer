use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use serde::Deserialize;

const METALFMT_FILENAME: &str = "metalfmt.toml";

/// Walks parent directories from `start` looking for `metalfmt.toml`.
/// Returns the path to the first one found, or `None`.
pub(crate) fn find_metalfmt_toml(start: &Path) -> Option<PathBuf> {
    let mut dir = if start.is_file() {
        start.parent()?
    } else {
        start
    };
    loop {
        let candidate = dir.join(METALFMT_FILENAME);
        if candidate.is_file() {
            return Some(candidate);
        }
        dir = dir.parent()?;
    }
}

/// Reads and parses a `metalfmt.toml` file, returning the clang-format
/// inline style string (the part inside `--style={...}`).
///
/// Returns `None` if the file cannot be read or parsed.
pub(crate) fn load_inline_style(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let config: MetalFmtConfig = toml::from_str(&content).ok()?;
    let style = config.to_inline_style();
    if style.is_empty() {
        None
    } else {
        Some(style)
    }
}

/// Attempts to resolve a `metalfmt.toml` for the given source file and
/// return the corresponding clang-format inline style string.
pub(crate) fn resolve_inline_style(source_path: &Path) -> Option<String> {
    let toml_path = find_metalfmt_toml(source_path)?;
    load_inline_style(&toml_path)
}

/// Strongly-typed keys from the `metalfmt.toml` documentation.
/// Unknown keys are captured by `extra` and passed through as-is,
/// making the format forward-compatible with any clang-format option.
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct MetalFmtConfig {
    // Base style
    based_on_style: Option<String>,

    // Indentation
    indent_width: Option<u32>,
    use_tab: Option<bool>,
    tab_width: Option<u32>,

    // Line length
    column_limit: Option<u32>,

    // Braces
    break_before_braces: Option<String>,
    brace_wrapping_after_function: Option<bool>,
    brace_wrapping_after_struct: Option<bool>,
    brace_wrapping_after_enum: Option<bool>,
    brace_wrapping_after_control_statement: Option<String>,

    // Spacing and alignment
    space_before_parens: Option<String>,
    pointer_alignment: Option<String>,
    reference_alignment: Option<String>,
    align_after_open_bracket: Option<String>,
    align_operands: Option<String>,
    align_trailing_comments: Option<bool>,

    // Includes
    sort_includes: Option<bool>,
    include_blocks: Option<String>,

    // Other
    allow_short_functions_on_a_single_line: Option<String>,
    allow_short_if_statements_on_a_single_line: Option<String>,
    allow_short_loops_on_a_single_line: Option<bool>,
    bin_pack_arguments: Option<bool>,
    bin_pack_parameters: Option<bool>,
    cpp_standard: Option<String>,
    max_empty_lines_to_keep: Option<u32>,

    // Forward-compatible: any key not listed above is passed through.
    #[serde(flatten)]
    extra: BTreeMap<String, toml::Value>,
}

impl MetalFmtConfig {
    /// Convert the config into a clang-format inline style string
    /// (the content inside `{...}`).
    fn to_inline_style(&self) -> String {
        let mut parts: Vec<String> = Vec::new();

        // Base style
        push_str(&mut parts, "BasedOnStyle", &self.based_on_style);

        // Indentation
        push_u32(&mut parts, "IndentWidth", &self.indent_width);
        if let Some(use_tab) = self.use_tab {
            parts.push(format!(
                "UseTab: {}",
                if use_tab {
                    "ForIndentation"
                } else {
                    "Never"
                }
            ));
        }
        push_u32(&mut parts, "TabWidth", &self.tab_width);

        // Line length
        push_u32(&mut parts, "ColumnLimit", &self.column_limit);

        // Braces
        push_str(&mut parts, "BreakBeforeBraces", &self.break_before_braces);
        push_bool(&mut parts, "BraceWrapping.AfterFunction", &self.brace_wrapping_after_function);
        push_bool(&mut parts, "BraceWrapping.AfterStruct", &self.brace_wrapping_after_struct);
        push_bool(&mut parts, "BraceWrapping.AfterEnum", &self.brace_wrapping_after_enum);
        push_str(&mut parts, "BraceWrapping.AfterControlStatement", &self.brace_wrapping_after_control_statement);

        // Spacing and alignment
        push_str(&mut parts, "SpaceBeforeParens", &self.space_before_parens);
        push_str(&mut parts, "PointerAlignment", &self.pointer_alignment);
        push_str(&mut parts, "ReferenceAlignment", &self.reference_alignment);
        push_str(&mut parts, "AlignAfterOpenBracket", &self.align_after_open_bracket);
        push_str(&mut parts, "AlignOperands", &self.align_operands);
        push_bool(&mut parts, "AlignTrailingComments", &self.align_trailing_comments);

        // Includes
        if let Some(v) = self.sort_includes {
            parts.push(format!(
                "SortIncludes: {}",
                if v {
                    "CaseSensitive"
                } else {
                    "Never"
                }
            ));
        }
        push_str(&mut parts, "IncludeBlocks", &self.include_blocks);

        // Other
        push_str(&mut parts, "AllowShortFunctionsOnASingleLine", &self.allow_short_functions_on_a_single_line);
        push_str(&mut parts, "AllowShortIfStatementsOnASingleLine", &self.allow_short_if_statements_on_a_single_line);
        push_bool(&mut parts, "AllowShortLoopsOnASingleLine", &self.allow_short_loops_on_a_single_line);
        push_bool(&mut parts, "BinPackArguments", &self.bin_pack_arguments);
        push_bool(&mut parts, "BinPackParameters", &self.bin_pack_parameters);
        push_str(&mut parts, "Standard", &self.cpp_standard);
        push_u32(&mut parts, "MaxEmptyLinesToKeep", &self.max_empty_lines_to_keep);

        // Extra keys: convert snake_case to PascalCase for clang-format
        for (key, value) in &self.extra {
            let pascal_key = snake_to_pascal(key);
            parts.push(format!("{pascal_key}: {}", toml_value_to_clang(value)));
        }

        parts.join(", ")
    }
}

fn push_str(
    parts: &mut Vec<String>,
    key: &str,
    val: &Option<String>,
) {
    if let Some(v) = val {
        parts.push(format!("{key}: {v}"));
    }
}

fn push_u32(
    parts: &mut Vec<String>,
    key: &str,
    val: &Option<u32>,
) {
    if let Some(v) = val {
        parts.push(format!("{key}: {v}"));
    }
}

fn push_bool(
    parts: &mut Vec<String>,
    key: &str,
    val: &Option<bool>,
) {
    if let Some(v) = val {
        parts.push(format!(
            "{key}: {}",
            if *v {
                "true"
            } else {
                "false"
            }
        ));
    }
}

/// Convert a `snake_case` key to `PascalCase`.
fn snake_to_pascal(s: &str) -> String {
    s.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
                    upper + chars.as_str()
                },
                None => String::new(),
            }
        })
        .collect()
}

/// Convert a TOML value to its clang-format string representation.
fn toml_value_to_clang(value: &toml::Value) -> String {
    match value {
        toml::Value::String(s) => s.clone(),
        toml::Value::Integer(i) => i.to_string(),
        toml::Value::Float(f) => f.to_string(),
        toml::Value::Boolean(b) => if *b {
            "true"
        } else {
            "false"
        }
        .to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
#[path = "../../tests/src/server/metalfmt_tests.rs"]
mod tests;
