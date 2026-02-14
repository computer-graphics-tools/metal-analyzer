use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

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
    pub(crate) fn apply_patch(
        &mut self,
        patch: LoggingSettingsPatch,
    ) {
        if let Some(v) = patch.level {
            self.level = v;
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct LoggingSettingsPatch {
    pub(crate) level: Option<LogLevel>,
    #[serde(flatten)]
    pub(crate) _extra: HashMap<String, Value>,
}
