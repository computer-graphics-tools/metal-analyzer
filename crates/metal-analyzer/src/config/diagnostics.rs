use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

pub const MIN_DIAGNOSTIC_DEBOUNCE_MS: u64 = 50;
pub const MAX_DIAGNOSTIC_DEBOUNCE_MS: u64 = 5000;

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
    pub(crate) fn apply_patch(
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

    pub(crate) fn normalize(&mut self) {
        self.debounce_ms = self.debounce_ms.clamp(MIN_DIAGNOSTIC_DEBOUNCE_MS, MAX_DIAGNOSTIC_DEBOUNCE_MS);
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct DiagnosticsSettingsPatch {
    pub(crate) on_type: Option<bool>,
    pub(crate) on_save: Option<bool>,
    pub(crate) debounce_ms: Option<u64>,
    pub(crate) scope: Option<DiagnosticsScope>,
    #[serde(flatten)]
    pub(crate) _extra: HashMap<String, Value>,
}
