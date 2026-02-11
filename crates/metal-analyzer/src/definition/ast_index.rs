use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::ref_site::RefSite;
use super::symbol_def::SymbolDef;
use super::utils::is_system_header;

/// Indexed AST data for a single translation unit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AstIndex {
    pub defs: Vec<SymbolDef>,
    pub refs: Vec<RefSite>,
    /// Map from ID to index in `defs`.
    pub id_to_def: HashMap<String, usize>,
    /// Map from name to indices in `defs`.
    pub name_to_defs: HashMap<String, Vec<usize>>,
    /// Map from target_id to all reference sites pointing to it.
    pub target_id_to_refs: HashMap<String, Vec<usize>>,
    /// Map from file path to indices in `defs` for definitions in that file.
    pub file_to_defs: HashMap<String, Vec<usize>>,
    /// Map from file path to indices in `refs` for references in that file.
    pub file_to_refs: HashMap<String, Vec<usize>>,
}

impl AstIndex {
    /// Get all declarations (non-definitions) for a symbol by name.
    pub fn get_declarations(&self, name: &str) -> Vec<&SymbolDef> {
        self.name_to_defs
            .get(name)
            .map(|indices| {
                indices
                    .iter()
                    .filter_map(|&i| {
                        let def = &self.defs[i];
                        if !def.is_definition { Some(def) } else { None }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get the type definition for a variable/field/parameter.
    /// Returns the definition of the type if `type_name` is set and found.
    pub fn get_type_definition(&self, def: &SymbolDef) -> Option<&SymbolDef> {
        let type_name = def.type_name.as_deref()?;
        let indices = self.name_to_defs.get(type_name)?;

        let candidates: Vec<&SymbolDef> = indices
            .iter()
            .map(|&i| &self.defs[i])
            .filter(|d| {
                matches!(
                    d.kind.as_str(),
                    "CXXRecordDecl"
                        | "TypedefDecl"
                        | "TypeAliasDecl"
                        | "EnumDecl"
                        | "TemplateTypeParmDecl"
                )
            })
            .collect();

        if candidates.is_empty() {
            return None;
        }

        let user_candidates: Vec<&SymbolDef> = candidates
            .iter()
            .copied()
            .filter(|d| !is_system_header(&d.file))
            .collect();
        let pool = if !user_candidates.is_empty() {
            user_candidates
        } else {
            candidates
        };

        let definitions: Vec<&SymbolDef> =
            pool.iter().copied().filter(|d| d.is_definition).collect();
        let pool = if !definitions.is_empty() {
            definitions
        } else {
            pool
        };

        pool.into_iter().next()
    }

    /// Get all references to a symbol by its ID.
    pub fn get_references(&self, target_id: &str) -> Vec<&RefSite> {
        self.target_id_to_refs
            .get(target_id)
            .map(|indices| indices.iter().map(|&i| &self.refs[i]).collect())
            .unwrap_or_default()
    }

    /// Get all references in a specific file.
    pub fn get_references_in_file(&self, file: &str) -> Vec<&RefSite> {
        self.file_to_refs
            .get(file)
            .map(|indices| indices.iter().map(|&i| &self.refs[i]).collect())
            .unwrap_or_default()
    }

    /// Find implementations - for now, this is the same as definitions.
    /// In the future, we could distinguish between interface and implementation.
    pub fn get_implementations(&self, name: &str) -> Vec<&SymbolDef> {
        self.name_to_defs
            .get(name)
            .map(|indices| {
                indices
                    .iter()
                    .filter_map(|&i| {
                        let def = &self.defs[i];
                        if def.is_definition { Some(def) } else { None }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}
