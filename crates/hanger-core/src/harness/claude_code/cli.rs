//! Changes to Claude Code's plugins, made through its own `claude plugin` command, so that
//! harness-hanger never edits Claude Code's files itself.

use std::env;
use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde_json::Value;

use super::scope::same_dir;
use super::{ClaudeCode, KNOWN_MARKETPLACES, Scope, ScopedPlugin, marketplace_source, read_json};
use crate::marketplace::MarketplaceSource;
use crate::{Error, Result, process};

/// A change to make with `claude plugin`. `plugin` is `plugin@marketplace`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginChange {
    Install {
        plugin: String,
        scope: Scope,
    },
    Uninstall {
        plugin: String,
        scope: Scope,
    },
    Enable {
        plugin: String,
        scope: Scope,
    },
    Disable {
        plugin: String,
        scope: Scope,
    },
    /// Declares a marketplace, by `owner/repo` or a path, so that its plugins can be installed.
    AddMarketplace {
        source: String,
        scope: Scope,
    },
}

impl PluginChange {
    fn args(&self) -> Vec<&str> {
        let (command, target, scope): (&[&str], &str, Scope) = match self {
            Self::Install { plugin, scope } => (&["install"], plugin, *scope),
            Self::Uninstall { plugin, scope } => (&["uninstall"], plugin, *scope),
            Self::Enable { plugin, scope } => (&["enable"], plugin, *scope),
            Self::Disable { plugin, scope } => (&["disable"], plugin, *scope),
            Self::AddMarketplace { source, scope } => (&["marketplace", "add"], source, *scope),
        };
        let mut args = vec!["plugin"];
        args.extend(command);
        // `--` keeps a path that starts with `-` from reading as an option.
        args.extend(["--scope", scope.as_str(), "--json", "--", target]);
        args
    }
}

/// One change at a time: `claude` writes settings files and clones marketplaces, and two runs at
/// once, such as two installs from one new marketplace, could get in each other's way.
static CHANGES: Mutex<()> = Mutex::new(());

fn one_at_a_time() -> MutexGuard<'static, ()> {
    CHANGES.lock().unwrap_or_else(PoisonError::into_inner)
}

impl ClaudeCode {
    /// Makes `change` in `project`, whose folder decides what the project and local scopes mean.
    pub fn change(&self, project: &Path, change: &PluginChange) -> Result<()> {
        let _guard = one_at_a_time();
        self.run(project, change)
    }

    fn run(&self, project: &Path, change: &PluginChange) -> Result<()> {
        let cli = self
            .cli
            .clone()
            .or_else(|| find_claude(env::var_os("PATH"), env::home_dir()))
            .ok_or(Error::ClaudeMissing)?;
        let mut command = process::background(cli);
        command.args(change.args()).current_dir(project);
        if self.pass_config_dir
            && let Some(dir) = &self.config_dir
        {
            command.env("CLAUDE_CONFIG_DIR", dir);
        }
        let output = command
            .output()
            .map_err(|error| Error::Harness(format!("cannot run claude: {error}")))?;
        outcome(&output)
    }

    /// Installs `plugin` from `marketplace` into `project`, and returns the scope it went into.
    ///
    /// `source` is where the catalog has the marketplace from. A marketplace from GitHub goes into
    /// the project scope, for everyone who works on it. A marketplace from a local folder goes
    /// into the person's own settings for the project, because the folder exists only on this
    /// machine. Either way the settings of that scope declare the marketplace, so that it travels
    /// with them.
    ///
    /// Refused when the plugin is installed for the user, since it applies in every project
    /// already (switch it off in the project instead), and when Claude Code knows another
    /// marketplace by this name, since it would install from that one.
    pub fn install_in_project(
        &self,
        project: &Path,
        plugin: &str,
        marketplace: &str,
        source: &MarketplaceSource,
    ) -> Result<Scope> {
        let _guard = one_at_a_time();
        let id = format!("{plugin}@{marketplace}");
        if self
            .scope_state(None)?
            .plugins
            .iter()
            .any(|installed| installed.id == id)
        {
            return Err(Error::Harness(format!(
                "{id} is installed for your user, so it applies in every project already; switch it off in this project instead"
            )));
        }
        if let Some(known) = self.known_source(marketplace)?
            && !same_source(&known, source)
        {
            return Err(Error::Harness(format!(
                "Claude Code knows a different marketplace named {marketplace} ({known}), and would install from that one"
            )));
        }
        let (declared, scope) = match source {
            MarketplaceSource::GitHub { owner, repo } => {
                (format!("{owner}/{repo}"), Scope::Project)
            }
            MarketplaceSource::Local { path } => (
                path.to_str()
                    .ok_or_else(|| Error::InvalidSource(path.display().to_string()))?
                    .to_owned(),
                Scope::Local,
            ),
            MarketplaceSource::Harness { .. } => {
                return Err(Error::Harness(format!(
                    "{marketplace} holds standalone components, not plugins"
                )));
            }
        };
        self.run(
            project,
            &PluginChange::AddMarketplace {
                source: declared,
                scope,
            },
        )?;
        self.run(project, &PluginChange::Install { plugin: id, scope })?;
        Ok(scope)
    }

