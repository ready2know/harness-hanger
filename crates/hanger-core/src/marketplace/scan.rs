//! Reads a marketplace directory and lists the plugins and components it contains.
//!
//! The rules follow the Claude Code plugin manifest and marketplace references. The scan is
//! read-only, and a bad file never stops it: it becomes a [`ScanWarning`].

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Component as PathPart, Path, PathBuf};

use serde::Serialize;

use super::manifest::{
    CommandSpec, ComponentPaths, MarketplaceManifest, PathList, PluginEntry, PluginManifest,
    PluginSourceSpec, RemoteSource,
};
use crate::frontmatter;
use crate::{Error, Result};

const MANIFEST: &str = ".claude-plugin/marketplace.json";
const PLUGIN_MANIFEST: &str = ".claude-plugin/plugin.json";
const SKILL_FILE: &str = "SKILL.md";
/// Stops runaway recursion, for example through a symlink loop.
const MAX_DEPTH: usize = 8;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplacePreview {
    pub name: String,
    pub description: Option<String>,
    pub owner: Option<String>,
    pub plugins: Vec<PluginPreview>,
    /// Problems that did not stop the scan, such as a skill with malformed frontmatter.
    pub warnings: Vec<ScanWarning>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginPreview {
    pub name: String,
    pub description: Option<String>,
    pub version: Option<String>,
    pub author: Option<String>,
    pub category: Option<String>,
    pub source: PluginSource,
    pub skills: Vec<Component>,
    pub agents: Vec<Component>,
    pub commands: Vec<Component>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PluginSource {
    /// A directory inside the marketplace. Its components are listed.
    Local { path: PathBuf },
    /// Fetched from elsewhere when it is installed, so its components are not known yet.
    External {
        /// The manifest's source type, such as `github`, `url`, or `npm`.
        source: String,
        /// The repository, URL, or package, when the manifest gives one.
        location: Option<String>,
    },
    /// Not a plugin: the skills, agents, and commands that a harness loads for the user on their
    /// own. `harness` is the adapter's ID.
    Harness { harness: String },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Component {
    /// The name Claude Code uses, with subdirectories as namespaces, such as `db:migrate`.
    pub name: String,
    pub description: Option<String>,
    /// The file that defines it. `None` for a command written inline in a manifest.
    pub path: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanWarning {
    pub path: PathBuf,
    pub message: String,
}

/// Reads `.claude-plugin/marketplace.json` under `root`.
pub(crate) fn read_manifest(root: &Path) -> Result<MarketplaceManifest> {
    let not_a_marketplace = |reason: String| Error::NotAMarketplace {
        path: root.to_owned(),
        reason,
    };
    if !root.is_dir() {
        return Err(not_a_marketplace("the directory does not exist".into()));
    }
    let text = fs::read_to_string(root.join(MANIFEST)).map_err(|error| match error.kind() {
        io::ErrorKind::NotFound => not_a_marketplace(format!("{MANIFEST} is missing")),
        _ => not_a_marketplace(format!("cannot read {MANIFEST}: {error}")),
    })?;
    serde_json::from_str(&text).map_err(|error| not_a_marketplace(format!("{MANIFEST}: {error}")))
}

/// The skills, agents, and commands that a harness loads for the user from `dir`, outside plugins,
/// as a preview with one entry.
///
/// Symlinks are followed wherever they lead: the directory is the user's own, and people link
/// skills into it from elsewhere. Symlink loops still end.
pub(crate) fn standalone(dir: &Path, harness: &str, name: &str) -> Result<MarketplacePreview> {
    let mut scan = Scan {
        root: dir,
        real_root: fs::canonicalize(dir).map_err(|source| Error::io(dir, source))?,
        plugin_root: None,
        follow_links_out: true,
        warnings: Vec::new(),
    };
    let components = PluginPreview {
        name: harness.to_owned(),
        description: None,
        version: None,
        author: None,
        category: None,
        source: PluginSource::Harness {
            harness: harness.to_owned(),
        },
        skills: scan.skills(dir, None, false),
        agents: scan.agents(dir, None),
        commands: scan.commands(dir, None),
    };
    Ok(MarketplacePreview {
        name: name.to_owned(),
        description: Some(format!(
            "The skills, agents, and commands that {name} loads for your user, outside plugins."
        )),
        owner: None,
        plugins: vec![components],
        warnings: scan.warnings,
    })
}

/// Lists everything the marketplace at `root` contains.
pub fn preview(root: &Path) -> Result<MarketplacePreview> {
    let manifest = read_manifest(root)?;
    let mut scan = Scan {
        root,
        real_root: fs::canonicalize(root).map_err(|source| Error::io(root, source))?,
        plugin_root: manifest.metadata.plugin_root.as_deref(),
        follow_links_out: false,
        warnings: Vec::new(),
    };
    let plugins = manifest
        .plugin_entries()
        .filter_map(|(value, entry)| match entry {
            Ok(entry) => Some(scan.plugin(entry)),
            Err(error) => {
                let name = value.get("name").and_then(|name| name.as_str());
                scan.warn(
                    &root.join(MANIFEST),
                    format!(
                        "skipped plugin {}: {error}",
                        name.unwrap_or("without a name")
                    ),
                );
                None
            }
        })
        .collect();
    Ok(MarketplacePreview {
        name: manifest.name.clone(),
        description: manifest.description().map(str::to_owned),
        owner: manifest.owner.as_ref().map(|owner| owner.name().to_owned()),
        plugins,
        warnings: scan.warnings,
    })
}

struct Scan<'a> {
    root: &'a Path,
    /// `root` with symlinks resolved. Nothing outside it is read, unless `follow_links_out`.
    real_root: PathBuf,
    plugin_root: Option<&'a str>,
    /// Whether symlinks may lead out of `root`. Only for the user's own directories, never for a
    /// marketplace, which can come from anyone.
    follow_links_out: bool,
    warnings: Vec<ScanWarning>,
}

/// What a plugin directory adds to its marketplace entry.
#[derive(Default)]
struct Contents {
    description: Option<String>,
    version: Option<String>,
    author: Option<String>,
    skills: Vec<Component>,
    agents: Vec<Component>,
    commands: Vec<Component>,
}

impl Scan<'_> {
    fn warn(&mut self, path: &Path, message: impl Into<String>) {
        self.warnings.push(ScanWarning {
            path: path.to_owned(),
            message: message.into(),
        });
    }

    fn plugin(&mut self, entry: PluginEntry) -> PluginPreview {
        let PluginEntry {
            name,
            description,
            version,
            author,
            category,
            source,
            strict,
            components,
        } = entry;
        let (source, contents) = match source {
            PluginSourceSpec::Remote(remote) => (external(remote), Contents::default()),
            PluginSourceSpec::Path(path) => match self.plugin_dir(&path) {
                Some(dir) if dir.is_dir() => {
                    let contents = self.contents(&dir, strict.unwrap_or(true), components);
                    (PluginSource::Local { path: dir }, contents)
                }
                Some(dir) => {
                    self.warn(&dir, format!("plugin {name}: the directory does not exist"));
                    (PluginSource::Local { path: dir }, Contents::default())
                }
                None => {
                    self.warn(
                        &self.root.join(MANIFEST),
                        format!("plugin {name}: source {path} is not a path inside the marketplace that starts with ./"),
                    );
                    // Shown, never read.
                    let path = self.root.join(path);
                    (PluginSource::Local { path }, Contents::default())
                }
            },
        };
        PluginPreview {
            name,
            // The entry wins for display fields, and plugin.json wins for the version.
            description: description.or(contents.description),
            version: contents.version.or(version),
            author: author
                .map(|author| author.name().to_owned())
                .or(contents.author),
            category,
            source,
            skills: contents.skills,
            agents: contents.agents,
            commands: contents.commands,
        }
    }

    fn plugin_dir(&self, source: &str) -> Option<PathBuf> {
        if source == "." || source.starts_with("./") {
            return join_inside(self.root, source);
        }
        // `metadata.pluginRoot` applies only to bare names such as `formatter`.
        let plugin_root = self.plugin_root?;
        if source.contains(['/', '\\']) {
            return None;
        }
        join_inside(&join_inside(self.root, plugin_root)?, source)
    }

    fn contents(&mut self, dir: &Path, strict: bool, entry: ComponentPaths) -> Contents {
        // An entry whose source is the marketplace root and that lists skills loads only those.
        let skills_replace_default = dir == self.root && entry.skills.is_some();
        let Some(manifest) = self.plugin_manifest(dir) else {
            // Without plugin.json, the entry is the manifest.
            return Contents {
                skills: self.skills(dir, entry.skills, skills_replace_default),
                agents: self.agents(dir, entry.agents),
                commands: self.commands(dir, entry.commands),
                ..Contents::default()
            };
        };
        let (declared, skills_replace_default) = if strict {
            (manifest.components.append(entry), skills_replace_default)
        } else if manifest.components.is_empty() {
            (entry, skills_replace_default)
        } else {
            if !entry.is_empty() {
                self.warn(
                    &dir.join(PLUGIN_MANIFEST),
                    "both plugin.json and the non-strict marketplace entry declare components, so Claude Code will not load this plugin",
                );
            }
            (manifest.components, false)
        };
        Contents {
            description: manifest.description,
            version: manifest.version,
            author: manifest.author.map(|author| author.name().to_owned()),
            skills: self.skills(dir, declared.skills, skills_replace_default),
            agents: self.agents(dir, declared.agents),
            commands: self.commands(dir, declared.commands),
        }
    }

    fn plugin_manifest(&mut self, dir: &Path) -> Option<PluginManifest> {
        let path = dir.join(PLUGIN_MANIFEST);
        if !self.is_inside(&path) {
            return None;
        }
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return None,
            Err(error) => {
                self.warn(&path, format!("cannot read: {error}"));
                return None;
            }
        };
        serde_json::from_str(&text)
            .inspect_err(|error| self.warn(&path, format!("ignored, it is not valid: {error}")))
            .ok()
    }

    /// Resolves manifest paths, which must start with `./` and stay inside the plugin.
    fn declared_paths(&mut self, dir: &Path, paths: PathList) -> Vec<PathBuf> {
        paths
            .into_vec()
            .into_iter()
            .filter_map(|path| {
                let relative = path == "." || path.starts_with("./");
                let resolved = relative.then(|| join_inside(dir, &path)).flatten();
                if resolved.is_none() {
                    self.warn(
                        dir,
                        format!("ignored {path}: a component path must start with ./ and stay inside the plugin"),
                    );
                }
                resolved
            })
            .collect()
    }

    fn skills(
        &mut self,
        dir: &Path,
        declared: Option<PathList>,
        replace_default: bool,
    ) -> Vec<Component> {
        let mut roots = Vec::new();
        if !replace_default {
            let default = dir.join("skills");
            if default.is_dir() {
                roots.push(default);
            } else if declared.is_none() && dir.join(SKILL_FILE).is_file() {
                // A plugin that is one skill keeps SKILL.md at its root.
                roots.push(dir.to_owned());
            }
        }
        if let Some(declared) = declared {
            roots.extend(self.declared_paths(dir, declared));
        }

        let mut skill_dirs = Vec::new();
        for root in roots {
            if root.join(SKILL_FILE).is_file() {
                skill_dirs.push(root);
            } else if root.is_dir() {
                skill_dirs.extend(
                    self.read_dir(&root)
                        .into_iter()
                        .filter(|entry| entry.join(SKILL_FILE).is_file()),
                );
            } else {
                self.warn(&root, "the skills directory does not exist");
            }
        }
        skill_dirs.sort();
        skill_dirs.dedup();
        skill_dirs.retain(|skill_dir| self.is_inside(skill_dir));

        let mut skills: Vec<_> = skill_dirs
            .iter()
            .map(|skill_dir| {
                let (name, description) = self.read_component(&skill_dir.join(SKILL_FILE), true);
                Component {
                    name: name.unwrap_or_else(|| file_name(skill_dir)),
                    description,
                    path: Some(skill_dir.join(SKILL_FILE)),
                }
            })
            .collect();
        sort(&mut skills);
        skills
    }

    fn agents(&mut self, dir: &Path, declared: Option<PathList>) -> Vec<Component> {
        // Declared agents replace the agents/ directory, and must be .md files.
        let files: Vec<(Vec<String>, PathBuf)> = match declared {
            None => self.markdown_files(&dir.join("agents")),
            Some(declared) => self
                .declared_paths(dir, declared)
                .into_iter()
                .filter(|path| {
                    let is_agent = path.is_file() && is_markdown(path);
                    if !is_agent {
                        self.warn(path, "an agent path must name an existing .md file");
                    }
                    is_agent && self.is_inside(path)
                })
                .map(|path| (Vec::new(), path))
                .collect(),
        };
        let mut agents: Vec<_> = files
            .into_iter()
            .map(|(namespace, path)| {
                let (name, description) = self.read_component(&path, false);
                let name = name.unwrap_or_else(|| file_stem(&path));
                Component {
                    name: [namespace, vec![name]].concat().join(":"),
                    description,
                    path: Some(path),
                }
            })
            .collect();
        sort(&mut agents);
        agents
    }

    fn commands(&mut self, dir: &Path, declared: Option<CommandSpec>) -> Vec<Component> {
        // Declared commands replace the commands/ directory.
        let files = match declared {
            None => self.markdown_files(&dir.join("commands")),
            Some(CommandSpec::Paths(paths)) => {
                let mut files = Vec::new();
                for path in self.declared_paths(dir, paths) {
                    if path.is_dir() {
                        files.extend(self.markdown_files(&path));
                    } else if path.is_file() && is_markdown(&path) {
                        if self.is_inside(&path) {
                            files.push((Vec::new(), path));
                        }
                    } else {
                        self.warn(&path, "a command path must name a .md file or a directory");
                    }
                }
                files
            }
            Some(CommandSpec::Map(definitions)) => {
                let mut commands = Vec::new();
                for (name, definition) in definitions {
                    let path = definition
                        .source
                        .and_then(|source| self.declared_paths(dir, PathList::One(source)).pop());
                    let description = definition.description.or_else(|| {
                        path.as_ref()
                            .and_then(|path| self.read_component(path, true).1)
                    });
                    commands.push(Component {
                        name,
                        description,
                        path,
                    });
                }
                return commands;
            }
        };
        let mut commands: Vec<_> = files
            .into_iter()
            .map(|(namespace, path)| {
                let (_, description) = self.read_component(&path, true);
                Component {
                    name: [namespace, vec![file_stem(&path)]].concat().join(":"),
                    description,
                    path: Some(path),
                }
            })
            .collect();
        sort(&mut commands);
        commands
    }

    /// Reads the frontmatter name and the description of a component file.
    ///
    /// `body_fallback` takes the description from the first body line when the frontmatter has
    /// none, as Claude Code does for skills and commands.
    fn read_component(
        &mut self,
        path: &Path,
        body_fallback: bool,
    ) -> (Option<String>, Option<String>) {
        if !self.is_inside(path) {
            return (None, None);
        }
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => {
                self.warn(path, format!("cannot read: {error}"));
                return (None, None);
            }
        };
        let document = frontmatter::parse(&text);
        let description = if body_fallback {
            document.description()
        } else {
            document.frontmatter.description.clone()
        };
        (document.frontmatter.name, description)
    }

    /// Lists the `.md` files under `dir`, each with the subdirectories that lead to it.
    fn markdown_files(&mut self, dir: &Path) -> Vec<(Vec<String>, PathBuf)> {
        let mut files = Vec::new();
        if let Ok(real) = fs::canonicalize(dir)
            && real.is_dir()
        {
            // Directories already walked, so that symlink loops end. Plugins can share a
            // directory, so each walk starts afresh.
            let mut visited = HashSet::from([real]);
            self.collect_markdown(dir, &mut Vec::new(), &mut visited, &mut files);
        }
        files
    }

    fn collect_markdown(
        &mut self,
        dir: &Path,
        namespace: &mut Vec<String>,
        visited: &mut HashSet<PathBuf>,
        files: &mut Vec<(Vec<String>, PathBuf)>,
    ) {
        for path in self.read_dir(dir) {
            if path.is_dir() {
                let first_visit = fs::canonicalize(&path).is_ok_and(|real| visited.insert(real));
                if first_visit && namespace.len() < MAX_DEPTH {
                    namespace.push(file_name(&path));
                    self.collect_markdown(&path, namespace, visited, files);
                    namespace.pop();
                }
            } else if is_markdown(&path) && self.is_inside(&path) {
                files.push((namespace.clone(), path));
            }
        }
    }

    /// Whether `path` resolves inside the marketplace. A symlink in a cloned repository can point
    /// anywhere, such as the home directory.
    fn is_inside(&mut self, path: &Path) -> bool {
        if self.follow_links_out {
            return true;
        }
        match fs::canonicalize(path) {
            Ok(real) if !real.starts_with(&self.real_root) => {
                self.warn(path, "skipped, because it links outside the marketplace");
                false
            }
            // A path that does not resolve fails, and is reported, when it is read.
            _ => true,
        }
    }

    /// Lists the entries of `dir`, sorted, without hidden ones such as `.git`.
    fn read_dir(&mut self, dir: &Path) -> Vec<PathBuf> {
        if !self.is_inside(dir) {
            return Vec::new();
        }
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(error) => {
                self.warn(dir, format!("cannot read: {error}"));
                return Vec::new();
            }
        };
        let mut paths: Vec<_> = entries
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| !file_name(path).starts_with('.'))
            .collect();
        paths.sort();
        paths.retain(|path| {
            let broken = fs::symlink_metadata(path).is_ok_and(|meta| meta.is_symlink())
                && fs::metadata(path).is_err();
            if broken {
                let target = fs::read_link(path).unwrap_or_default();
                self.warn(path, format!("a broken link to {}", target.display()));
            }
            !broken
        });
        paths
    }
}

