use tower_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind};

use crate::metal::builtins::{BuiltinEntry, BuiltinKind};

/// Build an `Hover` from a `BuiltinEntry`.
pub(crate) fn make_hover_from_entry(entry: &BuiltinEntry) -> Hover {
    let mut md = String::new();

    // Code block with detail / signature.
    if !entry.detail.is_empty() {
        md.push_str("```metal\n");
        md.push_str(&entry.detail);
        md.push_str("\n```\n");
    } else {
        md.push_str("```metal\n");
        md.push_str(&entry.label);
        md.push_str("\n```\n");
    }

    // Description.
    if !entry.documentation.is_empty() {
        md.push_str("\n---\n\n");
        md.push_str(&entry.documentation);
        md.push('\n');
    }

    // Kind badge and optional category.
    let kind_label = match entry.kind {
        BuiltinKind::Keyword => "Keyword",
        BuiltinKind::Type => "Type",
        BuiltinKind::Function => "Function",
        BuiltinKind::Attribute => "Attribute",
        BuiltinKind::Snippet => "Snippet",
        BuiltinKind::Constant => "Constant",
    };

    if let Some(cat) = entry.category {
        md.push_str(&format!("\n*({kind_label} · {cat})*\n"));
    } else {
        md.push_str(&format!("\n*({kind_label})*\n"));
    }

    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: md,
        }),
        range: None,
    }
}
