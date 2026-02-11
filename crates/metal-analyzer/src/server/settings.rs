use std::collections::{HashMap, HashSet};

use serde::Deserialize;
use serde_json::Value;

use crate::metal::compiler::CompilerPlatform;

pub(crate) const SETTINGS_SECTION_KEY: &str = "metal-analyzer";
const MIN_DIAGNOSTIC_DEBOUNCE_MS: u64 = 50;
const MAX_DIAGNOSTIC_DEBOUNCE_MS: u64 = 5000;
const MIN_INDEXING_CONCURRENCY: usize = 1;
const MAX_INDEXING_CONCURRENCY: usize = 32;
const MIN_MAX_FILE_SIZE_KB: u64 = 16;
const MAX_MAX_FILE_SIZE_KB: u64 = 1024 * 64;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ServerSettings {
    pub(crate) formatting: FormattingSettings,
    pub(crate) diagnostics: DiagnosticsSettings,
    pub(crate) indexing: IndexingSettings,
    pub(crate) compiler: CompilerSettings,
    pub(crate) logging: LoggingSettings,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            formatting: FormattingSettings::default(),
            diagnostics: DiagnosticsSettings::default(),
            indexing: IndexingSettings::default(),
            compiler: CompilerSettings::default(),
            logging: LoggingSettings::default(),
        }
    }
}

impl ServerSettings {
    pub(crate) fn from_lsp_payload(payload: Option<&Value>) -> Self {
        let mut settings = Self::default();
        if let Some(payload) = payload {
            settings = settings.merged_with_payload(payload);
        }
        settings
    }

    pub(crate) fn merged_with_payload(&self, payload: &Value) -> Self {
        let mut merged = self.clone();

        for candidate in payload_candidates(payload) {
            if let Ok(patch) = serde_json::from_value::<ServerSettingsPatch>(candidate.clone()) {
                merged.apply_patch(patch);
            }
        }

        merged.normalize();
        merged
    }

    fn apply_patch(&mut self, patch: ServerSettingsPatch) {
        if let Some(formatting) = patch.formatting {
            self.formatting.apply_patch(formatting);
        }
        if let Some(diagnostics) = patch.diagnostics {
            self.diagnostics.apply_patch(diagnostics);
        }
        if let Some(indexing) = patch.indexing {
            self.indexing.apply_patch(indexing);
        }
        if let Some(compiler) = patch.compiler {
            self.compiler.apply_patch(compiler);
        }
        if let Some(logging) = patch.logging {
            self.logging.apply_patch(logging);
        }
    }

    fn normalize(&mut self) {
        self.formatting.normalize();
        self.diagnostics.normalize();
        self.indexing.normalize();
        self.compiler.normalize();
        self.logging.normalize();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FormattingSettings {
    pub(crate) enabled: bool,
    pub(crate) command: String,
    pub(crate) args: Vec<String>,
}

impl Default for FormattingSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            command: "clang-format".to_string(),
            args: Vec::new(),
        }
    }
}

impl FormattingSettings {
    fn apply_patch(&mut self, patch: FormattingSettingsPatch) {
        if let Some(enabled) = patch.enabled {
            self.enabled = enabled;
        }
        if let Some(command) = patch.command {
            self.command = command;
        }
        if let Some(args) = patch.args {
            self.args = args;
        }
    }

    fn normalize(&mut self) {
        self.command = self.command.trim().to_string();
        if self.command.is_empty() {
            self.command = "clang-format".to_string();
        }

        self.args = self
            .args
            .iter()
            .map(|arg| arg.trim().to_string())
            .filter(|arg| !arg.is_empty())
            .collect();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DiagnosticsSettings {
    pub(crate) on_type: bool,
    pub(crate) on_save: bool,
    pub(crate) debounce_ms: u64,
    pub(crate) scope: DiagnosticsScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) enum DiagnosticsScope {
    #[default]
    OpenFiles,
    Workspace,
}

impl DiagnosticsScope {
    pub(crate) fn is_workspace(self) -> bool {
        matches!(self, DiagnosticsScope::Workspace)
    }
}

impl Default for DiagnosticsSettings {
    fn default() -> Self {
        Self {
            on_type: true,
            on_save: true,
            debounce_ms: 500,
            scope: DiagnosticsScope::OpenFiles,
        }
    }
}

impl DiagnosticsSettings {
    fn apply_patch(&mut self, patch: DiagnosticsSettingsPatch) {
        if let Some(on_type) = patch.on_type {
            self.on_type = on_type;
        }
        if let Some(on_save) = patch.on_save {
            self.on_save = on_save;
        }
        if let Some(debounce_ms) = patch.debounce_ms {
            self.debounce_ms = debounce_ms;
        }
        if let Some(scope) = patch.scope {
            self.scope = scope;
        }
    }

    fn normalize(&mut self) {
        self.debounce_ms = self
            .debounce_ms
            .clamp(MIN_DIAGNOSTIC_DEBOUNCE_MS, MAX_DIAGNOSTIC_DEBOUNCE_MS);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct IndexingSettings {
    pub(crate) enabled: bool,
    pub(crate) concurrency: usize,
    pub(crate) max_file_size_kb: u64,
    pub(crate) exclude_paths: Vec<String>,
}

impl Default for IndexingSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            concurrency: 1,
            max_file_size_kb: 512,
            exclude_paths: Vec::new(),
        }
    }
}

impl IndexingSettings {
    fn apply_patch(&mut self, patch: IndexingSettingsPatch) {
        if let Some(enabled) = patch.enabled {
            self.enabled = enabled;
        }
        if let Some(concurrency) = patch.concurrency {
            self.concurrency = concurrency;
        }
        if let Some(max_file_size_kb) = patch.max_file_size_kb {
            self.max_file_size_kb = max_file_size_kb;
        }
        if let Some(exclude_paths) = patch.exclude_paths {
            self.exclude_paths = exclude_paths;
        }
    }

