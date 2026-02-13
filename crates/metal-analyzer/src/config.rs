//! Declarative configuration system inspired by rust-analyzer.
//!
//! All server settings are defined once in this module. From that single
//! definition the module provides:
//!
//! - Public settings structs with defaults and `normalize()` logic.
//! - Private `*Patch` structs for partial JSON deserialization.
//! - `apply_patch()` glue between the two layers.
//! - [`schema_fields()`] – a list of [`SchemaField`] entries that can be
//!   rendered as a VS Code JSON schema or as Markdown documentation.

use std::collections::{HashMap, HashSet};

use serde::Deserialize;
use serde_json::Value;

use crate::metal::compiler::CompilerPlatform;

/// One entry in the generated configuration schema.
#[derive(Debug, Clone)]
pub struct SchemaField {
    /// Dot-separated key, e.g. `"metal-analyzer.formatting.enable"`.
    pub key: String,
    /// Human-readable description (used as `markdownDescription` in VS Code).
    pub description: String,
    /// JSON Schema `type` value.
    pub schema_type: SchemaType,
    /// JSON-encoded default value.
    pub default: Value,
}

/// Subset of JSON Schema types we support.
#[derive(Debug, Clone)]
pub enum SchemaType {
    Bool,
    String,
    Integer {
        minimum: Option<i64>,
        maximum: Option<i64>,
    },
    StringEnum {
        values: Vec<&'static str>,
    },
    StringArray,
}

impl SchemaField {
    /// Render this field as a JSON Schema property value.
    pub fn to_schema_value(&self) -> Value {
        let mut obj = serde_json::Map::new();
        obj.insert("markdownDescription".into(), Value::String(self.description.clone()));
        obj.insert("default".into(), self.default.clone());

        match &self.schema_type {
            SchemaType::Bool => {
                obj.insert("type".into(), Value::String("boolean".into()));
            },
            SchemaType::String => {
                obj.insert("type".into(), Value::String("string".into()));
            },
            SchemaType::Integer {
                minimum,
                maximum,
            } => {
                obj.insert("type".into(), Value::String("number".into()));
                if let Some(min) = minimum {
                    obj.insert("minimum".into(), Value::Number((*min).into()));
                }
                if let Some(max) = maximum {
                    obj.insert("maximum".into(), Value::Number((*max).into()));
                }
            },
            SchemaType::StringEnum {
                values,
            } => {
                obj.insert("type".into(), Value::String("string".into()));
                obj.insert("enum".into(), Value::Array(values.iter().map(|v| Value::String(v.to_string())).collect()));
            },
            SchemaType::StringArray => {
                obj.insert("type".into(), Value::String("array".into()));
                let mut items = serde_json::Map::new();
                items.insert("type".into(), Value::String("string".into()));
                obj.insert("items".into(), Value::Object(items));
            },
        }

        Value::Object(obj)
    }

    /// Render this field as a single Markdown list item.
    pub fn to_markdown(&self) -> String {
        format!("- `metal-analyzer.{}` - {}", self.key, self.description)
    }
}

pub const SETTINGS_SECTION_KEY: &str = "metal-analyzer";

pub const MIN_DIAGNOSTIC_DEBOUNCE_MS: u64 = 50;
pub const MAX_DIAGNOSTIC_DEBOUNCE_MS: u64 = 5000;
pub const MIN_INDEXING_CONCURRENCY: usize = 1;
pub const MAX_INDEXING_CONCURRENCY: usize = 32;
pub const MIN_WORKER_THREADS: usize = 1;
pub const MAX_WORKER_THREADS: usize = 64;
pub const MIN_FORMATTING_THREADS: usize = 1;
pub const MAX_FORMATTING_THREADS: usize = 8;
pub const MIN_MAX_FILE_SIZE_KB: u64 = 16;
pub const MAX_MAX_FILE_SIZE_KB: u64 = 1024 * 64;
pub const MIN_PROJECT_GRAPH_DEPTH: usize = 0;
pub const MAX_PROJECT_GRAPH_DEPTH: usize = 8;
pub const MIN_PROJECT_GRAPH_MAX_NODES: usize = 16;
pub const MAX_PROJECT_GRAPH_MAX_NODES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticsScope {
    #[default]
    OpenFiles,
    Workspace,
}

impl DiagnosticsScope {
    pub fn is_workspace(self) -> bool {
        matches!(self, DiagnosticsScope::Workspace)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    pub fn allows_info(self) -> bool {
        self >= LogLevel::Info
    }
}

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

#[derive(Debug, Clone, PartialEq)]
pub struct FormattingSettings {
    pub enable: bool,
    pub command: String,
    pub args: Vec<String>,
}

impl Default for FormattingSettings {
    fn default() -> Self {
        Self {
            enable: true,
            command: "clang-format".to_string(),
            args: Vec::new(),
        }
    }
}

impl FormattingSettings {
    fn apply_patch(
        &mut self,
        patch: FormattingSettingsPatch,
    ) {
        if let Some(v) = patch.enable {
            self.enable = v;
        }
        if let Some(v) = patch.command {
            self.command = v;
        }
        if let Some(v) = patch.args {
            self.args = v;
        }
    }

