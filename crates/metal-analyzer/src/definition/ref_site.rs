use serde::{Deserialize, Serialize};

/// A reference site — a place in the source where a symbol is *used*.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefSite {
    /// File containing the reference.
    pub file: String,
    /// 1-based line.
    pub line: u32,
    /// 1-based column.
    pub col: u32,
    /// Length of the referenced token in characters.
    pub tok_len: u32,
    /// AST node id of the declaration this reference points to.
    pub target_id: String,
    /// Name of the referenced symbol (for sanity-checking).
    pub target_name: String,
    /// Kind of the symbol being referenced.
    pub target_kind: String,
    /// Where the token is expanded (macro call-site), if available.
    pub expansion: Option<RefSiteLocation>,
    /// Where the token is spelled (macro body/definition), if available.
    pub spelling: Option<RefSiteLocation>,
}

/// A concrete source location for a reference token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefSiteLocation {
    /// File containing this location.
    pub file: String,
    /// 1-based line.
    pub line: u32,
    /// 1-based column.
    pub col: u32,
    /// Token length in characters.
    pub tok_len: u32,
}
