use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
};

use dashmap::DashMap;

use crate::{
    definition::{ast_index::AstIndex, ref_site::RefSite, symbol_def::SymbolDef, utils::is_system_header},
    vfs::FileId,
};

/// Project-wide symbol index built from AST dumps of all `.metal` files
/// in the workspace.
///
/// Each file gets its own [`AstIndex`]; cross-file queries iterate over
/// all of them. Since Clang node IDs are per-translation-unit, cross-file
/// lookups use symbol *names* rather than IDs.
pub struct ProjectIndex {
    files: DashMap<FileId, ProjectFileIndex>,
}

struct ProjectFileIndex {
    index: Arc<AstIndex>,
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

    pub fn update_file(
        &self,
        path: PathBuf,
        index: AstIndex,
    ) {
        let file_id = FileId::from_path(&path);
        self.files.insert(
            file_id,
            ProjectFileIndex {
                index: Arc::new(index),
            },
        );
    }

    pub fn remove_file(
        &self,
        path: &Path,
    ) {
        let file_id = FileId::from_path(path);
        self.files.remove(&file_id);
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Find definitions by name across all indexed files.
    ///
    /// Results are sorted: user files before system headers, definitions
    /// before forward declarations.
    pub fn find_definitions(
        &self,
        name: &str,
    ) -> Vec<SymbolDef> {
        let mut results = Vec::new();
        for entry in self.files.iter() {
            if let Some(indices) = entry.value().index.name_to_defs.get(name) {
                for &i in indices {
                    let def = &entry.value().index.defs[i];
                    if !def.file.is_empty() && def.line > 0 {
                        results.push(def.clone());
                    }
                }
            }
        }
        results.sort_by(|a, b| {
            let a_sys = is_system_header(&a.file);
            let b_sys = is_system_header(&b.file);
            a_sys.cmp(&b_sys).then_with(|| b.is_definition.cmp(&a.is_definition))
        });
        results
    }

    /// Find definitions by name in a scoped subset of files.
    pub fn find_definitions_in_files(
        &self,
        name: &str,
        file_scope: &HashSet<FileId>,
    ) -> Vec<SymbolDef> {
        let mut results = Vec::new();
        for entry in self.files.iter() {
            if !file_scope.contains(entry.key()) {
                continue;
            }
            if let Some(indices) = entry.value().index.name_to_defs.get(name) {
                for &i in indices {
                    let def = &entry.value().index.defs[i];
                    if !def.file.is_empty() && def.line > 0 {
                        results.push(def.clone());
                    }
                }
            }
        }
        results.sort_by(|a, b| {
            let a_sys = is_system_header(&a.file);
            let b_sys = is_system_header(&b.file);
            a_sys.cmp(&b_sys).then_with(|| b.is_definition.cmp(&a.is_definition))
        });
        results
    }

    /// Find all reference sites whose target name matches, across all files.
    pub fn find_references_by_name(
        &self,
        name: &str,
    ) -> Vec<RefSite> {
        let mut results = Vec::new();
        for entry in self.files.iter() {
            for r in &entry.value().index.refs {
                if r.target_name == name && !r.file.is_empty() && r.line > 0 {
                    results.push(r.clone());
                }
            }
        }
        results
    }
}
