use tower_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind};

use crate::symbols::SymbolLocation;

/// Build a `Hover` from a user symbol lookup.
pub(crate) async fn make_hover_from_user_symbol(word: &str, locations: &[SymbolLocation]) -> Hover {
    let mut md = String::new();
    let mut found_snippet = false;
    let mut doc_comment = String::new();

    // Try to read the definition line from the first location to show a snippet.
    if let Some(loc) = locations.first()
        && let Ok(path) = loc.uri.to_file_path()
        && let Ok(content) = tokio::fs::read_to_string(path).await
    {
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = loc.range.start.line as usize;

        if line_idx < lines.len() {
            let line = lines[line_idx];
            md.push_str("```metal\n");
            md.push_str(line.trim());
            md.push_str("\n```\n");
            found_snippet = true;

            // Look for doc comments preceding the definition (/// style)
            let mut comment_lines = Vec::new();
            let mut curr = line_idx;
            while curr > 0 {
                curr -= 1;
                let l = lines[curr].trim();
                if let Some(comment) = l.strip_prefix("///") {
                    comment_lines.push(comment.trim());
                } else {
                    if !comment_lines.is_empty() {
                        break;
                    }
                    if l.starts_with('[') || l.starts_with("template") || l.is_empty() {
                        continue;
                    }
                    break;
                }
            }
            if !comment_lines.is_empty() {
                comment_lines.reverse();
                doc_comment = comment_lines.join("\n");
            }
        }
    }

    if !found_snippet {
        md.push_str(&format!("**{}**\n\n", word));
    }

    if !doc_comment.is_empty() {
        md.push_str("\n---\n\n");
        md.push_str(&doc_comment);
        md.push('\n');
    }

    md.push_str("\nDefined in:\n");

    for loc in locations.iter().take(5) {
        let filename = loc
            .uri
            .to_file_path()
            .ok()
            .and_then(|p| p.file_name().map(|s| s.to_string_lossy().to_string()))
            .unwrap_or_else(|| loc.uri.path().to_string());

        let line = loc.range.start.line + 1;

        md.push_str(&format!(
            "- [`{}:{}`]({}#L{})\n",
            filename, line, loc.uri, line
        ));
    }

    if locations.len() > 5 {
        md.push_str(&format!("- *...and {} more*\n", locations.len() - 5));
    }

    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: md,
        }),
        range: None,
    }
}
