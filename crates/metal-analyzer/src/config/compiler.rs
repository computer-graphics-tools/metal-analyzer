use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

use crate::metal::compiler::CompilerPlatform;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CompilerSettings {
    pub include_paths: Vec<String>,
    pub extra_flags: Vec<String>,
    pub platform: CompilerPlatform,
}

impl CompilerSettings {
    pub(crate) fn apply_patch(
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

    pub(crate) fn normalize(&mut self) {
        self.include_paths =
            self.include_paths.iter().map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect();
        self.extra_flags = self.extra_flags.iter().map(|f| f.trim().to_string()).filter(|f| !f.is_empty()).collect();
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct CompilerSettingsPatch {
    pub(crate) include_paths: Option<Vec<String>>,
    pub(crate) extra_flags: Option<Vec<String>>,
    pub(crate) platform: Option<String>,
    #[serde(flatten)]
    pub(crate) _extra: HashMap<String, Value>,
}
