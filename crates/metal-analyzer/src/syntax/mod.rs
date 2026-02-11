pub mod ast;
pub mod cst;
pub mod cst_parser;
pub mod helpers;
pub mod kind;
pub mod lexer;
pub mod queries;

use dashmap::DashMap;
use std::sync::Arc;
use tower_lsp::lsp_types::{Range, Url};

use crate::syntax::cst::SyntaxNode;
use crate::syntax::cst_parser::Parser;

/// Immutable syntax snapshot for parsed documents.
#[derive(Clone)]
pub struct SyntaxTree {
    green: rowan::GreenNode,
    source: Arc<str>,
}

impl SyntaxTree {
    pub fn parse(source: &str) -> Self {
        let parser = Parser::new(source);
        let green = parser.parse();
        Self {
            green,
            source: Arc::from(source),
        }
    }

    pub fn root(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    pub fn source(&self) -> &str {
        &self.source
    }
}

/// Thread-safe store of parsed syntax trees for all open documents.
pub struct DocumentTrees {
    snapshots: DashMap<Url, SyntaxTree>,
}

impl DocumentTrees {
    pub fn new() -> Self {
        Self {
            snapshots: DashMap::new(),
        }
    }

    /// Full parse of a document, replacing any existing snapshot.
    pub fn parse_and_store(&self, uri: &Url, source: &str) {
        self.snapshots
            .insert(uri.clone(), SyntaxTree::parse(source));
    }

    pub fn insert(&self, uri: Url, tree: SyntaxTree) {
        self.snapshots.insert(uri, tree);
    }

    /// Get an Arc-cloned snapshot. No lock held after return.
    pub fn get(&self, uri: &Url) -> Option<SyntaxTree> {
        self.snapshots.get(uri).map(|entry| entry.clone())
    }

    /// Apply an edit and reparse using the new source.
    pub fn apply_change(
        &self,
        uri: &Url,
        range: Option<Range>,
        new_text: &str,
        new_full_source: &str,
    ) {
        let _ = (range, new_text);
        self.parse_and_store(uri, new_full_source);
    }

    pub fn remove(&self, uri: &Url) {
        self.snapshots.remove(uri);
    }
}

impl Default for DocumentTrees {
    fn default() -> Self {
        Self::new()
    }
}
