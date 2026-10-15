use std::path::PathBuf;

use serde::Serialize;

use crate::{VERSION, paths};

/// Basic facts about this installation, shown by `harness-hanger info` and the desktop app.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: &'static str,
    /// `None` when the directory cannot be resolved on this machine.
    pub claude_config_dir: Option<PathBuf>,
}

impl AppInfo {
    #[must_use]
    pub fn collect() -> Self {
        Self {
            version: VERSION,
            claude_config_dir: paths::claude_config_dir().ok(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_with_camel_case_keys() {
        let info = AppInfo {
            version: "1.2.3",
            claude_config_dir: Some("/home/u/.claude".into()),
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "version": "1.2.3", "claudeConfigDir": "/home/u/.claude" })
        );
    }
}
