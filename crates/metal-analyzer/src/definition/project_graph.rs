use std::{
    collections::{HashSet, VecDeque},
    path::{Path, PathBuf},
};

use dashmap::DashMap;

use crate::vfs::{FileId, normalized_path};

/// Include-graph relationships across indexed project files.
///
/// The graph is directed (owner -> include), but we also maintain reverse edges
/// so callers can cheaply gather a local neighborhood around a source file.
pub(super) struct ProjectGraph {
    owner_to_includes: DashMap<FileId, HashSet<FileId>>,
    include_to_owners: DashMap<FileId, HashSet<FileId>>,
}

impl Default for ProjectGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectGraph {
    pub(super) fn new() -> Self {
        Self {
            owner_to_includes: DashMap::new(),
            include_to_owners: DashMap::new(),
        }
    }

    pub(super) fn update_file(
        &self,
        owner_path: &Path,
        source: &str,
        include_paths: &[String],
    ) {
        let owner = normalized_path(owner_path);
        let owner_id = FileId::from_path(&owner);

        let new_includes: HashSet<FileId> = parse_include_directives(source)
            .into_iter()
            .filter_map(|(include, is_system)| resolve_include_path(&owner, &include, is_system, include_paths))
            .map(|path| FileId::from_path(&path))
            .collect();

        if let Some((_, previous_includes)) = self.owner_to_includes.remove(&owner_id) {
            for include_id in previous_includes {
                if let Some(mut owners) = self.include_to_owners.get_mut(&include_id) {
                    owners.remove(&owner_id);
                    if owners.is_empty() {
                        drop(owners);
                        self.include_to_owners.remove(&include_id);
                    }
                }
            }
        }

        for include_id in &new_includes {
            self.include_to_owners.entry(include_id.clone()).or_default().insert(owner_id.clone());
        }

        self.owner_to_includes.insert(owner_id, new_includes);
    }

    pub(super) fn scoped_files(
        &self,
        seed: &FileId,
        max_depth: usize,
        max_nodes: usize,
    ) -> HashSet<FileId> {
        let mut visited: HashSet<FileId> = HashSet::new();
        let mut queue: VecDeque<(FileId, usize)> = VecDeque::new();

        visited.insert(seed.clone());
        queue.push_back((seed.clone(), 0));

        while let Some((current, depth)) = queue.pop_front() {
            if depth >= max_depth || visited.len() >= max_nodes {
                continue;
            }

            if let Some(includes) = self.owner_to_includes.get(&current) {
                for include in includes.iter() {
                    if visited.len() >= max_nodes {
                        break;
                    }
                    if visited.insert(include.clone()) {
                        queue.push_back((include.clone(), depth + 1));
                    }
                }
            }

            if let Some(owners) = self.include_to_owners.get(&current) {
                for owner in owners.iter() {
                    if visited.len() >= max_nodes {
                        break;
                    }
                    if visited.insert(owner.clone()) {
                        queue.push_back((owner.clone(), depth + 1));
                    }
                }
            }
        }

        visited
    }
}

fn parse_include_directives(source: &str) -> Vec<(String, bool)> {
    let mut includes = Vec::new();
    for raw_line in source.lines() {
        let line = raw_line.trim_start();
        if !line.starts_with("#include") {
            continue;
        }
        if let Some(start) = line.find('<')
            && let Some(end) = line[start + 1..].find('>')
        {
            includes.push((line[start + 1..start + 1 + end].to_owned(), true));
            continue;
        }
        if let Some(start) = line.find('"')
            && let Some(end) = line[start + 1..].find('"')
        {
            includes.push((line[start + 1..start + 1 + end].to_owned(), false));
        }
    }
    includes
}

fn resolve_include_path(
    owner: &Path,
    include_path: &str,
    is_system: bool,
    include_paths: &[String],
) -> Option<PathBuf> {
    let include = Path::new(include_path);
    if include.is_absolute() && include.exists() {
        return Some(normalized_path(include));
    }

    if !is_system && let Some(parent) = owner.parent() {
        let candidate = parent.join(include);
        if candidate.exists() {
            return Some(normalized_path(&candidate));
        }
    }

    for include_dir in include_paths {
        let candidate = Path::new(include_dir).join(include);
        if candidate.exists() {
            return Some(normalized_path(&candidate));
        }
    }

    None
}