    fn normalize(&mut self) {
        self.concurrency = self
            .concurrency
            .clamp(MIN_INDEXING_CONCURRENCY, MAX_INDEXING_CONCURRENCY);
        self.max_file_size_kb = self
            .max_file_size_kb
            .clamp(MIN_MAX_FILE_SIZE_KB, MAX_MAX_FILE_SIZE_KB);
        let mut seen = HashSet::new();
        self.exclude_paths = self
            .exclude_paths
            .iter()
            .map(|path| path.trim().to_string())
            .filter(|path| !path.is_empty())
            .filter(|path| seen.insert(path.clone()))
            .collect();
    }

    pub(crate) fn max_file_size_bytes(&self) -> u64 {
        self.max_file_size_kb.saturating_mul(1024)
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct CompilerSettings {
    pub(crate) include_paths: Vec<String>,
    pub(crate) extra_flags: Vec<String>,
    pub(crate) platform: CompilerPlatform,
}

impl CompilerSettings {
    fn apply_patch(&mut self, patch: CompilerSettingsPatch) {
        if let Some(include_paths) = patch.include_paths {
            self.include_paths = include_paths;
        }
        if let Some(extra_flags) = patch.extra_flags {
            self.extra_flags = extra_flags;
        }
        if let Some(platform) = patch.platform {
            self.platform = CompilerPlatform::from_setting_value(&platform);
        }
    }

    fn normalize(&mut self) {
        self.include_paths = self
            .include_paths
            .iter()
            .map(|path| path.trim().to_string())
            .filter(|path| !path.is_empty())
            .collect();

        self.extra_flags = self
            .extra_flags
            .iter()
            .map(|flag| flag.trim().to_string())
            .filter(|flag| !flag.is_empty())
            .collect();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LoggingSettings {
    pub(crate) level: LoggingLevel,
}

impl Default for LoggingSettings {
    fn default() -> Self {
        Self {
            level: LoggingLevel::Info,
        }
    }
}

impl LoggingSettings {
    fn apply_patch(&mut self, patch: LoggingSettingsPatch) {
        if let Some(level) = patch.level {
            self.level = level;
        }
    }

    fn normalize(&mut self) {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub(crate) enum LoggingLevel {
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
}

impl LoggingLevel {
    pub(crate) fn allows_info(self) -> bool {
        self >= LoggingLevel::Info
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
    #[serde(flatten)]
    _extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct FormattingSettingsPatch {
    enabled: Option<bool>,
    command: Option<String>,
    args: Option<Vec<String>>,
    #[serde(flatten)]
    _extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct DiagnosticsSettingsPatch {
    on_type: Option<bool>,
    on_save: Option<bool>,
    debounce_ms: Option<u64>,
    scope: Option<DiagnosticsScope>,
    #[serde(flatten)]
    _extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct IndexingSettingsPatch {
    enabled: Option<bool>,
    concurrency: Option<usize>,
    max_file_size_kb: Option<u64>,
    exclude_paths: Option<Vec<String>>,
    #[serde(flatten)]
    _extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct CompilerSettingsPatch {
    include_paths: Option<Vec<String>>,
    extra_flags: Option<Vec<String>>,
    platform: Option<String>,
    #[serde(flatten)]
    _extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct LoggingSettingsPatch {
    level: Option<LoggingLevel>,
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

#[cfg(test)]
#[path = "../../tests/src/server/settings_tests.rs"]
mod tests;
