use std::sync::Arc;

use tower_lsp::lsp_types::{DocumentSymbol, Location, Range, SymbolInformation, SymbolKind, Url};
use tracing::debug;

use crate::syntax::SyntaxTree;

use super::index::SymbolIndex;
use super::scanner::{build_symbols, flatten_symbols};
use super::types::SymbolLocation;

#[derive(Clone)]
pub struct SymbolProvider {
    index: Arc<SymbolIndex>,
}

impl Default for SymbolProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolProvider {
    pub fn new() -> Self {
        Self {
            index: Arc::new(SymbolIndex::new()),
        }
    }

    pub fn index(&self) -> &SymbolIndex {
        &self.index
    }

    /// Index all symbols in a file for workspace-wide lookup.
    pub fn scan_file(&self, uri: &Url, text: &str) {
        let symbols = self.extract_symbols(text);
        // Remove old entries for this file first
        self.remove_file(uri);

        for sym in flatten_symbols(&symbols) {
            self.index.insert(
                sym.name.clone(),
                SymbolLocation {
                    uri: uri.clone(),
                    range: sym.selection_range,
                },
            );
        }
    }

    /// Remove all symbols for a file from the index.
    pub fn remove_file(&self, uri: &Url) {
        // DashMap doesn't support easy iteration with removal of value items.
        // We have to iterate all entries and retain only locations that don't match URI.
        for mut entry in self.index.map.iter_mut() {
            let locs: &mut Vec<SymbolLocation> = entry.value_mut();
            locs.retain(|l| l.uri != *uri);
        }
        // Cleanup empty entries
        self.index
            .map
            .retain(|_, v: &mut Vec<SymbolLocation>| !v.is_empty());
    }

    /// Extract document symbols from source text (parses internally).
    pub fn extract_symbols(&self, source: &str) -> Vec<DocumentSymbol> {
        let snapshot = SyntaxTree::parse(source);
        self.extract_symbols_from_snapshot(&snapshot)
    }

    /// Extract symbols from a pre-parsed snapshot (avoids redundant parsing).
    pub fn extract_symbols_from_snapshot(&self, snapshot: &SyntaxTree) -> Vec<DocumentSymbol> {
        let text = snapshot.source();
        let root = snapshot.root();
        build_symbols(&root, text)
    }

    /// Find the definition of a symbol by name in the given source text.
    pub fn quick_definition(&self, source: &str, word: &str) -> Option<Range> {
        let snapshot = SyntaxTree::parse(source);
        self.quick_definition_from_snapshot(&snapshot, word)
    }

    /// Find definition using a pre-parsed snapshot.
    ///
    /// Returns `None` when the name is ambiguous (multiple symbols match) to
    /// force fallback to the scope-aware AST resolver.
    pub fn quick_definition_from_snapshot(
        &self,
        snapshot: &SyntaxTree,
        word: &str,
    ) -> Option<Range> {
        let symbols = self.extract_symbols_from_snapshot(snapshot);
        let mut matches = symbols.iter().filter(|s| s.name == word);
        let first = matches.next()?;
        if matches.next().is_some() {
            debug!(
                "[quick-def] '{word}' is ambiguous (multiple definitions in file), deferring to AST",
            );
            return None;
        }
        debug!(
            "[quick-def] matched '{word}' → line {} col {} (out of {} symbols)",
            first.selection_range.start.line,
            first.selection_range.start.character,
            symbols.len(),
        );
        Some(first.selection_range)
    }

    /// Returns symbols for the given document (flat list from index).
    pub fn document_symbols(&self, uri: &Url) -> Vec<SymbolInformation> {
        let mut results = Vec::new();
        for entry in self.index.map.iter() {
            let name: &String = entry.key();
            for loc in entry.value() {
                if loc.uri == *uri {
                    #[allow(deprecated)]
                    results.push(SymbolInformation {
                        name: name.clone(),
                        kind: SymbolKind::VARIABLE, // Kind is not stored in SymbolLocation
                        tags: None,
                        deprecated: None,
                        location: Location {
                            uri: loc.uri.clone(),
                            range: loc.range,
                        },
                        container_name: None,
                    });
                }
            }
        }
        results
    }

    pub fn workspace_symbols(&self, query: &str) -> Vec<SymbolInformation> {
        let hits = self.index.search(query, 100);
        hits.into_iter()
            .map(|(name, loc)| {
                #[allow(deprecated)]
                SymbolInformation {
                    name,
                    kind: SymbolKind::VARIABLE, // Kind is not stored in SymbolLocation
                    tags: None,
                    deprecated: None,
                    location: Location {
                        uri: loc.uri,
                        range: loc.range,
                    },
                    container_name: None,
                }
            })
            .collect()
    }
}