fn external(remote: RemoteSource) -> PluginSource {
    let location = match remote.source.as_str() {
        "github" => remote.repo,
        "git-subdir" => remote.url.map(|url| match remote.path {
            Some(path) => format!("{url} ({path})"),
            None => url,
        }),
        "npm" => remote.package,
        _ => remote.url,
    };
    PluginSource::External {
        source: remote.source,
        location,
    }
}

/// Joins a relative path onto `base`, refusing absolute paths and `..`.
fn join_inside(base: &Path, relative: &str) -> Option<PathBuf> {
    let mut joined = base.to_owned();
    for part in Path::new(relative).components() {
        match part {
            PathPart::CurDir => {}
            PathPart::Normal(name) => joined.push(name),
            PathPart::ParentDir | PathPart::RootDir | PathPart::Prefix(_) => return None,
        }
    }
    Some(joined)
}

fn is_markdown(path: &Path) -> bool {
    path.extension().is_some_and(|extension| extension == "md")
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn sort(components: &mut [Component]) {
    components.sort_by(|a, b| a.name.cmp(&b.name));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/marketplace")
    }

    fn plugin<'a>(preview: &'a MarketplacePreview, name: &str) -> &'a PluginPreview {
        preview
            .plugins
            .iter()
            .find(|plugin| plugin.name == name)
            .unwrap_or_else(|| panic!("no plugin {name}"))
    }

    fn names(components: &[Component]) -> Vec<&str> {
        components.iter().map(|c| c.name.as_str()).collect()
    }

    fn descriptions(components: &[Component]) -> Vec<Option<&str>> {
        components
            .iter()
            .map(|c| c.description.as_deref())
            .collect()
    }

    #[test]
    fn reads_the_marketplace_and_skips_entries_it_cannot_parse() {
        let preview = preview(&fixture()).unwrap();

        assert_eq!(preview.name, "fixture-market");
        assert_eq!(
            preview.description.as_deref(),
            Some("A marketplace for tests.")
        );
        assert_eq!(preview.owner.as_deref(), Some("Fixture Owner"));
        let plugins: Vec<_> = preview.plugins.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            plugins,
            [
                "standard",
                "bare",
                "custom",
                "root-one",
                "remote",
                "subdir",
                "lenient",
                "conflict",
                "nonstrict",
                "escape",
                "missing",
                "inline"
            ]
        );
    }

    #[test]
    fn lists_the_components_of_the_default_layout() {
        let preview = preview(&fixture()).unwrap();
        let standard = plugin(&preview, "standard");

        assert_eq!(
            standard.description.as_deref(),
            Some("Uses the default layout.")
        );
        assert_eq!(standard.version.as_deref(), Some("1.2.0"));
        assert_eq!(standard.author.as_deref(), Some("Standard Author"));
        assert_eq!(standard.category.as_deref(), Some("development"));
        assert_eq!(
            standard.source,
            PluginSource::Local {
                path: fixture().join("plugins/standard")
            }
        );
        assert_eq!(names(&standard.skills), ["alpha-skill", "beta"]);
        assert_eq!(
            descriptions(&standard.skills),
            [
                Some("Does alpha things across two lines."),
                Some("Beta helps with beta work")
            ]
        );
        assert_eq!(
            names(&standard.agents),
            ["code-reviewer", "review:security"]
        );
        assert_eq!(names(&standard.commands), ["db:migrate", "deploy"]);
        assert_eq!(
            descriptions(&standard.commands),
            [Some("Runs migrations."), Some("Deploys the app.")]
        );
    }

    #[test]
    fn resolves_bare_names_under_the_plugin_root() {
        let preview = preview(&fixture()).unwrap();
        let bare = plugin(&preview, "bare");

        assert_eq!(
            bare.source,
            PluginSource::Local {
                path: fixture().join("plugins/bare")
            }
        );
        // A SKILL.md at the plugin root makes the plugin one skill, named after its directory.
        assert_eq!(names(&bare.skills), ["bare"]);
    }

    #[test]
    fn declared_skills_add_to_the_default_but_agents_and_commands_replace_it() {
        let preview = preview(&fixture()).unwrap();
        let custom = plugin(&preview, "custom");

        assert_eq!(custom.author.as_deref(), Some("Custom Author"));
        assert_eq!(names(&custom.skills), ["delta", "gamma"]);
        assert_eq!(names(&custom.agents), ["helper"]);
        assert_eq!(names(&custom.commands), ["build"]);
    }

    #[test]
    fn an_entry_at_the_marketplace_root_loads_only_the_skills_it_lists() {
        let preview = preview(&fixture()).unwrap();

        assert_eq!(names(&plugin(&preview, "root-one").skills), ["one"]);
    }

    #[test]
    fn lists_external_plugins_without_components() {
        let preview = preview(&fixture()).unwrap();

        let remote = plugin(&preview, "remote");
        assert_eq!(
            remote.source,
            PluginSource::External {
                source: "github".into(),
                location: Some("acme/remote".into())
            }
        );
        assert!(remote.skills.is_empty() && remote.agents.is_empty());
        assert_eq!(
            plugin(&preview, "subdir").source,
            PluginSource::External {
                source: "git-subdir".into(),
                location: Some("https://example.com/mono.git (tools/plugin)".into())
            }
        );
    }

    #[test]
    fn reads_inline_command_definitions() {
        let preview = preview(&fixture()).unwrap();
        let inline = plugin(&preview, "inline");

        assert_eq!(names(&inline.commands), ["hello"]);
        assert_eq!(descriptions(&inline.commands), [Some("Greets you.")]);
        assert_eq!(inline.commands[0].path, None);
    }

    #[test]
    fn reads_frontmatter_that_is_not_valid_yaml_as_claude_code_does() {
        let preview = preview(&fixture()).unwrap();
        let lenient = plugin(&preview, "lenient");

        assert_eq!(names(&lenient.skills), ["odd"]);
        assert_eq!(
            descriptions(&lenient.skills),
            [Some("Use it when: the YAML is not valid.")]
        );
    }

    #[test]
    fn turns_bad_entries_into_warnings() {
        let preview = preview(&fixture()).unwrap();

        // The conflicting entry's agents are ignored in favour of plugin.json.
        assert_eq!(names(&plugin(&preview, "conflict").agents), ["a"]);

        let messages: Vec<_> = preview
            .warnings
            .iter()
            .map(|w| w.message.as_str())
            .collect();
        assert_eq!(messages.len(), 4, "{messages:#?}");
        for expected in [
            "both plugin.json and the non-strict marketplace entry declare components",
            "plugin escape: source ../outside is not a path inside the marketplace",
            "plugin missing: the directory does not exist",
            "skipped plugin without a name",
        ] {
            assert!(
                messages.iter().any(|message| message.contains(expected)),
                "no warning containing {expected:?} in {messages:#?}"
            );
        }
    }

    #[test]
    fn a_non_strict_entry_supplies_the_components_that_plugin_json_does_not_declare() {
        let preview = preview(&fixture()).unwrap();
        let nonstrict = plugin(&preview, "nonstrict");

        assert_eq!(nonstrict.version.as_deref(), Some("2.0.0"));
        assert_eq!(names(&nonstrict.commands), ["build"]);
        assert!(!preview.warnings.iter().any(|warning| {
            warning
                .path
                .starts_with(fixture().join("plugins/nonstrict"))
        }));
    }

    #[cfg(unix)]
    #[test]
    fn does_not_follow_symlinks_in_loops_or_out_of_the_marketplace() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::tempdir().unwrap();
        let outside = tmp.path().join("outside");
        fs::create_dir_all(outside.join("secret-skill")).unwrap();
        fs::write(outside.join("secret.md"), "Secret.").unwrap();
        fs::write(outside.join("secret-skill/SKILL.md"), "Secret.").unwrap();
        let root = tmp.path().join("market");
        let plugin_dir = root.join("plugin");
        fs::create_dir_all(root.join(".claude-plugin")).unwrap();
        fs::write(
            root.join(".claude-plugin/marketplace.json"),
            r#"{ "name": "links", "plugins": [{ "name": "p", "source": "./plugin" }] }"#,
        )
        .unwrap();
        fs::create_dir_all(plugin_dir.join("commands")).unwrap();
        fs::create_dir_all(plugin_dir.join("skills")).unwrap();
        fs::write(plugin_dir.join("commands/real.md"), "Real.").unwrap();
        for name in ["l1", "l2", "l3", "l4"] {
            symlink(".", plugin_dir.join("commands").join(name)).unwrap();
        }
        symlink(&outside, plugin_dir.join("commands/home")).unwrap();
        symlink(
            outside.join("secret-skill"),
            plugin_dir.join("skills/secret"),
        )
        .unwrap();

        let preview = preview(&root).unwrap();

        let linked = &preview.plugins[0];
        assert_eq!(names(&linked.commands), ["real"]);
        assert!(linked.skills.is_empty());
        assert!(
            preview
                .warnings
                .iter()
                .any(|warning| warning.message.contains("links outside the marketplace"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn reports_broken_links() {
        let tmp = tempfile::tempdir().unwrap();
        let skills = tmp.path().join("skills");
        fs::create_dir_all(skills.join("real")).unwrap();
        fs::write(skills.join("real/SKILL.md"), "Real.").unwrap();
        std::os::unix::fs::symlink(tmp.path().join("moved"), skills.join("gone")).unwrap();

        let preview = standalone(tmp.path(), "test", "Test").unwrap();

        assert_eq!(names(&preview.plugins[0].skills), ["real"]);
        assert_eq!(preview.warnings.len(), 1);
        assert!(preview.warnings[0].message.starts_with("a broken link to"));
    }

    #[test]
    fn plugins_that_share_a_directory_each_list_its_components() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".claude-plugin")).unwrap();
        fs::create_dir_all(root.join("agents")).unwrap();
        fs::write(root.join("agents/shared.md"), "---\nname: shared\n---\n").unwrap();
        fs::write(
            root.join(".claude-plugin/marketplace.json"),
            r#"{ "name": "shared", "plugins": [
                { "name": "one", "source": "./" },
                { "name": "two", "source": "./" }
            ] }"#,
        )
        .unwrap();

        let preview = preview(root).unwrap();

        assert_eq!(names(&preview.plugins[0].agents), ["shared"]);
        assert_eq!(names(&preview.plugins[1].agents), ["shared"]);
    }

    #[test]
    fn a_directory_without_a_manifest_is_not_a_marketplace() {
        let dir = tempfile::tempdir().unwrap();

        let error = preview(dir.path()).unwrap_err();

        assert!(
            matches!(&error, Error::NotAMarketplace { reason, .. } if reason.contains("is missing")),
            "{error}"
        );
    }
}
