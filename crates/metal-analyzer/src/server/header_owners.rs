use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use dashmap::DashMap;

pub(crate) fn is_header_file(path: &Path) -> bool {
    matches!(path.extension().and_then(|s| s.to_str()), Some("h" | "hh" | "hpp" | "hxx"))
}

pub(crate) fn parse_include_directives(source: &str) -> Vec<(String, bool)> {
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

pub(crate) fn collect_included_headers(
    owner: &Path,
    source: &str,
    include_paths: &[String],
) -> BTreeSet<PathBuf> {
    parse_include_directives(source)
        .into_iter()
        .filter_map(|(inc, is_system)| resolve_include_path(owner, &inc, is_system, include_paths))
        .filter(|p| is_header_file(p))
        .collect()
}

pub(crate) fn update_owner_links(
    header_owners: &DashMap<PathBuf, BTreeSet<PathBuf>>,
    owner_headers: &DashMap<PathBuf, BTreeSet<PathBuf>>,
    owner: &Path,
    new_headers: BTreeSet<PathBuf>,
) {
    let owner = normalize_path(owner);

    if let Some((_, previous_headers)) = owner_headers.remove(&owner) {
        for header in previous_headers {
            if let Some(mut owners) = header_owners.get_mut(&header) {
                owners.remove(&owner);
                if owners.is_empty() {
                    drop(owners);
                    header_owners.remove(&header);
                }
            }
        }
    }

    for header in &new_headers {
        let mut owners = header_owners.entry(header.clone()).or_default();
        owners.insert(owner.clone());
    }

    owner_headers.insert(owner, new_headers);
}

pub(crate) fn get_owner_candidates_for_header(
    header_owners: &DashMap<PathBuf, BTreeSet<PathBuf>>,
    header: &Path,
    cap: usize,
) -> Vec<PathBuf> {
    let Some(owners) = header_owners.get(&normalize_path(header)) else {
        return Vec::new();
    };
    owners.iter().take(cap).cloned().collect()
}

pub(crate) fn resolve_include_path(
    owner: &Path,
    include_path: &str,
    is_system: bool,
    include_paths: &[String],
) -> Option<PathBuf> {
    let include = Path::new(include_path);
    if include.is_absolute() && include.exists() {
        return Some(normalize_path(include));
    }

    if !is_system && let Some(parent) = owner.parent() {
        let candidate = parent.join(include);
        if candidate.exists() {
            return Some(normalize_path(&candidate));
        }
    }

    for include_dir in include_paths {
        if let Some(framework_root) = include_dir.strip_prefix(crate::metal::compiler::FRAMEWORK_DIR_PREFIX) {
            if let Some(resolved) = resolve_framework_include(framework_root, include_path) {
                return Some(resolved);
            }
        } else {
            let candidate = Path::new(include_dir).join(include);
            if candidate.exists() {
                return Some(normalize_path(&candidate));
            }
        }
    }

    None
}

/// Resolve a framework-style include using clang's lookup rule:
///   `Foo/Bar.h`  →  `<framework_root>/Foo.framework/Headers/Bar.h`
pub(crate) fn resolve_framework_include(framework_root: &str, include_path: &str) -> Option<PathBuf> {
    let include = Path::new(include_path);
    let mut components = include.components();
    let first = components.next()?.as_os_str().to_str()?;
    let rest = components.as_path();
    if rest.as_os_str().is_empty() {
        return None;
    }
    let candidate = Path::new(framework_root)
        .join(format!("{first}.framework"))
        .join("Headers")
        .join(rest);
    if candidate.exists() { Some(normalize_path(&candidate)) } else { None }
}

pub(crate) fn normalize_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
#[path = "../../tests/src/server/header_owners_tests.rs"]
mod tests;
