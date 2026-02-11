use tower_lsp::lsp_types::{Range, Url};

#[derive(Clone, Debug)]
pub struct SymbolLocation {
    pub uri: Url,
    pub range: Range,
}
