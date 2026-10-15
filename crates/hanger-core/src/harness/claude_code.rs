//! The Claude Code adapter, and what Claude Code's own files say about its plugins.
//!
//! Read only: nothing here changes Claude Code's files. The status rule matches what
//! `claude plugin list` reports for the user scope: a plugin is enabled when it is installed for
//! the user and `enabledPlugins` in the user's `settings.json` sets it to `true`.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use super::{Discovery, FoundMarketplace, FoundStandalone, HarnessAdapter, PluginStatus};
use crate::marketplace::MarketplaceSource;
use crate::{Error, Result, paths};

const INSTALLED_PLUGINS: &str = "plugins/installed_plugins.json";
const KNOWN_MARKETPLACES: &str = "plugins/known_marketplaces.json";
const USER_SETTINGS: &str = "settings.json";

/// Reads what Claude Code has from its config directory.
pub struct ClaudeCode {
    config_dir: Option<PathBuf>,
}

impl ClaudeCode {
    /// Uses `$CLAUDE_CONFIG_DIR`, else `~/.claude`.
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            config_dir: paths::claude_config_dir().ok(),
        }
    }

    /// Uses `config_dir`, such as a fixture in tests.
    #[must_use]
    pub fn new(config_dir: impl Into<PathBuf>) -> Self {
        Self {
            config_dir: Some(config_dir.into()),
        }
    }

    fn existing_config_dir(&self) -> Option<&Path> {
        self.config_dir.as_deref().filter(|dir| dir.is_dir())
    }
}

impl HarnessAdapter for ClaudeCode {
    fn id(&self) -> &'static str {
        "claude-code"
    }

    fn name(&self) -> &'static str {
        "Claude Code"
    }

    fn standalone_dir(&self) -> Option<PathBuf> {
        self.existing_config_dir().map(Path::to_owned)
    }

    fn discover(&self) -> Result<Discovery> {
        let mut discovery = Discovery {
            harness: self.id(),
            name: self.name(),
            config_dir: None,
            marketplaces: Vec::new(),
            standalone: None,
            unmatched_plugins: Vec::new(),
        };
        let Some(dir) = self.existing_config_dir() else {
            return Ok(discovery);
        };
        discovery.config_dir = Some(dir.to_owned());

        // Name to source and the copy that Claude Code keeps.
        let mut declared: BTreeMap<String, (Value, Option<PathBuf>)> = BTreeMap::new();
        let known = read_json(&dir.join(KNOWN_MARKETPLACES))?;
        for (name, entry) in known
            .as_ref()
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            let location = entry
                .get("installLocation")
                .and_then(Value::as_str)
                .map(PathBuf::from);
            declared.insert(name.clone(), (source_of(entry), location));
        }
        // Declared in settings, but perhaps not fetched by Claude Code yet. Only this key and
        // `enabledPlugins` are read: the rest of the settings can hold secrets.
        let settings = read_json(&dir.join(USER_SETTINGS))?;
        let extra = settings
            .as_ref()
            .and_then(|settings| settings.get("extraKnownMarketplaces"))
            .and_then(Value::as_object);
        for (name, entry) in extra.into_iter().flatten() {
            declared
                .entry(name.clone())
                .or_insert_with(|| (source_of(entry), None));
        }

        let installed = read_json(&dir.join(INSTALLED_PLUGINS))?;
        let mut installed_by_marketplace: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for id in installed
            .as_ref()
            .and_then(|file| file.get("plugins"))
            .and_then(Value::as_object)
            .into_iter()
            .flat_map(|plugins| plugins.keys())
        {
            if let Some((plugin, marketplace)) = id.split_once('@') {
                installed_by_marketplace
                    .entry(marketplace.to_owned())
                    .or_default()
                    .insert(plugin.to_owned());
            }
        }

        for (name, (source, location)) in declared {
            let (source, problem) = match marketplace_source(&source) {
                Ok(source) => (Some(source), None),
                Err(problem) => (None, Some(problem)),
            };
            let local_copy = location
                .filter(|_| matches!(source, Some(MarketplaceSource::GitHub { .. })))
                .filter(|location| location.is_absolute() && location.join(".git").exists());
            let installed_plugins = installed_by_marketplace
                .remove(&name)
                .map(|plugins| plugins.into_iter().collect())
                .unwrap_or_default();
            discovery.marketplaces.push(FoundMarketplace {
                name,
                source,
                problem,
                local_copy,
                installed_plugins,
                in_catalog: false,
            });
        }
        discovery.unmatched_plugins = installed_by_marketplace
            .into_iter()
            .flat_map(|(marketplace, plugins)| {
                plugins
                    .into_iter()
                    .map(move |plugin| format!("{plugin}@{marketplace}"))
            })
            .collect();

        if let Some(Ok(preview)) = self.standalone()
            && let Some(components) = preview.plugins.first()
            && !(components.skills.is_empty()
                && components.agents.is_empty()
                && components.commands.is_empty())
        {
            discovery.standalone = Some(FoundStandalone {
                source: MarketplaceSource::Harness {
                    harness: self.id().to_owned(),
                },
                path: dir.to_owned(),
                skills: components.skills.len(),
                agents: components.agents.len(),
                commands: components.commands.len(),
                in_catalog: false,
            });
        }
        Ok(discovery)
    }
}