    /// Switches `plugin` on or off in `project`, and checks that it took effect.
    ///
    /// A plugin installed for the user is switched in the person's own settings for the project,
    /// so the change reaches no one else. A plugin installed in the project is switched where it
    /// is installed, unless the person's own settings decide it already.
    pub fn set_enabled(&self, project: &Path, plugin: &str, enabled: bool) -> Result<()> {
        let _guard = one_at_a_time();
        let installed = self.installed_here(project, plugin)?;
        let base = match installed.installed_in {
            Scope::User => Scope::Local,
            scope => scope,
        };
        let scope = installed
            .decided_by
            .map_or(base, |decided_by| decided_by.max(base));
        let plugin = plugin.to_owned();
        self.run(
            project,
            &if enabled {
                PluginChange::Enable { plugin, scope }
            } else {
                PluginChange::Disable { plugin, scope }
            },
        )?;
        let now = self.installed_here(project, &installed.id)?;
        if now.enabled != enabled {
            return Err(Error::Harness(format!(
                "{} is still {}: other settings decide it",
                installed.id,
                if now.enabled { "on" } else { "off" }
            )));
        }
        Ok(())
    }

    /// Uninstalls `plugin` from `project`. A plugin installed for the user is refused: it is not
    /// the project's to remove, but it can be switched off there.
    pub fn uninstall_from_project(&self, project: &Path, plugin: &str) -> Result<()> {
        let _guard = one_at_a_time();
        let installed = self.installed_here(project, plugin)?;
        if installed.installed_in == Scope::User {
            return Err(Error::Harness(format!(
                "{plugin} is installed for your user; switch it off in this project instead"
            )));
        }
        self.run(
            project,
            &PluginChange::Uninstall {
                plugin: installed.id,
                scope: installed.installed_in,
            },
        )
    }

    fn installed_here(&self, project: &Path, plugin: &str) -> Result<ScopedPlugin> {
        self.scope_state(Some(project))?
            .plugins
            .into_iter()
            .find(|installed| installed.id == plugin)
            .ok_or_else(|| Error::Harness(format!("{plugin} is not installed for this project")))
    }

    /// Where Claude Code has the marketplace `name` from, when it knows one by that name.
    fn known_source(&self, name: &str) -> Result<Option<KnownSource>> {
        let Some(config) = self.existing_config_dir() else {
            return Ok(None);
        };
        let known = read_json(&config.join(KNOWN_MARKETPLACES))?;
        Ok(known
            .as_ref()
            .and_then(|known| known.get(name))
            .map(|entry| {
                let source = entry.get("source").cloned().unwrap_or(Value::Null);
                match marketplace_source(&source) {
                    Ok(source) => KnownSource::Usable(source),
                    Err(_) => KnownSource::Other(source.to_string()),
                }
            }))
    }
}

/// A marketplace source as Claude Code records it.
enum KnownSource {
    Usable(MarketplaceSource),
    /// One that the catalog cannot use, such as an npm package.
    Other(String),
}

impl fmt::Display for KnownSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usable(source) => write!(f, "{source}"),
            Self::Other(source) => write!(f, "{source}"),
        }
    }
}

