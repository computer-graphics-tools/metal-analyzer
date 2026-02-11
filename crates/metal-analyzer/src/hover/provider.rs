use std::sync::Arc;

use tower_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position, Url};

use crate::definition::DefinitionProvider;
use crate::metal::builtins;
use crate::symbols::SymbolProvider;
use crate::syntax::SyntaxTree;
use crate::syntax::helpers;

use super::attribute::{try_attribute_hover, try_attribute_hover_from_tree};
use super::builtins::make_hover_from_entry;
use super::user_symbol::make_hover_from_user_symbol;

/// Provides hover information for Metal Shading Language symbols.
pub struct HoverProvider {
    symbol_provider: Arc<SymbolProvider>,
    definition_provider: Arc<DefinitionProvider>,
}

impl HoverProvider {
    pub fn new(
        symbol_provider: Arc<SymbolProvider>,
        definition_provider: Arc<DefinitionProvider>,
    ) -> Self {
        Self {
            symbol_provider,
            definition_provider,
        }
    }

    /// Return hover information for the symbol at the given position in `text`.
    pub async fn provide(
        &self,
        uri: &Url,
        text: &str,
        position: Position,
        snapshot: Option<&SyntaxTree>,
    ) -> Option<Hover> {
        let (attr_hover, word) = {
            let root = snapshot.map(|s| s.root());
            let attr_hover = root
                .as_ref()
                .and_then(|t| try_attribute_hover_from_tree(t, text, position))
                .or_else(|| try_attribute_hover(text, position));
            let word = root
                .as_ref()
                .and_then(|t| helpers::word_at_position(t, text, position))
                .or_else(|| helpers::word_at_position_text_fallback(text, position));
            (attr_hover, word)
        };

        if let Some(hover) = attr_hover {
            return Some(hover);
        }

        let word = word?;
        if word.is_empty() {
            return None;
        }

        tracing::debug!("Hover requested for symbol: {word}");

        if let Some(entry) = builtins::lookup(&word) {
            return Some(make_hover_from_entry(entry));
        }

        let lower = word.to_lowercase();
        if lower != word
            && let Some(entry) = builtins::lookup(&lower)
        {
            return Some(make_hover_from_entry(entry));
        }

        // AST-based hover: check per-file cache and project index.
        if let Some(hover) = self.hover_from_ast(uri, &word) {
            return Some(hover);
        }

        let locations = self.symbol_provider.index().get(&word);
        if !locations.is_empty() {
            return Some(make_hover_from_user_symbol(&word, &locations).await);
        }

        for entry in builtins::all() {
            if entry.label.eq_ignore_ascii_case(&word) {
                return Some(make_hover_from_entry(entry));
            }
        }

        None
    }

    /// Build hover from AST index data (per-file cache or project index).
    fn hover_from_ast(&self, uri: &Url, word: &str) -> Option<Hover> {
        // Try per-file cached AST first.
        if let Some(index) = self.definition_provider.get_cached_index(uri) {
            if let Some(indices) = index.name_to_defs.get(word) {
                for &i in indices {
                    let def = &index.defs[i];
                    if let Some(hover) = format_symbol_hover(def) {
                        return Some(hover);
                    }
                }
            }
        }

        // Fall back to project index.
        let defs = self.definition_provider.project_index().find_definitions(word);
        for def in &defs {
            if let Some(hover) = format_symbol_hover(def) {
                return Some(hover);
            }
        }

        None
    }
}

/// Format a hover from a [`SymbolDef`] with type information.
fn format_symbol_hover(def: &crate::definition::SymbolDef) -> Option<Hover> {
    let qual_type = def.qual_type.as_deref()?;

    let snippet = match def.kind.as_str() {
        "FunctionDecl" | "CXXMethodDecl" => {
            format!("{} {}", qual_type_to_return_type(qual_type), def.name)
        }
        "VarDecl" | "FieldDecl" => format!("{}: {}", def.name, qual_type),
        "ParmVarDecl" => format!("{}: {}", def.name, qual_type),
        "TypedefDecl" | "TypeAliasDecl" => format!("typedef {} = {}", def.name, qual_type),
        "EnumConstantDecl" => format!("{} (enum constant)", def.name),
        _ => return None,
    };

    let mut md = format!("```metal\n{snippet}\n```\n");

    let filename = std::path::Path::new(&def.file)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    if !filename.is_empty() {
        md.push_str(&format!("\n*Defined in `{filename}:{}`*\n", def.line));
    }

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: md,
        }),
        range: None,
    })
}

/// Extract the return type from a function's qualified type string.
///
/// Clang reports function types as e.g. `"void (float *, uint)"`.
/// This extracts the part before the first `(`.
fn qual_type_to_return_type(qual_type: &str) -> &str {
    qual_type
        .find('(')
        .map(|i| qual_type[..i].trim())
        .unwrap_or(qual_type)
}