fn source_of(entry: &Value) -> Value {
    entry.get("source").cloned().unwrap_or(Value::Null)
}

/// How the catalog can add a marketplace that Claude Code describes with `source`, or why not.
fn marketplace_source(source: &Value) -> Result<MarketplaceSource, String> {
    let field = |name: &str| source.get(name).and_then(Value::as_str);
    let github = |text: Option<&str>| match text.map(MarketplaceSource::parse) {
        Some(Ok(source @ MarketplaceSource::GitHub { .. })) => Some(source),
        _ => None,
    };
    match field("source").unwrap_or("unknown") {
        "github" => {
            github(field("repo")).ok_or_else(|| "its GitHub repository is not valid".into())
        }
        "git" => github(field("url"))
            .ok_or_else(|| "only repositories on GitHub can be cloned so far".into()),
        "directory" => match field("path").map(Path::new) {
            Some(path) if path.is_absolute() && path.is_dir() => Ok(MarketplaceSource::Local {
                path: path.to_owned(),
            }),
            _ => Err("its directory does not exist".into()),
        },
        kind => Err(format!(
            "a marketplace from a {kind} source cannot be added yet"
        )),
    }
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

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    /// A config directory with marketplaces of every kind of source, settings, installed plugins,
    /// and standalone components. Returns it and the copy and the local marketplace it points at.
    fn claude_fixture(tmp: &Path) -> (PathBuf, PathBuf, PathBuf) {
        let config = tmp.join("claude");
        let copy = config.join("plugins/marketplaces/acme");
        fs::create_dir_all(copy.join(".git")).unwrap();
        let local = tmp.join("local-market");
        fs::create_dir_all(&local).unwrap();
        let known = serde_json::json!({
            "acme": { "source": { "source": "github", "repo": "Acme/Tools" }, "installLocation": copy },
            "omc": { "source": { "source": "git", "url": "https://github.com/Yeachan-Heo/oh-my-claudecode.git" } },
            "self-hosted": { "source": { "source": "git", "url": "https://gitlab.com/x/y.git" } },
            "local": { "source": { "source": "directory", "path": local } },
            "gone": { "source": { "source": "directory", "path": tmp.join("gone") } },
            "packaged": { "source": { "source": "npm", "package": "x" } }
        });
        write(&config.join(KNOWN_MARKETPLACES), &known.to_string());
        write(
            &config.join(USER_SETTINGS),
            r#"{ "env": { "TOKEN": "secret" }, "extraKnownMarketplaces": {
                "pending": { "source": { "source": "github", "repo": "acme/pending" } },
                "acme": { "source": { "source": "github", "repo": "acme/other" } }
            } }"#,
        );
        write(
            &config.join(INSTALLED_PLUGINS),
            r#"{ "version": 2, "plugins": {
                "b@acme": [{ "scope": "project" }], "a@acme": [{ "scope": "user" }],
                "c@local": [{ "scope": "user" }], "linear@hosted": [{ "scope": "user" }]
            } }"#,
        );
        write(
            &config.join("skills/one/SKILL.md"),
            "---\ndescription: One.\n---\n",
        );
        write(
            &config.join("agents/reviewer.md"),
            "---\nname: reviewer\n---\n",
        );
        #[cfg(unix)]
        {
            let elsewhere = tmp.join("elsewhere/linked");
            write(
                &elsewhere.join("SKILL.md"),
                "---\ndescription: Linked.\n---\n",
            );
            std::os::unix::fs::symlink(&elsewhere, config.join("skills/linked")).unwrap();
        }

        (config, copy, local)
    }

    #[test]
    fn discovers_marketplaces_installed_plugins_and_standalone_components() {
        let tmp = tempfile::tempdir().unwrap();
        let (config, copy, local) = claude_fixture(tmp.path());

        let discovery = ClaudeCode::new(&config).discover().unwrap();

        assert_eq!(discovery.harness, "claude-code");
        assert_eq!(discovery.config_dir.as_deref(), Some(config.as_path()));
        let names: Vec<_> = discovery
            .marketplaces
            .iter()
            .map(|m| m.name.as_str())
            .collect();
        assert_eq!(
            names,
            [
                "acme",
                "gone",
                "local",
                "omc",
                "packaged",
                "pending",
                "self-hosted"
            ]
        );
        let find = |name: &str| {
            discovery
                .marketplaces
                .iter()
                .find(|m| m.name == name)
                .unwrap()
        };
        let github = |owner: &str, repo: &str| MarketplaceSource::GitHub {
            owner: owner.into(),
            repo: repo.into(),
        };
        // known_marketplaces.json wins over the settings for the same name.
        assert_eq!(find("acme").source, Some(github("acme", "tools")));
        assert_eq!(find("acme").local_copy.as_deref(), Some(copy.as_path()));
        assert_eq!(find("acme").installed_plugins, ["a", "b"]);
        assert_eq!(
            find("omc").source,
            Some(github("yeachan-heo", "oh-my-claudecode"))
        );
        assert_eq!(find("omc").local_copy, None);
        assert_eq!(find("pending").source, Some(github("acme", "pending")));
        assert_eq!(
            find("local").source,
            Some(MarketplaceSource::Local {
                path: local.clone()
            })
        );
        assert_eq!(find("local").installed_plugins, ["c"]);
        for (name, problem) in [
            ("self-hosted", "only repositories on GitHub"),
            ("gone", "its directory does not exist"),
            ("packaged", "from a npm source"),
        ] {
            assert_eq!(find(name).source, None);
            assert!(
                find(name)
                    .problem
                    .as_deref()
                    .is_some_and(|p| p.contains(problem)),
                "{name}: {:?}",
                find(name).problem
            );
        }
        assert_eq!(discovery.unmatched_plugins, ["linear@hosted"]);

        let standalone = discovery.standalone.unwrap();
        assert_eq!(
            standalone.source,
            MarketplaceSource::Harness {
                harness: "claude-code".into()
            }
        );
        // The symlinked skill leads outside the directory, and it still counts.
        assert_eq!(standalone.skills, if cfg!(unix) { 2 } else { 1 });
        assert_eq!(standalone.agents, 1);
        assert_eq!(standalone.commands, 0);
    }

    #[test]
    fn discovers_nothing_when_claude_code_is_not_on_this_machine() {
        let tmp = tempfile::tempdir().unwrap();

        let discovery = ClaudeCode::new(tmp.path().join("missing"))
            .discover()
            .unwrap();

        assert_eq!(discovery.config_dir, None);
        assert!(discovery.marketplaces.is_empty() && discovery.standalone.is_none());
    }

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
