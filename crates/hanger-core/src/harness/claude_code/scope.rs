//! What applies in one place: the user's global setup, or one project.
//!
//! Claude Code reads `enabledPlugins` from the user's settings, then the project's shared
//! `.claude/settings.json`, then the person's own `.claude/settings.local.json`; the last one that
//! mentions a plugin decides. A plugin applies when it is installed for the user, or for this
//! project. `claude plugin list` reports the same.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{ClaudeCode, INSTALLED_PLUGINS, USER_SETTINGS, enabled_plugin_settings, read_json};
use crate::Result;
use crate::harness::HarnessAdapter;
use crate::marketplace::{PluginPreview, ScanWarning, standalone};

/// Where Claude Code installs a plugin, and which settings file switches it on or off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// For the user, in every project: `~/.claude/settings.json`.
    User,
    /// For everyone who works on the project: its `.claude/settings.json`, in version control.
    Project,
    /// For the person, in one project: its `.claude/settings.local.json`.
    Local,
}

impl Scope {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Project => "project",
            Self::Local => "local",
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeState {
    /// The project, or `None` for the global setup.
    pub project: Option<PathBuf>,
    pub plugins: Vec<ScopedPlugin>,
    /// Skills, agents, and commands outside plugins: in `~/.claude` for the global setup, in the
    /// project's `.claude` for a project. `None` when there are none.
    pub standalone: Option<PluginPreview>,
    /// Problems with the standalone components, such as broken links.
    pub warnings: Vec<ScanWarning>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopedPlugin {
    /// `plugin@marketplace`.
    pub id: String,
    pub name: String,
    pub marketplace: String,
    /// Where it is installed. A plugin installed in more than one scope shows the most specific.
    pub installed_in: Scope,
    /// Whether Claude Code loads it here.
    pub enabled: bool,
    /// The settings that decided `enabled`. `None` when none mention it, which leaves it off.
    pub decided_by: Option<Scope>,
    pub version: Option<String>,
    pub description: Option<String>,
}

impl ClaudeCode {
    /// The plugins and standalone components that apply globally, or in `project`.
    pub fn scope_state(&self, project: Option<&Path>) -> Result<ScopeState> {
        let mut state = ScopeState {
            project: project.map(Path::to_owned),
            plugins: Vec::new(),
            standalone: None,
            warnings: Vec::new(),
        };

        let standalone_dir = match project {
            Some(project) => Some(project.join(".claude")),
            None => self.standalone_dir(),
        };
        if let Some(dir) = standalone_dir.filter(|dir| dir.is_dir()) {
            let preview = standalone(&dir, self.id(), self.name())?;
            state.warnings = preview.warnings;
            state.standalone = preview.plugins.into_iter().next().filter(|components| {
                !(components.skills.is_empty()
                    && components.agents.is_empty()
                    && components.commands.is_empty())
            });
        }

        let Some(config) = self.existing_config_dir() else {
            return Ok(state);
        };
        let mut layers = vec![(Scope::User, settings_at(&config.join(USER_SETTINGS))?)];
        if let Some(project) = project {
            layers.push((
                Scope::Project,
                settings_at(&project.join(".claude/settings.json"))?,
            ));
            layers.push((
                Scope::Local,
                settings_at(&project.join(".claude/settings.local.json"))?,
            ));
        }

        let installed = read_json(&config.join(INSTALLED_PLUGINS))?;
        let plugins = installed
            .as_ref()
            .and_then(|file| file.get("plugins"))
            .and_then(Value::as_object);
        for (id, installs) in plugins.into_iter().flatten() {
            let installs = match installs {
                Value::Array(installs) => installs.iter().collect(),
                install => vec![install],
            };
            let Some((installed_in, install)) = installs
                .into_iter()
                .filter_map(|install| applies_in(install, project).map(|scope| (scope, install)))
                .max_by_key(|(scope, _)| *scope)
            else {
                continue;
            };
            let (enabled, decided_by) = layers
                .iter()
                .rev()
                .find_map(|(scope, settings)| {
                    settings.get(id).map(|enabled| (*enabled, Some(*scope)))
                })
                .unwrap_or((false, None));
            let (name, marketplace) = id.split_once('@').unwrap_or((id, ""));
            let text = |key: &str| install.get(key).and_then(Value::as_str);
            state.plugins.push(ScopedPlugin {
                id: id.clone(),
                name: name.to_owned(),
                marketplace: marketplace.to_owned(),
                installed_in,
                enabled,
                decided_by,
                version: text("version").map(str::to_owned),
                description: text("installPath").and_then(|path| description_of(Path::new(path))),
            });
        }
        state
            .plugins
            .sort_by(|a, b| (&a.name, &a.marketplace).cmp(&(&b.name, &b.marketplace)));
        Ok(state)
    }
}

/// The scope in which `install` applies here, if it does. An install without a scope is the
/// user's, as in older files.
fn applies_in(install: &Value, project: Option<&Path>) -> Option<Scope> {
    let scope = match install.get("scope").and_then(Value::as_str) {
        None | Some("user") => return Some(Scope::User),
        Some("project") => Scope::Project,
        Some("local") => Scope::Local,
        Some(_) => return None,
    };
    let install_path = install.get("projectPath").and_then(Value::as_str)?;
    project
        .is_some_and(|project| same_dir(Path::new(install_path), project))
        .then_some(scope)
}

pub(super) fn same_dir(a: &Path, b: &Path) -> bool {
    a == b
        || matches!(
            (fs::canonicalize(a), fs::canonicalize(b)),
            (Ok(a), Ok(b)) if a == b
        )
}

fn settings_at(path: &Path) -> Result<BTreeMap<String, bool>> {
    Ok(read_json(path)?
        .map(|settings| enabled_plugin_settings(&settings))
        .unwrap_or_default())
}

/// The description in an installed plugin's `plugin.json`. A missing or bad file has none.
fn description_of(install_path: &Path) -> Option<String> {
    let manifest = read_json(&install_path.join(".claude-plugin/plugin.json")).ok()??;
    manifest
        .get("description")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    /// A Claude Code config with plugins for the user, for `project`, and for another project.
    fn fixture(tmp: &Path) -> (PathBuf, PathBuf) {
        let config = tmp.join("claude");
        let project = tmp.join("project");
        let cached = tmp.join("cache/a");
        write(
            &cached.join(".claude-plugin/plugin.json"),
            r#"{ "name": "a", "description": "Plugin A." }"#,
        );
        let installed = serde_json::json!({ "version": 2, "plugins": {
            "a@m": [{ "scope": "user", "version": "1.0.0", "installPath": cached }],
            "b@m": [{ "scope": "user" }],
            "c@m": [{ "scope": "user" }],
            "p@m": [{ "scope": "project", "projectPath": project }],
            "q@m": [{ "scope": "local", "projectPath": project }],
            "r@m": [{ "scope": "project", "projectPath": tmp.join("other") }]
        } });
        write(&config.join(INSTALLED_PLUGINS), &installed.to_string());
        write(
            &config.join(USER_SETTINGS),
            r#"{ "env": { "TOKEN": "secret" }, "enabledPlugins": { "a@m": true, "b@m": true, "c@m": false } }"#,
        );
        write(
            &project.join(".claude/settings.json"),
            r#"{ "enabledPlugins": { "b@m": false, "p@m": true } }"#,
        );
        write(
            &project.join(".claude/settings.local.json"),
            r#"{ "enabledPlugins": { "c@m": true, "q@m": true } }"#,
        );
        write(&config.join("skills/global/SKILL.md"), "Global.");
        write(&project.join(".claude/skills/local/SKILL.md"), "Local.");
        (config, project)
    }

    fn summary(state: &ScopeState) -> Vec<(&str, Scope, bool, Option<Scope>)> {
        state
            .plugins
            .iter()
            .map(|p| (p.name.as_str(), p.installed_in, p.enabled, p.decided_by))
            .collect()
    }

    #[test]
    fn the_global_setup_has_the_user_plugins_and_skills() {
        let tmp = tempfile::tempdir().unwrap();
        let (config, _) = fixture(tmp.path());

        let state = ClaudeCode::new(&config).scope_state(None).unwrap();

        assert_eq!(
            summary(&state),
            [
                ("a", Scope::User, true, Some(Scope::User)),
                ("b", Scope::User, true, Some(Scope::User)),
                ("c", Scope::User, false, Some(Scope::User)),
            ]
        );
        assert_eq!(state.plugins[0].version.as_deref(), Some("1.0.0"));
        assert_eq!(state.plugins[0].description.as_deref(), Some("Plugin A."));
        assert_eq!(state.standalone.unwrap().skills[0].name, "global");
    }

    #[test]
    fn a_project_layers_its_settings_over_the_user_settings() {
        let tmp = tempfile::tempdir().unwrap();
        let (config, project) = fixture(tmp.path());

        let state = ClaudeCode::new(&config)
            .scope_state(Some(&project))
            .unwrap();

        assert_eq!(
            summary(&state),
            [
                ("a", Scope::User, true, Some(Scope::User)),
                // Off for everyone in the project.
                ("b", Scope::User, false, Some(Scope::Project)),
                // Off for the user, but on for this person in this project.
                ("c", Scope::User, true, Some(Scope::Local)),
                ("p", Scope::Project, true, Some(Scope::Project)),
                ("q", Scope::Local, true, Some(Scope::Local)),
            ]
        );
        assert_eq!(state.standalone.unwrap().skills[0].name, "local");
    }

    #[cfg(unix)]
    #[test]
    fn matches_an_install_recorded_through_a_symlinked_path() {
        let tmp = tempfile::tempdir().unwrap();
        let (config, project) = fixture(tmp.path());
        let link = tmp.path().join("link");
        std::os::unix::fs::symlink(&project, &link).unwrap();
        let installed = serde_json::json!({ "plugins": {
            "p@m": [{ "scope": "project", "projectPath": link }]
        } });
        write(&config.join(INSTALLED_PLUGINS), &installed.to_string());

        let state = ClaudeCode::new(&config)
            .scope_state(Some(&project))
            .unwrap();

        assert_eq!(
            summary(&state),
            [("p", Scope::Project, true, Some(Scope::Project))]
        );
    }

    #[test]
    fn a_project_without_settings_has_the_user_plugins() {
        let tmp = tempfile::tempdir().unwrap();
        let (config, _) = fixture(tmp.path());
        let bare = tmp.path().join("bare");
        fs::create_dir_all(&bare).unwrap();

        let state = ClaudeCode::new(&config).scope_state(Some(&bare)).unwrap();

        assert_eq!(state.plugins.len(), 3);
        assert!(state.standalone.is_none());
    }
}
