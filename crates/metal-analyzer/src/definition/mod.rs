//! Definition provider and AST index utilities.

pub(crate) mod ast_index;
pub(crate) mod clang_nodes;
pub(crate) mod compiler;
pub(crate) mod fallback_lookup;
pub(crate) mod index_cache;
pub(crate) mod indexer;
pub(crate) mod perf;
pub(crate) mod precise_lookup;
pub(crate) mod project_graph;
pub(crate) mod project_index;
pub(crate) mod provider;
pub(crate) mod ref_site;
pub(crate) mod symbol_def;
pub(crate) mod symbol_rank;
pub(crate) mod symbol_text;
pub(crate) mod system_lookup;
pub(crate) mod utils;

pub use ast_index::AstIndex;
pub use project_index::ProjectIndex;
pub use provider::DefinitionProvider;
pub use ref_site::RefSite;
pub use symbol_def::SymbolDef;
pub use utils::{def_to_location, is_system_header, normalize_type_name, paths_match};
