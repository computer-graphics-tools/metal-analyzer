use std::collections::{HashMap, HashSet};

use serde::Deserialize;
use serde_json::Value;

pub const MIN_INDEXING_CONCURRENCY: usize = 1;
pub const MAX_INDEXING_CONCURRENCY: usize = 32;
pub const MIN_MAX_FILE_SIZE_KB: u64 = 16;
pub const MAX_MAX_FILE_SIZE_KB: u64 = 1024 * 64;
pub const MIN_PROJECT_GRAPH_DEPTH: usize = 0;
pub const MAX_PROJECT_GRAPH_DEPTH: usize = 8;
pub const MIN_PROJECT_GRAPH_MAX_NODES: usize = 16;
pub const MAX_PROJECT_GRAPH_MAX_NODES: usize = 4096;

#[derive(Debug, Clone, PartialEq)]
pub struct IndexingSettings {
    pub enable: bool,
    pub concurrency: usize,
    pub max_file_size_kb: u64,
    pub project_graph_depth: usize,
    pub project_graph_max_nodes: usize,
    pub exclude_paths: Vec<String>,
}

impl Default for IndexingSettings {
    fn default() -> Self {
        Self {
            enable: true,
            concurrency: 1,
            max_file_size_kb: 512,
            project_graph_depth: 3,
            project_graph_max_nodes: 256,
            exclude_paths: Vec::new(),
        }
    }
}

impl IndexingSettings {
    pub(crate) fn apply_patch(
        &mut self,
        patch: IndexingSettingsPatch,
    ) {
        if let Some(v) = patch.enable {
            self.enable = v;
        }
        if let Some(v) = patch.concurrency {
            self.concurrency = v;
        }
        if let Some(v) = patch.max_file_size_kb {
            self.max_file_size_kb = v;
        }
        if let Some(v) = patch.project_graph_depth {
            self.project_graph_depth = v;
        }
        if let Some(v) = patch.project_graph_max_nodes {
            self.project_graph_max_nodes = v;
        }
        if let Some(v) = patch.exclude_paths {
            self.exclude_paths = v;
        }
    }

    pub(crate) fn normalize(&mut self) {
        self.concurrency = self.concurrency.clamp(MIN_INDEXING_CONCURRENCY, MAX_INDEXING_CONCURRENCY);
        self.max_file_size_kb = self.max_file_size_kb.clamp(MIN_MAX_FILE_SIZE_KB, MAX_MAX_FILE_SIZE_KB);
        self.project_graph_depth = self.project_graph_depth.clamp(MIN_PROJECT_GRAPH_DEPTH, MAX_PROJECT_GRAPH_DEPTH);
        self.project_graph_max_nodes =
            self.project_graph_max_nodes.clamp(MIN_PROJECT_GRAPH_MAX_NODES, MAX_PROJECT_GRAPH_MAX_NODES);
        let mut seen = HashSet::new();
        self.exclude_paths = self
            .exclude_paths
            .iter()
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .filter(|p| seen.insert(p.clone()))
            .collect();
    }

    pub fn max_file_size_bytes(&self) -> u64 {
        self.max_file_size_kb.saturating_mul(1024)
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct IndexingSettingsPatch {
    pub(crate) enable: Option<bool>,
    pub(crate) concurrency: Option<usize>,
    pub(crate) max_file_size_kb: Option<u64>,
    pub(crate) project_graph_depth: Option<usize>,
    pub(crate) project_graph_max_nodes: Option<usize>,
    pub(crate) exclude_paths: Option<Vec<String>>,
    #[serde(flatten)]
    pub(crate) _extra: HashMap<String, Value>,
}
