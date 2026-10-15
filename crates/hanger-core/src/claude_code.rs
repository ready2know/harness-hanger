//! What Claude Code's own files say about the plugins it has installed.
//!
//! Read only. The rule matches what `claude plugin list` reports for the user scope: a plugin is
//! enabled when it is installed for the user and `enabledPlugins` in the user's `settings.json`
//! sets it to `true`.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Error, Result};

const INSTALLED_PLUGINS: &str = "plugins/installed_plugins.json";
const USER_SETTINGS: &str = "settings.json";

/// Whether Claude Code loads a plugin for the user, in every project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PluginStatus {
    Enabled,
    /// Installed for the user, but turned off.
    Disabled,
    /// Not installed for the user. It can still be installed in a project.
    NotInstalled,
}

/// The plugins installed for the user, by ID (`plugin@marketplace`).
#[derive(Debug, Default)]
pub struct UserPlugins {
    installed: HashSet<String>,
    enabled: HashSet<String>,
}

impl UserPlugins {
    /// Reads the user's plugin state from a Claude Code config directory, such as `~/.claude`.
    /// Missing files mean that nothing is installed.
    pub fn read(config_dir: &Path) -> Result<Self> {
        let installed = read_json(&config_dir.join(INSTALLED_PLUGINS))?
            .map(|file| user_installs(&file))
            .unwrap_or_default();
        let enabled = read_json(&config_dir.join(USER_SETTINGS))?
            .map(|file| enabled_plugins(&file))
            .unwrap_or_default();
        Ok(Self { installed, enabled })
    }

    /// The status of `plugin` from the marketplace that Claude Code knows as `marketplace`.
    #[must_use]
    pub fn status(&self, plugin: &str, marketplace: &str) -> PluginStatus {
        let id = format!("{plugin}@{marketplace}");
        if !self.installed.contains(&id) {
            PluginStatus::NotInstalled
        } else if self.enabled.contains(&id) {
            PluginStatus::Enabled
        } else {
            PluginStatus::Disabled
        }
    }
}

fn read_json(path: &Path) -> Result<Option<Value>> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(Error::io(path, error)),
    };
    serde_json::from_str(&text).map(Some).map_err(|error| {
        Error::io(
            path,
            io::Error::new(io::ErrorKind::InvalidData, error.to_string()),
        )
    })
}

/// IDs with an install in the user scope. Each ID maps to a list of installs; an older format maps
/// it to one install, and an install without a scope belongs to the user.
fn user_installs(file: &Value) -> HashSet<String> {
    let Some(plugins) = file.get("plugins").and_then(Value::as_object) else {
        return HashSet::new();
    };
    plugins
        .iter()
        .filter(|(_, installs)| has_user_install(installs))
        .map(|(id, _)| id.clone())
        .collect()
}

fn has_user_install(installs: &Value) -> bool {
    let is_user = |install: &Value| {
        install
            .get("scope")
            .and_then(Value::as_str)
            .is_none_or(|scope| scope == "user")
    };
    match installs {
        Value::Array(installs) => installs.iter().any(is_user),
        install => is_user(install),
    }
}

/// IDs that `enabledPlugins` sets to `true`. Only this key of the settings is read: the rest can
/// hold secrets, such as tokens in `env`.
fn enabled_plugins(settings: &Value) -> HashSet<String> {
    #[derive(Deserialize)]
    struct Settings {
        #[serde(rename = "enabledPlugins", default)]
        enabled_plugins: serde_json::Map<String, Value>,
    }
    Settings::deserialize(settings)
        .map(|settings| {
            settings
                .enabled_plugins
                .into_iter()
                .filter(|(_, enabled)| enabled == &Value::Bool(true))
                .map(|(id, _)| id)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_dir(installed: &str, settings: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("plugins")).unwrap();
        fs::write(dir.path().join(INSTALLED_PLUGINS), installed).unwrap();
        fs::write(dir.path().join(USER_SETTINGS), settings).unwrap();
        dir
    }

    #[test]
    fn a_plugin_is_enabled_when_installed_for_the_user_and_switched_on() {
        let dir = config_dir(
            r#"{ "version": 2, "plugins": {
                "on@market": [{ "scope": "user" }],
                "off@market": [{ "scope": "user" }],
                "unlisted@market": [{ "scope": "user" }],
                "project@market": [{ "scope": "project", "projectPath": "/p" }],
                "both@market": [{ "scope": "local" }, { "scope": "user" }]
            } }"#,
            r#"{ "env": { "TOKEN": "secret" }, "enabledPlugins": {
                "on@market": true,
                "off@market": false,
                "project@market": true,
                "both@market": true,
                "missing@market": true
            } }"#,
        );

        let plugins = UserPlugins::read(dir.path()).unwrap();

        assert_eq!(plugins.status("on", "market"), PluginStatus::Enabled);
        assert_eq!(plugins.status("both", "market"), PluginStatus::Enabled);
        assert_eq!(plugins.status("off", "market"), PluginStatus::Disabled);
        // Not listed in enabledPlugins: Claude Code reports these as disabled.
        assert_eq!(plugins.status("unlisted", "market"), PluginStatus::Disabled);
        // Enabled, but installed only in a project, or not at all.
        assert_eq!(
            plugins.status("project", "market"),
            PluginStatus::NotInstalled
        );
        assert_eq!(
            plugins.status("missing", "market"),
            PluginStatus::NotInstalled
        );
        assert_eq!(plugins.status("on", "other"), PluginStatus::NotInstalled);
    }

    #[test]
    fn reads_the_older_format_with_one_install_per_plugin() {
        let dir = config_dir(
            r#"{ "version": 1, "plugins": { "old@market": { "installPath": "/x" } } }"#,
            r#"{ "enabledPlugins": { "old@market": true } }"#,
        );

        let plugins = UserPlugins::read(dir.path()).unwrap();

        assert_eq!(plugins.status("old", "market"), PluginStatus::Enabled);
    }

    #[test]
    fn nothing_is_installed_without_the_files() {
        let dir = tempfile::tempdir().unwrap();

        let plugins = UserPlugins::read(dir.path()).unwrap();

        assert_eq!(plugins.status("any", "market"), PluginStatus::NotInstalled);
    }

    #[test]
    fn a_malformed_file_is_an_error_that_names_it() {
        let dir = config_dir("{ not json", "{}");

        let error = UserPlugins::read(dir.path()).unwrap_err();

        assert!(
            error.to_string().contains("installed_plugins.json"),
            "{error}"
        );
    }
}