fn same_source(known: &KnownSource, source: &MarketplaceSource) -> bool {
    match (known, source) {
        (
            KnownSource::Usable(MarketplaceSource::Local { path: a }),
            MarketplaceSource::Local { path: b },
        ) => same_dir(a, b),
        (KnownSource::Usable(known), source) => known == source,
        (KnownSource::Other(_), _) => false,
    }
}

/// Reads the result line that `--json` prints. A change that was made already counts as made.
fn outcome(output: &Output) -> Result<()> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let result = stdout
        .lines()
        .rev()
        .find_map(|line| serde_json::from_str::<Value>(line).ok());
    match result {
        Some(result)
            if result["outcome"] == "ok" || result["alreadyInGoalState"] == Value::Bool(true) =>
        {
            Ok(())
        }
        Some(result) => Err(Error::Harness(
            result["message"]
                .as_str()
                .unwrap_or("the change failed")
                .to_owned(),
        )),
        None if output.status.success() => Ok(()),
        None => Err(Error::Harness(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        )),
    }
}

/// Finds the `claude` command in `path` (the value of `PATH`) and the usual install locations,
/// which a desktop app on macOS needs because it does not get the shell's `PATH`.
///
/// Only absolute directories count: a relative one would resolve against the project folder,
/// which can come from anyone.
fn find_claude(path: Option<OsString>, home: Option<PathBuf>) -> Option<PathBuf> {
    let names: &[&str] = if cfg!(windows) {
        &["claude.exe", "claude.cmd"]
    } else {
        &["claude"]
    };
    let mut dirs: Vec<PathBuf> = path
        .map(|path| env::split_paths(&path).collect())
        .unwrap_or_default();
    if let Some(home) = home {
        dirs.extend([
            home.join(".local/bin"),
            home.join(".claude/local"),
            home.join(".npm-global/bin"),
            home.join(".bun/bin"),
        ]);
    }
    dirs.extend(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from));
    dirs.iter()
        .filter(|dir| dir.is_absolute())
        .flat_map(|dir| names.iter().map(move |name| dir.join(name)))
        .find(|candidate| candidate.is_file())
}

