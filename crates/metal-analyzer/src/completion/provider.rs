use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, Documentation, MarkupContent, MarkupKind, Position};

use crate::{
    completion::{
        builtins::{
            METAL_HEADERS, PREPROCESSOR_DIRECTIVES, TEXTURE_METHODS, builtin_to_completion_item, detect_function_name,
            first_identifier,
        },
        context::{CursorContext, detect_context},
    },
    metal::builtins::{self, BuiltinKind},
    syntax::SyntaxTree,
};

/// Provides intelligent completion items for Metal Shading Language.
pub struct CompletionProvider {
    // Currently stateless – all data lives in the lazy-static builtins database.
    // This struct exists so we can add per-session state later (e.g. workspace
    // symbol indices).
}

impl Default for CompletionProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CompletionProvider {
    pub fn new() -> Self {
        Self {}
    }

    /// Build a completion list for the given document text and cursor position.
    pub fn provide(
        &self,
        text: Option<&str>,
        position: Position,
        snapshot: Option<&SyntaxTree>,
    ) -> Vec<CompletionItem> {
        let text = text.unwrap_or("");
        let ctx = detect_context(text, position, snapshot.map(|s| s.root()));

        match ctx {
            CursorContext::Attribute => self.attribute_completions(),
            CursorContext::MemberAccess {
                ref receiver,
            } => self.member_completions(receiver),
            CursorContext::Preprocessor => self.preprocessor_completions(),
            CursorContext::Include => self.include_completions(),
            CursorContext::General => self.general_completions(text),
        }
    }

    // ───────────────────────────── completions ──────────────────────────────

    fn attribute_completions(&self) -> Vec<CompletionItem> {
        builtins::all()
            .iter()
            .filter(|e| e.kind == BuiltinKind::Attribute)
            .map(|e| builtin_to_completion_item(e, "0"))
            .collect()
    }

    fn member_completions(
        &self,
        receiver: &str,
    ) -> Vec<CompletionItem> {
        let mut items = Vec::new();

        let is_vector = Self::looks_like_vector_type(receiver);

        if is_vector {
            for comp in ["x", "y", "z", "w", "xy", "xyz", "xyzw", "xz", "yw", "zw"] {
                items.push(CompletionItem {
                    label: comp.to_string(),
                    kind: Some(CompletionItemKind::FIELD),
                    detail: Some("Vector swizzle".to_string()),
                    sort_text: Some(format!("0_{comp}")),
                    ..Default::default()
                });
            }
            for comp in ["r", "g", "b", "a", "rg", "rgb", "rgba", "rb", "ga", "ba"] {
                items.push(CompletionItem {
                    label: comp.to_string(),
                    kind: Some(CompletionItemKind::FIELD),
                    detail: Some("Color swizzle".to_string()),
                    sort_text: Some(format!("1_{comp}")),
                    ..Default::default()
                });
            }
        }

        let lower = receiver.to_lowercase();
        if lower.starts_with("texture")
            || lower.starts_with("tex")
            || lower.contains("texture")
            || lower.starts_with("depth")
        {
            for (name, sig, doc) in TEXTURE_METHODS {
                items.push(CompletionItem {
                    label: name.to_string(),
                    kind: Some(CompletionItemKind::METHOD),
                    detail: Some(sig.to_string()),
                    documentation: Some(Documentation::MarkupContent(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: doc.to_string(),
                    })),
                    sort_text: Some(format!("0_{name}")),
                    ..Default::default()
                });
            }
        }

