use serde_json::Value;

use crate::config::{
    diagnostics::{MAX_DIAGNOSTIC_DEBOUNCE_MS, MIN_DIAGNOSTIC_DEBOUNCE_MS},
    indexing::{
        MAX_INDEXING_CONCURRENCY, MAX_MAX_FILE_SIZE_KB, MAX_PROJECT_GRAPH_DEPTH, MAX_PROJECT_GRAPH_MAX_NODES,
        MIN_INDEXING_CONCURRENCY, MIN_MAX_FILE_SIZE_KB, MIN_PROJECT_GRAPH_DEPTH, MIN_PROJECT_GRAPH_MAX_NODES,
    },
    thread_pool::{MAX_FORMATTING_THREADS, MAX_WORKER_THREADS, MIN_FORMATTING_THREADS},
};

/// One entry in the generated configuration schema.
#[derive(Debug, Clone)]
pub struct SchemaField {
    pub key: String,
    pub description: String,
    pub schema_type: SchemaType,
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

    pub fn to_markdown(&self) -> String {
        format!("- `metal-analyzer.{}` - {}", self.key, self.description)
    }
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
            description: "Diagnostics scope. `openFiles` analyzes documents as they are opened/edited/saved. \
                           `workspace` also analyzes all `.metal` files in the workspace at startup and when \
                           settings change."
                .into(),
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
            description: "Maximum number of graph nodes considered during scoped cross-file go-to-definition fallback."
                .into(),
            schema_type: SchemaType::Integer {
                minimum: Some(MIN_PROJECT_GRAPH_MAX_NODES as i64),
                maximum: Some(MAX_PROJECT_GRAPH_MAX_NODES as i64),
            },
            default: Value::Number(256.into()),
        },
        SchemaField {
            key: "indexing.excludePaths".into(),
            description: "Workspace paths to skip during background scanning. Relative paths are resolved from \
                           each workspace root; absolute paths are also supported. Excluded folders are skipped \
                           for both indexing and workspace-scope diagnostics."
                .into(),
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
            description: "Target platform for Metal diagnostics. Determines which platform define \
                           (e.g. `__METAL_MACOS__`) is injected unless platform flags are already present \
                           in extra flags."
                .into(),
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
