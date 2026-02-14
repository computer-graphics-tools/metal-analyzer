use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

pub const MIN_WORKER_THREADS: usize = 1;
pub const MAX_WORKER_THREADS: usize = 64;
pub const MIN_FORMATTING_THREADS: usize = 1;
pub const MAX_FORMATTING_THREADS: usize = 8;

#[derive(Debug, Clone, PartialEq)]
pub struct ThreadPoolSettings {
    pub worker_threads: usize,
    pub formatting_threads: usize,
}

impl Default for ThreadPoolSettings {
    fn default() -> Self {
        Self {
            worker_threads: 0,
            formatting_threads: 1,
        }
    }
}

impl ThreadPoolSettings {
    pub fn resolved_worker_threads(&self) -> usize {
        if self.worker_threads == 0 {
            return std::thread::available_parallelism().map(|n| n.get()).unwrap_or(MIN_WORKER_THREADS);
        }
        self.worker_threads
    }

    pub fn resolved_formatting_threads(&self) -> usize {
        if self.formatting_threads == 0 {
            return MIN_FORMATTING_THREADS;
        }
        self.formatting_threads
    }

    pub(crate) fn apply_patch(
        &mut self,
        patch: ThreadPoolSettingsPatch,
    ) {
        if let Some(v) = patch.worker_threads {
            self.worker_threads = v;
        }
        if let Some(v) = patch.formatting_threads {
            self.formatting_threads = v;
        }
    }

    pub(crate) fn normalize(&mut self) {
        if self.worker_threads != 0 {
            self.worker_threads = self.worker_threads.clamp(MIN_WORKER_THREADS, MAX_WORKER_THREADS);
        }
        if self.formatting_threads == 0 {
            self.formatting_threads = MIN_FORMATTING_THREADS;
        }
        self.formatting_threads = self.formatting_threads.clamp(MIN_FORMATTING_THREADS, MAX_FORMATTING_THREADS);
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct ThreadPoolSettingsPatch {
    pub(crate) worker_threads: Option<usize>,
    pub(crate) formatting_threads: Option<usize>,
    #[serde(flatten)]
    pub(crate) _extra: HashMap<String, Value>,
}
