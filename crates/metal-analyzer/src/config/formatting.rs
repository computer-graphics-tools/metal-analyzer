use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

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
    pub(crate) fn apply_patch(
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

    pub(crate) fn normalize(&mut self) {
        self.command = self.command.trim().to_string();
        if self.command.is_empty() {
            self.command = "clang-format".to_string();
        }
        self.args = self.args.iter().map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect();
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct FormattingSettingsPatch {
    pub(crate) enable: Option<bool>,
    pub(crate) command: Option<String>,
    pub(crate) args: Option<Vec<String>>,
    #[serde(flatten)]
    pub(crate) _extra: HashMap<String, Value>,
}
