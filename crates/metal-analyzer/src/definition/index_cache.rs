use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::debug;

use super::AstIndex;

const CACHE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct CachedAstIndex {
    schema_version: u32,
    source_file: String,
    source_hash: String,
    include_hash: String,
    index: AstIndex,
}

pub(crate) async fn load(
    source_file: &Path,
    source_hash: &str,
    include_paths: &[String],
) -> Option<AstIndex> {
    let root = default_cache_dir();
    load_from_root(&root, source_file, source_hash, include_paths).await
}

pub(crate) async fn save(
    source_file: &Path,
    source_hash: &str,
    include_paths: &[String],
    index: &AstIndex,
) {
    let root = default_cache_dir();
    save_to_root(&root, source_file, source_hash, include_paths, index).await;
}

async fn load_from_root(
    root: &Path,
    source_file: &Path,
    source_hash: &str,
    include_paths: &[String],
) -> Option<AstIndex> {
    let cache_file = cache_file_path(root, source_file);
    let content = tokio::fs::read_to_string(&cache_file).await.ok()?;
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

async fn save_to_root(
    root: &Path,
    source_file: &Path,
    source_hash: &str,
    include_paths: &[String],
    index: &AstIndex,
) {
    if tokio::fs::create_dir_all(root).await.is_err() {
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
    let _ = tokio::fs::write(cache_file, json).await;
}

fn default_cache_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".metal-analyzer").join("index-cache");
    }
    std::env::temp_dir().join("metal-analyzer-index-cache")
}

fn cache_file_path(root: &Path, source_file: &Path) -> PathBuf {
    let key = stable_hash_hex(&normalized_path_string(source_file));
    root.join(format!("{key}.json"))
}

fn include_paths_hash(include_paths: &[String]) -> String {
    let serialized = include_paths.join("\n");
    stable_hash_hex(&serialized)
}

fn stable_hash_hex(input: &str) -> String {
    // FNV-1a 64-bit
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn normalized_path_string(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::definition::ref_site::RefSite;
    use crate::definition::symbol_def::SymbolDef;

    #[tokio::test]
    async fn disk_cache_roundtrip_and_invalidation() {
        let root = std::env::temp_dir().join(format!(
            "metal-analyzer-index-cache-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock drift")
                .as_nanos()
        ));
        let file = root.join("shader.metal");
        let include_paths = vec!["/tmp/include".to_owned(), "/tmp/include2".to_owned()];
        let index = AstIndex {
            defs: vec![SymbolDef {
                id: "0x1".to_owned(),
                name: "foo".to_owned(),
                kind: "FunctionDecl".to_owned(),
                file: "/tmp/shader.metal".to_owned(),
                line: 1,
                col: 1,
                is_definition: true,
                type_name: None,
                qual_type: None,
            }],
            refs: vec![RefSite {
                file: "/tmp/shader.metal".to_owned(),
                line: 2,
                col: 3,
                tok_len: 3,
                target_id: "0x1".to_owned(),
                target_name: "foo".to_owned(),
                target_kind: "FunctionDecl".to_owned(),
                expansion: None,
                spelling: None,
            }],
            id_to_def: HashMap::from([("0x1".to_owned(), 0)]),
            name_to_defs: HashMap::from([("foo".to_owned(), vec![0])]),
            target_id_to_refs: HashMap::from([("0x1".to_owned(), vec![0])]),
            file_to_defs: HashMap::from([("/tmp/shader.metal".to_owned(), vec![0])]),
            file_to_refs: HashMap::from([("/tmp/shader.metal".to_owned(), vec![0])]),
        };

        save_to_root(&root, &file, "source-hash-1", &include_paths, &index).await;

        let loaded = load_from_root(&root, &file, "source-hash-1", &include_paths).await;
        assert!(loaded.is_some(), "cache should load for matching key");
        assert_eq!(loaded.expect("cache payload").defs.len(), 1);

        let stale_source = load_from_root(&root, &file, "source-hash-2", &include_paths).await;
        assert!(stale_source.is_none(), "cache must invalidate by source hash");

        let stale_include =
            load_from_root(&root, &file, "source-hash-1", &["/tmp/other".to_owned()]).await;
        assert!(
            stale_include.is_none(),
            "cache must invalidate by include path fingerprint"
        );

        let _ = tokio::fs::remove_dir_all(root).await;
    }
}
