use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::definition::AstIndex;

const CACHE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct CachedAstIndex {
    schema_version: u32,
    source_file: String,
    source_hash: String,
    include_hash: String,
    index: AstIndex,
}

pub(crate) fn load(
    source_file: &Path,
    source_hash: &str,
    include_paths: &[String],
) -> Option<AstIndex> {
    let root = default_cache_dir();
    load_from_root(&root, source_file, source_hash, include_paths)
}

pub(crate) fn save(
    source_file: &Path,
    source_hash: &str,
    include_paths: &[String],
    index: &AstIndex,
) {
    let root = default_cache_dir();
    save_to_root(&root, source_file, source_hash, include_paths, index);
}

fn load_from_root(
    root: &Path,
    source_file: &Path,
    source_hash: &str,
    include_paths: &[String],
) -> Option<AstIndex> {
    let cache_file = cache_file_path(root, source_file);
    let content = std::fs::read_to_string(&cache_file).ok()?;
    let payload = serde_json::from_str::<CachedAstIndex>(&content).ok()?;
    let normalized_source_file = normalized_path_string(source_file);
    let include_hash = include_paths_hash(include_paths);

    let valid = payload.schema_version == CACHE_SCHEMA_VERSION
        && payload.source_file == normalized_source_file
        && payload.source_hash == source_hash
        && payload.include_hash == include_hash;
    if !valid {
        return None;
    }

    debug!("[index-cache] hit {}", source_file.display());
    Some(payload.index)
}

fn save_to_root(
    root: &Path,
    source_file: &Path,
    source_hash: &str,
    include_paths: &[String],
    index: &AstIndex,
) {
    if std::fs::create_dir_all(root).is_err() {
        return;
    }

    let payload = CachedAstIndex {
        schema_version: CACHE_SCHEMA_VERSION,
        source_file: normalized_path_string(source_file),
        source_hash: source_hash.to_owned(),
        include_hash: include_paths_hash(include_paths),
        index: index.clone(),
    };
    let cache_file = cache_file_path(root, source_file);
    let Ok(json) = serde_json::to_string(&payload) else {
        return;
    };
    let _ = std::fs::write(cache_file, json);
}

fn default_cache_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".metal-analyzer").join("index-cache");
    }
    std::env::temp_dir().join("metal-analyzer-index-cache")
}

fn cache_file_path(
    root: &Path,
    source_file: &Path,
) -> PathBuf {
    let key = stable_hash_hex(&normalized_path_string(source_file));
    root.join(format!("{key}.json"))
}

fn include_paths_hash(include_paths: &[String]) -> String {
    let serialized = include_paths.join("\n");
    stable_hash_hex(&serialized)
}

fn stable_hash_hex(input: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn normalized_path_string(path: &Path) -> String {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf()).display().to_string()
}

#[cfg(test)]
#[path = "../../tests/src/definition/index_cache_tests.rs"]
mod tests;
