use dashmap::DashMap;
use tower_lsp::lsp_types::{TextDocumentContentChangeEvent, Url};

use crate::document::Document;

/// Thread-safe store of all open documents.
///
/// Uses `DashMap` internally so that all operations are safe to call
/// concurrently from any async task without external synchronisation.
///
/// Documents are keyed by a normalized form of their URI, so a URI built
/// from a filesystem path finds the document the client opened even when the
/// client spells it differently (VS Code on Windows sends `file:///c%3A/...`,
/// while `Url::from_file_path` produces `file:///C:/...`).
#[derive(Debug)]
pub struct DocumentStore {
    documents: DashMap<Url, Document>,
}

fn key(uri: &Url) -> Url {
    uri.to_file_path().ok().and_then(|path| Url::from_file_path(path).ok()).unwrap_or_else(|| uri.clone())
}

impl DocumentStore {
    pub fn new() -> Self {
        Self {
            documents: DashMap::new(),
        }
    }

    /// Open (register) a new document.
    pub fn open(
        &self,
        uri: Url,
        text: String,
        version: i32,
    ) {
        self.documents.insert(key(&uri), Document::new(uri, text, version));
    }

    /// Replace the full content of an already-open document.
    ///
    /// This is the API used by `did_change` with `TextDocumentSyncKind::FULL`
    /// and by `did_save` when `includeText` is enabled.
    pub fn update(
        &self,
        uri: Url,
        text: String,
        version: i32,
    ) {
        if let Some(mut doc) = self.documents.get_mut(&key(&uri)) {
            doc.set_content(text, version);
        } else {
            // Defensive: treat as open if not already tracked.
            self.documents.insert(key(&uri), Document::new(uri, text, version));
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
        if let Some(mut doc) = self.documents.get_mut(&key(uri)) {
            doc.apply_changes(changes, version);
        }
    }

    /// Close (unregister) a document.
    pub fn close(
        &self,
        uri: &Url,
    ) {
        self.documents.remove(&key(uri));
    }

    /// Return a clone of the full document text, if the URI is tracked.
    pub fn get_content(
        &self,
        uri: &Url,
    ) -> Option<String> {
        self.documents.get(&key(uri)).map(|r| r.value().text.clone())
    }

    /// Return a clone of the full `Document`, if the URI is tracked.
    #[allow(dead_code)]
    pub fn get(
        &self,
        uri: &Url,
    ) -> Option<Document> {
        self.documents.get(&key(uri)).map(|r| r.value().clone())
    }

    /// Return the URI the client used to open the document matching `uri`.
    pub fn client_uri(
        &self,
        uri: &Url,
    ) -> Option<Url> {
        self.documents.get(&key(uri)).map(|r| r.value().uri.clone())
    }

    /// Return all currently open document URIs.
    #[allow(dead_code)]
    pub fn all_uris(&self) -> Vec<Url> {
        self.documents.iter().map(|r| r.value().uri.clone()).collect()
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
