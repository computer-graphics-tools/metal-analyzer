use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use dashmap::DashMap;

pub(crate) fn is_header_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|s| s.to_str()),
        Some("h" | "hh" | "hpp" | "hxx")
    )
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

    if !is_system
        && let Some(parent) = owner.parent()
    {
        let candidate = parent.join(include);
        if candidate.exists() {
            return Some(normalize_path(&candidate));
        }
    }

    for include_dir in include_paths {
        let candidate = Path::new(include_dir).join(include);
        if candidate.exists() {
            return Some(normalize_path(&candidate));
        }
    }

    None
}

pub(crate) fn normalize_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_include_directives_detects_system_and_local() {
        let src = r#"
#include <metal_stdlib>
#include "common/utils.h"
// #include "ignored.h"
"#;
        let parsed = parse_include_directives(src);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0], ("metal_stdlib".to_owned(), true));
        assert_eq!(parsed[1], ("common/utils.h".to_owned(), false));
    }

    #[test]
    fn update_owner_links_replaces_previous_headers() {
        let headers_to_owners = DashMap::new();
        let owners_to_headers = DashMap::new();
        let owner = PathBuf::from("/tmp/owner.metal");
        let h1 = PathBuf::from("/tmp/a.h");
        let h2 = PathBuf::from("/tmp/b.h");

        update_owner_links(
            &headers_to_owners,
            &owners_to_headers,
            &owner,
            BTreeSet::from([h1.clone()]),
        );
        update_owner_links(
            &headers_to_owners,
            &owners_to_headers,
            &owner,
            BTreeSet::from([h2.clone()]),
        );

        assert!(headers_to_owners.get(&h1).is_none());
        assert!(headers_to_owners.get(&h2).is_some());
        assert_eq!(
            owners_to_headers
                .get(&owner)
                .expect("owner exists")
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec![h2]
        );
    }
}