#[cfg(all(test, unix))]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    /// A fake `claude` that logs where and how it ran, and prints `result`.
    fn fake_claude(dir: &Path, result: &str) -> (PathBuf, PathBuf) {
        let log = dir.join("calls.log");
        let script = dir.join("claude");
        fs::write(
            &script,
            format!(
                "#!/bin/sh\necho \"$PWD|$CLAUDE_CONFIG_DIR|$*\" >> '{}'\necho '{result}'\n",
                log.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        (script, log)
    }

    fn calls(log: &Path) -> Vec<String> {
        fs::read_to_string(log)
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    struct Setup {
        _tmp: tempfile::TempDir,
        config: PathBuf,
        project: PathBuf,
    }

    fn setup(known: &str, installed: &str) -> Setup {
        let tmp = tempfile::tempdir().unwrap();
        let config = tmp.path().join("claude");
        let project = tmp.path().join("project");
        fs::create_dir_all(config.join("plugins")).unwrap();
        fs::create_dir_all(&project).unwrap();
        fs::write(config.join(KNOWN_MARKETPLACES), known).unwrap();
        fs::write(config.join("plugins/installed_plugins.json"), installed).unwrap();
        Setup {
            _tmp: tmp,
            config,
            project,
        }
    }

    const OK: &str = r#"{"outcome":"ok"}"#;

    #[test]
    fn runs_claude_in_the_project_with_its_config_dir() {
        let setup = setup("{}", r#"{"plugins":{}}"#);
        let (cli, log) = fake_claude(&setup.config, OK);
        let claude = ClaudeCode::new(&setup.config).with_cli(&cli);

        claude
            .change(
                &setup.project,
                &PluginChange::Disable {
                    plugin: "review@acme".into(),
                    scope: Scope::Local,
                },
            )
            .unwrap();

        let project = fs::canonicalize(&setup.project).unwrap();
        assert_eq!(
            calls(&log),
            [format!(
                "{}|{}|plugin disable --scope local --json -- review@acme",
                project.display(),
                setup.config.display()
            )]
        );
    }

    #[test]
    fn reports_what_claude_said_and_accepts_a_change_made_already() {
        let setup = setup("{}", r#"{"plugins":{}}"#);
        let change = PluginChange::Enable {
            plugin: "review@acme".into(),
            scope: Scope::Local,
        };

        let (cli, _) = fake_claude(
            &setup.config,
            r#"{"outcome":"failed","message":"Plugin not found"}"#,
        );
        let error = ClaudeCode::new(&setup.config)
            .with_cli(&cli)
            .change(&setup.project, &change)
            .unwrap_err();
        assert_eq!(error.to_string(), "Claude Code: Plugin not found");

        let (cli, _) = fake_claude(
            &setup.config,
            r#"{"outcome":"failed","alreadyInGoalState":true}"#,
        );
        ClaudeCode::new(&setup.config)
            .with_cli(&cli)
            .change(&setup.project, &change)
            .unwrap();
    }

    fn call_args(log: &Path) -> Vec<String> {
        calls(log)
            .iter()
            .map(|call| call.rsplit('|').next().unwrap().to_owned())
            .collect()
    }

    #[test]
    fn installs_a_github_marketplace_plugin_for_everyone_and_declares_the_marketplace() {
        let setup = setup(
            r#"{"known": {"source": {"source": "github", "repo": "Acme/Tools"}}}"#,
            r#"{"plugins":{}}"#,
        );
        let (cli, log) = fake_claude(&setup.config, OK);
        let claude = ClaudeCode::new(&setup.config).with_cli(&cli);
        let github = MarketplaceSource::GitHub {
            owner: "acme".into(),
            repo: "tools".into(),
        };

        let new = claude
            .install_in_project(&setup.project, "review", "acme", &github)
            .unwrap();
        // Claude Code knows this one already; the project still declares it for the others.
        let known = claude
            .install_in_project(&setup.project, "lint", "known", &github)
            .unwrap();

        assert_eq!((new, known), (Scope::Project, Scope::Project));
        assert_eq!(
            call_args(&log),
            [
                "plugin marketplace add --scope project --json -- acme/tools",
                "plugin install --scope project --json -- review@acme",
                "plugin marketplace add --scope project --json -- acme/tools",
                "plugin install --scope project --json -- lint@known",
            ]
        );
    }

    #[test]
    fn installs_a_local_folder_marketplace_plugin_only_for_the_person() {
        let setup = setup("{}", r#"{"plugins":{}}"#);
        let (cli, log) = fake_claude(&setup.config, OK);
        let folder = setup.config.join("my-market");

        let scope = ClaudeCode::new(&setup.config)
            .with_cli(&cli)
            .install_in_project(
                &setup.project,
                "review",
                "mine",
                &MarketplaceSource::Local {
                    path: folder.clone(),
                },
            )
            .unwrap();

        assert_eq!(scope, Scope::Local);
        assert_eq!(
            call_args(&log),
            [
                format!(
                    "plugin marketplace add --scope local --json -- {}",
                    folder.display()
                ),
                "plugin install --scope local --json -- review@mine".to_owned(),
            ]
        );
    }

    #[test]
    fn refuses_a_marketplace_that_claude_code_knows_from_elsewhere() {
        let setup = setup(
            r#"{"tools": {"source": {"source": "github", "repo": "fork-b/tools"}}}"#,
            r#"{"plugins":{}}"#,
        );
        let (cli, log) = fake_claude(&setup.config, OK);

        let error = ClaudeCode::new(&setup.config)
            .with_cli(&cli)
            .install_in_project(
                &setup.project,
                "x",
                "tools",
                &MarketplaceSource::GitHub {
                    owner: "fork-a".into(),
                    repo: "tools".into(),
                },
            )
            .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("knows a different marketplace named tools (github.com/fork-b/tools)"),
            "{error}"
        );
        assert_eq!(calls(&log), Vec::<String>::new());
    }

    #[test]
    fn leaves_the_default_config_dir_to_the_environment() {
        let setup = setup("{}", r#"{"plugins":{}}"#);
        let (cli, log) = fake_claude(&setup.config, OK);

        ClaudeCode::from_env()
            .with_cli(&cli)
            .change(
                &setup.project,
                &PluginChange::Enable {
                    plugin: "review@acme".into(),
                    scope: Scope::Local,
                },
            )
            .unwrap();

        // Whatever the test runs with, not a directory that harness-hanger chose.
        let inherited = env::var("CLAUDE_CONFIG_DIR").unwrap_or_default();
        assert_eq!(calls(&log)[0].split('|').nth(1).unwrap(), inherited);
    }

    #[test]
    fn finds_claude_only_in_absolute_directories() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        fs::write(bin.join("claude"), "").unwrap();
        let path =
            env::join_paths([PathBuf::from("relative/bin"), PathBuf::new(), bin.clone()]).unwrap();

        assert_eq!(find_claude(Some(path), None), Some(bin.join("claude")));
        let only_relative = env::join_paths([PathBuf::from("."), PathBuf::from("bin")]).unwrap();
        assert_eq!(
            find_claude(Some(only_relative), None)
                .filter(|found| !found.starts_with("/opt") && !found.starts_with("/usr")),
            None
        );
    }

    #[test]
    fn refuses_to_install_a_user_plugin_in_a_project() {
        let setup = setup(
            r#"{"acme": {}}"#,
            r#"{"plugins":{"review@acme":[{"scope":"user"}]}}"#,
        );
        let (cli, log) = fake_claude(&setup.config, OK);

        let error = ClaudeCode::new(&setup.config)
            .with_cli(&cli)
            .install_in_project(
                &setup.project,
                "review",
                "acme",
                &MarketplaceSource::Local { path: "/m".into() },
            )
            .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("switch it off in this project instead")
        );
        assert_eq!(calls(&log), Vec::<String>::new());
    }

    /// A fake `claude` that applies `enable`/`disable` to the project's settings file for the
    /// scope, as Claude Code does.
    fn settings_writing_claude(dir: &Path, log: &Path) -> PathBuf {
        let script = dir.join("claude-writes");
        fs::write(
            &script,
            format!(
                r#"#!/bin/sh
echo "$*" >> '{log}'
case "$4" in local) file=.claude/settings.local.json ;; *) file=.claude/settings.json ;; esac
case "$2" in enable) value=true ;; *) value=false ;; esac
mkdir -p .claude
echo "{{ \"enabledPlugins\": {{ \"$7\": $value }} }}" > "$file"
echo '{{"outcome":"ok"}}'
"#,
                log = log.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        script
    }

    #[test]
    fn switches_a_user_plugin_in_the_persons_own_settings_for_the_project() {
        let setup = setup("{}", r#"{"plugins":{"review@acme":[{"scope":"user"}]}}"#);
        fs::write(
            setup.config.join("settings.json"),
            r#"{"enabledPlugins":{"review@acme":true}}"#,
        )
        .unwrap();
        let log = setup.config.join("calls.log");
        let cli = settings_writing_claude(&setup.config, &log);
        let claude = ClaudeCode::new(&setup.config).with_cli(&cli);

        claude
            .set_enabled(&setup.project, "review@acme", false)
            .unwrap();

        assert_eq!(
            calls(&log),
            ["plugin disable --scope local --json -- review@acme"]
        );
        let error = claude
            .uninstall_from_project(&setup.project, "review@acme")
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("switch it off in this project instead")
        );
    }

    #[test]
    fn switches_a_project_plugin_where_it_is_installed() {
        let setup = setup("{}", "{}");
        let installed = serde_json::json!({ "plugins": {
            "lint@acme": [{ "scope": "project", "projectPath": setup.project }]
        } });
        fs::write(
            setup.config.join("plugins/installed_plugins.json"),
            installed.to_string(),
        )
        .unwrap();
        let log = setup.config.join("calls.log");
        let cli = settings_writing_claude(&setup.config, &log);
        let claude = ClaudeCode::new(&setup.config).with_cli(&cli);

        claude
            .set_enabled(&setup.project, "lint@acme", true)
            .unwrap();

        assert_eq!(
            calls(&log),
            ["plugin enable --scope project --json -- lint@acme"]
        );
        assert!(claude.scope_state(Some(&setup.project)).unwrap().plugins[0].enabled);
    }
}