    fn normalize(&mut self) {
        self.command = self.command.trim().to_string();
        if self.command.is_empty() {
            self.command = "clang-format".to_string();
        }
        self.args = self.args.iter().map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiagnosticsSettings {
    pub on_type: bool,
    pub on_save: bool,
    pub debounce_ms: u64,
    pub scope: DiagnosticsScope,
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
    fn apply_patch(
        &mut self,
        patch: DiagnosticsSettingsPatch,
    ) {
        if let Some(v) = patch.on_type {
            self.on_type = v;
        }
        if let Some(v) = patch.on_save {
            self.on_save = v;
        }
        if let Some(v) = patch.debounce_ms {
            self.debounce_ms = v;
        }
        if let Some(v) = patch.scope {
            self.scope = v;
        }
    }

    fn normalize(&mut self) {
        self.debounce_ms = self.debounce_ms.clamp(MIN_DIAGNOSTIC_DEBOUNCE_MS, MAX_DIAGNOSTIC_DEBOUNCE_MS);
    }
}

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
    fn apply_patch(
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

    fn normalize(&mut self) {
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

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CompilerSettings {
    pub include_paths: Vec<String>,
    pub extra_flags: Vec<String>,
    pub platform: CompilerPlatform,
}

impl CompilerSettings {
    fn apply_patch(
        &mut self,
        patch: CompilerSettingsPatch,
    ) {
        if let Some(v) = patch.include_paths {
            self.include_paths = v;
        }
        if let Some(v) = patch.extra_flags {
            self.extra_flags = v;
        }
        if let Some(v) = patch.platform {
            self.platform = CompilerPlatform::from_setting_value(&v);
        }
    }

    fn normalize(&mut self) {
        self.include_paths =
            self.include_paths.iter().map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect();
        self.extra_flags = self.extra_flags.iter().map(|f| f.trim().to_string()).filter(|f| !f.is_empty()).collect();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoggingSettings {
    pub level: LogLevel,
}

impl Default for LoggingSettings {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
        }
    }
}

impl LoggingSettings {
    fn apply_patch(
        &mut self,
        patch: LoggingSettingsPatch,
    ) {
        if let Some(v) = patch.level {
            self.level = v;
        }
    }
}

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

    fn apply_patch(
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

    fn normalize(&mut self) {
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

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct FormattingSettingsPatch {
    enable: Option<bool>,
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
    enable: Option<bool>,
    concurrency: Option<usize>,
    max_file_size_kb: Option<u64>,
    project_graph_depth: Option<usize>,
    project_graph_max_nodes: Option<usize>,
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
    level: Option<LogLevel>,
    #[serde(flatten)]
    _extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ThreadPoolSettingsPatch {
    worker_threads: Option<usize>,
    formatting_threads: Option<usize>,
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

/// Return the full list of schema fields for every setting.
pub fn schema_fields() -> Vec<SchemaField> {
    vec![
        SchemaField {
            key: "formatting.enable".into(),
            description: "Enable LSP-backed document formatting.".into(),
            schema_type: SchemaType::Bool,
            default: Value::Bool(true),
        },
        SchemaField {
            key: "formatting.command".into(),
            description: "Formatting executable used by metal-analyzer.".into(),
            schema_type: SchemaType::String,
            default: Value::String("clang-format".into()),
        },
        SchemaField {
            key: "formatting.args".into(),
            description: "Additional arguments passed to the formatting command.".into(),
            schema_type: SchemaType::StringArray,
            default: Value::Array(vec![]),
        },
        SchemaField {
            key: "diagnostics.onType".into(),
            description: "Run diagnostics while typing.".into(),
            schema_type: SchemaType::Bool,
            default: Value::Bool(true),
        },
        SchemaField {
            key: "diagnostics.onSave".into(),
            description: "Run diagnostics when a document is saved.".into(),
            schema_type: SchemaType::Bool,
            default: Value::Bool(true),
        },
        SchemaField {
            key: "diagnostics.debounceMs".into(),
            description: "Debounce delay for on-type diagnostics and background indexing work.".into(),
            schema_type: SchemaType::Integer {
                minimum: Some(MIN_DIAGNOSTIC_DEBOUNCE_MS as i64),
                maximum: Some(MAX_DIAGNOSTIC_DEBOUNCE_MS as i64),
            },
            default: Value::Number(500.into()),
        },
        SchemaField {
            key: "diagnostics.scope".into(),
            description: "Diagnostics scope. `openFiles` analyzes documents as they are opened/edited/saved. `workspace` also analyzes all `.metal` files in the workspace at startup and when settings change.".into(),
            schema_type: SchemaType::StringEnum {
                values: vec!["openFiles", "workspace"],
            },
            default: Value::String("openFiles".into()),
        },
        SchemaField {
            key: "indexing.enable".into(),
            description: "Enable background workspace indexing.".into(),
            schema_type: SchemaType::Bool,
            default: Value::Bool(true),
        },
        SchemaField {
            key: "indexing.concurrency".into(),
            description: "Maximum number of concurrent background indexing jobs.".into(),
            schema_type: SchemaType::Integer {
                minimum: Some(MIN_INDEXING_CONCURRENCY as i64),
                maximum: Some(MAX_INDEXING_CONCURRENCY as i64),
            },
            default: Value::Number(1.into()),
        },
        SchemaField {
            key: "indexing.maxFileSizeKb".into(),
            description: "Skip workspace files larger than this size during background indexing.".into(),
            schema_type: SchemaType::Integer {
                minimum: Some(MIN_MAX_FILE_SIZE_KB as i64),
                maximum: Some(MAX_MAX_FILE_SIZE_KB as i64),
            },
            default: Value::Number(512.into()),
        },
        SchemaField {
            key: "indexing.projectGraphDepth".into(),
            description: "Maximum include-graph traversal depth for scoped cross-file go-to-definition fallback."
                .into(),
            schema_type: SchemaType::Integer {
                minimum: Some(MIN_PROJECT_GRAPH_DEPTH as i64),
                maximum: Some(MAX_PROJECT_GRAPH_DEPTH as i64),
            },
            default: Value::Number(3.into()),
        },
        SchemaField {
            key: "indexing.projectGraphMaxNodes".into(),
            description:
                "Maximum number of graph nodes considered during scoped cross-file go-to-definition fallback.".into(),
            schema_type: SchemaType::Integer {
                minimum: Some(MIN_PROJECT_GRAPH_MAX_NODES as i64),
                maximum: Some(MAX_PROJECT_GRAPH_MAX_NODES as i64),
            },
            default: Value::Number(256.into()),
        },
        SchemaField {
            key: "indexing.excludePaths".into(),
            description: "Workspace paths to skip during background scanning. Relative paths are resolved from each workspace root; absolute paths are also supported. Excluded folders are skipped for both indexing and workspace-scope diagnostics.".into(),
            schema_type: SchemaType::StringArray,
            default: Value::Array(vec![]),
        },
        SchemaField {
            key: "compiler.includePaths".into(),
            description: "Extra include directories passed to the Metal compiler.".into(),
            schema_type: SchemaType::StringArray,
            default: Value::Array(vec![]),
        },
        SchemaField {
            key: "compiler.extraFlags".into(),
            description: "Extra compiler flags passed to `xcrun metal`.".into(),
            schema_type: SchemaType::StringArray,
            default: Value::Array(vec![]),
        },
        SchemaField {
            key: "compiler.platform".into(),
            description: "Target platform for Metal diagnostics. Determines which platform define (e.g. `__METAL_MACOS__`) is injected unless platform flags are already present in extra flags.".into(),
            schema_type: SchemaType::StringEnum {
                values: vec!["macos", "ios", "tvos", "watchos", "xros"],
            },
            default: Value::String("macos".into()),
        },
        SchemaField {
            key: "logging.level".into(),
            description: "Runtime logging verbosity for metal-analyzer.".into(),
            schema_type: SchemaType::StringEnum {
                values: vec!["error", "warn", "info", "debug", "trace"],
            },
            default: Value::String("info".into()),
        },
        SchemaField {
            key: "threadPool.workerThreads".into(),
            description: "Worker thread pool size. `0` uses `available_parallelism`. Requires restart.".into(),
            schema_type: SchemaType::Integer {
                minimum: Some(0),
                maximum: Some(MAX_WORKER_THREADS as i64),
            },
            default: Value::Number(0.into()),
        },
        SchemaField {
            key: "threadPool.formattingThreads".into(),
            description: "Formatting thread pool size. Requires restart.".into(),
            schema_type: SchemaType::Integer {
                minimum: Some(MIN_FORMATTING_THREADS as i64),
                maximum: Some(MAX_FORMATTING_THREADS as i64),
            },
            default: Value::Number(1.into()),
        },
    ]
}

/// Generate the `"properties"` object for the VS Code `contributes.configuration` section.
pub fn generate_package_json_properties() -> Value {
    let mut properties = serde_json::Map::new();

    properties.insert(
        "metal-analyzer.serverPath".into(),
        serde_json::json!({
            "type": "string",
            "default": "metal-analyzer",
            "markdownDescription": "Path to the metal-analyzer binary. The default value uses PATH first, then auto-downloads the latest macOS release binary."
        }),
    );

    for field in schema_fields() {
        let full_key = format!("metal-analyzer.{}", field.key);
        properties.insert(full_key, field.to_schema_value());
    }

    Value::Object(properties)
}

/// Generate markdown documentation for all settings.
pub fn generate_configuration_markdown() -> String {
    let mut out = String::new();
    let fields = schema_fields();

    let mut current_section = String::new();
    for field in &fields {
        let section = field.key.split('.').next().unwrap_or("");
        if section != current_section {
            current_section = section.to_string();
            let title = match section {
                "formatting" => "Formatting",
                "diagnostics" => "Diagnostics",
                "indexing" => "Indexing",
                "compiler" => "Compiler",
                "logging" => "Logging",
                "threadPool" => "Thread Pool",
                other => other,
            };
            out.push_str(&format!("\n## {title}\n\n"));
        }
        out.push_str(&field.to_markdown());
        out.push('\n');
    }

    out
}
