pub mod completion;
pub mod config;
pub mod definition;
pub mod document;
pub mod hover;
pub mod ide;
pub mod metal;
pub mod progress;
pub mod semantic_tokens;
pub mod server;
pub mod symbols;
pub mod syntax;
pub(crate) mod text_pos;
pub mod vfs;

pub use completion::CompletionProvider;
pub use definition::{
    AstIndex, DefinitionProvider, RefSite, SymbolDef, def_to_location, is_system_header, normalize_type_name,
    paths_match,
};
pub use hover::HoverProvider;
pub use ide::navigation::{IdeLocation, IdePosition, IdeRange, NavigationTarget};
pub use semantic_tokens::SemanticTokenProvider;
pub use server::MetalLanguageServer;
pub use symbols::{SymbolIndex, SymbolLocation, SymbolProvider};
pub use vfs::FileId;
