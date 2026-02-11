//! Definition provider and AST index utilities.

mod ast_index;
mod clang_nodes;
mod compiler;
mod indexer;
mod index_cache;
mod project_index;
mod provider;
mod ref_site;
mod symbol_def;
mod utils;

pub use ast_index::AstIndex;
pub use project_index::ProjectIndex;
pub use provider::DefinitionProvider;
pub use ref_site::RefSite;
pub use symbol_def::SymbolDef;
pub use utils::{def_to_location, is_system_header, normalize_type_name, paths_match};
