use std::path::{Path, PathBuf};
use std::sync::Arc;

use dashmap::DashMap;

use super::ast_index::AstIndex;
use super::ref_site::RefSite;
use super::symbol_def::SymbolDef;
use super::utils::is_system_header;

/// Project-wide symbol index built from AST dumps of all `.metal` files
/// in the workspace.
///
/// Each file gets its own [`AstIndex`]; cross-file queries iterate over
/// all of them. Since Clang node IDs are per-translation-unit, cross-file
/// lookups use symbol *names* rather than IDs.
pub struct ProjectIndex {
    files: DashMap<PathBuf, Arc<AstIndex>>,
}

impl Default for ProjectIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectIndex {
    pub fn new() -> Self {
        Self {
            files: DashMap::new(),
        }
    }

    pub fn update_file(&self, path: PathBuf, index: AstIndex) {
        self.files.insert(path, Arc::new(index));
    }

    pub fn remove_file(&self, path: &Path) {
        self.files.remove(path);
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Find definitions by name across all indexed files.
    ///
    /// Results are sorted: user files before system headers, definitions
    /// before forward declarations.
    pub fn find_definitions(&self, name: &str) -> Vec<SymbolDef> {
        let mut results = Vec::new();
        for entry in self.files.iter() {
            if let Some(indices) = entry.value().name_to_defs.get(name) {
                for &i in indices {
                    let def = &entry.value().defs[i];
                    if !def.file.is_empty() && def.line > 0 {
                        results.push(def.clone());
                    }
                }
            }
        }
        // User files first, then system headers; definitions before declarations.
        results.sort_by(|a, b| {
            let a_sys = is_system_header(&a.file);
            let b_sys = is_system_header(&b.file);
            a_sys
                .cmp(&b_sys)
                .then_with(|| b.is_definition.cmp(&a.is_definition))
        });
        results
    }

    /// Find all reference sites whose target name matches, across all files.
    pub fn find_references_by_name(&self, name: &str) -> Vec<RefSite> {
        let mut results = Vec::new();
        for entry in self.files.iter() {
            for r in &entry.value().refs {
                if r.target_name == name && !r.file.is_empty() && r.line > 0 {
                    results.push(r.clone());
                }
            }
        }
        results
    }
}
