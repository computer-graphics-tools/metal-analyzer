use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tower_lsp::lsp_types::Url;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FileId(String);

impl FileId {
    pub fn from_path(path: &Path) -> Self {
        Self(normalized_path(path).display().to_string())
    }

    pub fn from_url(url: &Url) -> Self {
        if let Ok(path) = url.to_file_path() {
            return Self::from_path(&path);
        }
        Self(url.as_str().to_owned())
    }

    pub fn from_source_path(path: &str) -> Option<Self> {
        if path.is_empty() {
            return None;
        }
        Some(Self::from_path(Path::new(path)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for FileId {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

pub fn normalized_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}
