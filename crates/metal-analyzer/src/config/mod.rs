//! Declarative configuration system inspired by rust-analyzer.
//!
//! Settings are split into one file per category. [`ServerSettings`]
//! aggregates all categories and handles JSON deserialization from LSP
//! initialization options and `didChangeConfiguration` payloads.

pub(crate) mod compiler;
pub(crate) mod diagnostics;
pub(crate) mod formatting;
pub(crate) mod indexing;
pub(crate) mod logging;
pub(crate) mod schema;
pub(crate) mod thread_pool;

use std::collections::HashMap;

pub use compiler::CompilerSettings;
use compiler::CompilerSettingsPatch;
use diagnostics::DiagnosticsSettingsPatch;
pub use diagnostics::{DiagnosticsScope, DiagnosticsSettings, MAX_DIAGNOSTIC_DEBOUNCE_MS, MIN_DIAGNOSTIC_DEBOUNCE_MS};
pub use formatting::FormattingSettings;
use formatting::FormattingSettingsPatch;
use indexing::IndexingSettingsPatch;
pub use indexing::{
    IndexingSettings, MAX_INDEXING_CONCURRENCY, MAX_MAX_FILE_SIZE_KB, MAX_PROJECT_GRAPH_DEPTH,
    MAX_PROJECT_GRAPH_MAX_NODES, MIN_INDEXING_CONCURRENCY, MIN_MAX_FILE_SIZE_KB, MIN_PROJECT_GRAPH_DEPTH,
    MIN_PROJECT_GRAPH_MAX_NODES,
};
use logging::LoggingSettingsPatch;
pub use logging::{LogLevel, LoggingSettings};
pub use schema::{
    SchemaField, SchemaType, generate_configuration_markdown, generate_package_json_properties, schema_fields,
};
use serde::Deserialize;
use serde_json::Value;
use thread_pool::ThreadPoolSettingsPatch;
pub use thread_pool::{
    MAX_FORMATTING_THREADS, MAX_WORKER_THREADS, MIN_FORMATTING_THREADS, MIN_WORKER_THREADS, ThreadPoolSettings,
};

pub const SETTINGS_SECTION_KEY: &str = "metal-analyzer";

#[derive(Debug, Clone, PartialEq)]
pub struct ServerSettings {
    pub formatting: FormattingSettings,
    pub diagnostics: DiagnosticsSettings,
    pub indexing: IndexingSettings,
    pub compiler: CompilerSettings,
    pub logging: LoggingSettings,
    pub thread_pool: ThreadPoolSettings,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            formatting: FormattingSettings::default(),
            diagnostics: DiagnosticsSettings::default(),
            indexing: IndexingSettings::default(),
            compiler: CompilerSettings::default(),
            logging: LoggingSettings::default(),
            thread_pool: ThreadPoolSettings::default(),
        }
    }
}

impl ServerSettings {
    pub fn from_lsp_payload(payload: Option<&Value>) -> Self {
        let mut settings = Self::default();
        if let Some(payload) = payload {
            settings = settings.merged_with_payload(payload);
        }
        settings
    }

    pub fn merged_with_payload(
        &self,
        payload: &Value,
    ) -> Self {
        let mut merged = self.clone();

        for candidate in payload_candidates(payload) {
            if let Ok(patch) = serde_json::from_value::<ServerSettingsPatch>(candidate.clone()) {
                merged.apply_patch(patch);
            }
        }

        merged.normalize();
        merged
    }

    fn apply_patch(
        &mut self,
        patch: ServerSettingsPatch,
    ) {
        if let Some(p) = patch.formatting {
            self.formatting.apply_patch(p);
        }
        if let Some(p) = patch.diagnostics {
            self.diagnostics.apply_patch(p);
        }
        if let Some(p) = patch.indexing {
            self.indexing.apply_patch(p);
        }
        if let Some(p) = patch.compiler {
            self.compiler.apply_patch(p);
        }
        if let Some(p) = patch.logging {
            self.logging.apply_patch(p);
        }
        if let Some(p) = patch.thread_pool {
            self.thread_pool.apply_patch(p);
        }
    }

    fn normalize(&mut self) {
        self.formatting.normalize();
        self.diagnostics.normalize();
        self.indexing.normalize();
        self.compiler.normalize();
        self.thread_pool.normalize();
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ServerSettingsPatch {
    formatting: Option<FormattingSettingsPatch>,
    diagnostics: Option<DiagnosticsSettingsPatch>,
    indexing: Option<IndexingSettingsPatch>,
    compiler: Option<CompilerSettingsPatch>,
    logging: Option<LoggingSettingsPatch>,
    thread_pool: Option<ThreadPoolSettingsPatch>,
    #[serde(flatten)]
    _extra: HashMap<String, Value>,
}

fn payload_candidates(payload: &Value) -> Vec<Value> {
    let mut candidates = Vec::new();
    candidates.push(payload.clone());
    if let Some(scoped) = payload.get(SETTINGS_SECTION_KEY) {
        candidates.push(scoped.clone());
    }
    candidates
}
