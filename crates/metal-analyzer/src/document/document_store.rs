use dashmap::DashMap;
use tower_lsp::lsp_types::{TextDocumentContentChangeEvent, Url};

use super::Document;

/// Thread-safe store of all open documents.
///
/// Uses `DashMap` internally so that all operations are safe to call
/// concurrently from any async task without external synchronisation.
#[derive(Debug)]
pub struct DocumentStore {
    documents: DashMap<Url, Document>,
}

impl DocumentStore {
    pub fn new() -> Self {
        Self {
            documents: DashMap::new(),
        }
    }

    /// Open (register) a new document.
    pub fn open(&self, uri: Url, text: String, version: i32) {
        self.documents
            .insert(uri.clone(), Document::new(uri, text, version));
    }

    /// Replace the full content of an already-open document.
    ///
    /// This is the API used by `did_change` with `TextDocumentSyncKind::FULL`
    /// and by `did_save` when `includeText` is enabled.
    pub fn update(&self, uri: Url, text: String, version: i32) {
        if let Some(mut doc) = self.documents.get_mut(&uri) {
            doc.set_content(text, version);
        } else {
            // Defensive: treat as open if not already tracked.
            self.documents
                .insert(uri.clone(), Document::new(uri, text, version));
        }
    }

    /// Apply incremental or full-content changes to an already-open document.
    #[allow(dead_code)]
    pub fn apply_changes(
        &self,
        uri: &Url,
        changes: Vec<TextDocumentContentChangeEvent>,
        version: i32,
    ) {
        if let Some(mut doc) = self.documents.get_mut(uri) {
            doc.apply_changes(changes, version);
        }
    }

    /// Close (unregister) a document.
    pub fn close(&self, uri: &Url) {
        self.documents.remove(uri);
    }

    /// Return a clone of the full document text, if the URI is tracked.
    pub fn get_content(&self, uri: &Url) -> Option<String> {
        self.documents.get(uri).map(|r| r.value().text.clone())
    }

    /// Return a clone of the full `Document`, if the URI is tracked.
    #[allow(dead_code)]
    pub fn get(&self, uri: &Url) -> Option<Document> {
        self.documents.get(uri).map(|r| r.value().clone())
    }

    /// Return all currently open document URIs.
    #[allow(dead_code)]
    pub fn all_uris(&self) -> Vec<Url> {
        self.documents.iter().map(|r| r.key().clone()).collect()
    }
}

impl Default for DocumentStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "../../tests/src/document/document_store_tests.rs"]
mod tests;
