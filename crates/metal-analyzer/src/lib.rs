pub mod completion;
pub mod definition;
pub mod document;
pub mod hover;
pub mod metal;
pub mod progress;
pub mod semantic_tokens;
pub mod server;
pub mod symbols;
pub mod syntax;

pub use completion::CompletionProvider;
pub use definition::{
    AstIndex, DefinitionProvider, RefSite, SymbolDef, def_to_location, is_system_header,
    normalize_type_name, paths_match,
};
pub use hover::HoverProvider;
pub use semantic_tokens::SemanticTokenProvider;
pub use server::MetalLanguageServer;
pub use symbols::{SymbolIndex, SymbolLocation, SymbolProvider};