        items
    }

    fn preprocessor_completions(&self) -> Vec<CompletionItem> {
        PREPROCESSOR_DIRECTIVES
            .iter()
            .enumerate()
            .map(|(i, (directive, doc))| CompletionItem {
                label: directive.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some("Preprocessor directive".to_string()),
                documentation: Some(Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: doc.to_string(),
                })),
                sort_text: Some(format!("{i:02}_{directive}")),
                ..Default::default()
            })
            .collect()
    }

    fn include_completions(&self) -> Vec<CompletionItem> {
        METAL_HEADERS
            .iter()
            .enumerate()
            .map(|(i, (header, doc))| CompletionItem {
                label: header.to_string(),
                kind: Some(CompletionItemKind::FILE),
                detail: Some("Metal standard library header".to_string()),
                documentation: Some(Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: doc.to_string(),
                })),
                insert_text: Some(format!("<{header}>")),
                sort_text: Some(format!("{i:02}_{header}")),
                ..Default::default()
            })
            .collect()
    }

    fn general_completions(
        &self,
        text: &str,
    ) -> Vec<CompletionItem> {
        let mut items: Vec<CompletionItem> = builtins::all()
            .iter()
            .map(|e| {
                let sort_prefix = match e.kind {
                    BuiltinKind::Keyword => "3",
                    BuiltinKind::Type => "2a",
                    BuiltinKind::Function => "2b",
                    BuiltinKind::Constant => "2c",
                    BuiltinKind::Attribute => "4",
                    BuiltinKind::Snippet => "5",
                };
                builtin_to_completion_item(e, sort_prefix)
            })
            .collect();

        items.extend(self.document_symbol_completions(text));

        items
    }

    /// Lightweight scan of the current document to offer completions for
    /// user-defined symbols (functions, structs, variables, macros).
    fn document_symbol_completions(
        &self,
        text: &str,
    ) -> Vec<CompletionItem> {
        let mut items = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for line in text.lines() {
            let trimmed = line.trim();

            if let Some(rest) = trimmed.strip_prefix("struct ")
                && let Some(name) = first_identifier(rest)
                && seen.insert(name.clone())
            {
                items.push(CompletionItem {
                    label: name.clone(),
                    kind: Some(CompletionItemKind::STRUCT),
                    detail: Some("User-defined struct".to_string()),
                    sort_text: Some(format!("1_{name}")),
                    ..Default::default()
                });
            }

            if let Some(rest) = trimmed.strip_prefix("enum ")
                && let Some(name) = first_identifier(rest)
                && seen.insert(name.clone())
            {
                items.push(CompletionItem {
                    label: name.clone(),
                    kind: Some(CompletionItemKind::ENUM),
                    detail: Some("User-defined enum".to_string()),
                    sort_text: Some(format!("1_{name}")),
                    ..Default::default()
                });
            }

            if trimmed.starts_with("typedef ")
                && let Some(name) = trimmed.trim_end_matches(';').split_whitespace().last()
            {
                let name = name.to_string();
                if !name.is_empty() && seen.insert(name.clone()) {
                    items.push(CompletionItem {
                        label: name.clone(),
                        kind: Some(CompletionItemKind::TYPE_PARAMETER),
                        detail: Some("Type alias".to_string()),
                        sort_text: Some(format!("1_{name}")),
                        ..Default::default()
                    });
                }
            }

            if let Some(fname) = detect_function_name(trimmed)
                && seen.insert(fname.clone())
            {
                items.push(CompletionItem {
                    label: fname.clone(),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some("User-defined function".to_string()),
                    sort_text: Some(format!("1_{fname}")),
                    ..Default::default()
                });
            }

            if let Some(rest) = trimmed.strip_prefix("#define ")
                && let Some(name) = first_identifier(rest)
                && seen.insert(name.clone())
            {
                items.push(CompletionItem {
                    label: name.clone(),
                    kind: Some(CompletionItemKind::CONSTANT),
                    detail: Some("Macro".to_string()),
                    sort_text: Some(format!("2_{name}")),
                    ..Default::default()
                });
            }
        }

        items
    }

    // ───────────────────────────── helpers ───────────────────────────────────

    fn looks_like_vector_type(name: &str) -> bool {
        let vector_prefixes = [
            "float",
            "half",
            "int",
            "uint",
            "short",
            "ushort",
            "char",
            "uchar",
            "bool",
            "vec",
            "pos",
            "color",
            "col",
            "normal",
            "norm",
            "dir",
            "uv",
            "coord",
            "position",
            "direction",
        ];
        let lower = name.to_lowercase();
        vector_prefixes.iter().any(|p| lower.starts_with(p))
            || lower.ends_with("color")
            || lower.ends_with("position")
            || lower.ends_with("normal")
            || lower.ends_with("coord")
    }
}
