//! The parts of `marketplace.json` and `plugin.json` that the preview reads.
//!
//! Claude Code ignores unknown keys, and so do these types.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

/// `.claude-plugin/marketplace.json` at the marketplace root.
#[derive(Debug, Deserialize)]
pub(crate) struct MarketplaceManifest {
    pub name: String,
    pub description: Option<String>,
    pub owner: Option<Person>,
    #[serde(default)]
    pub metadata: Metadata,
    /// Parsed one by one, so that one bad entry does not hide the others.
    #[serde(default)]
    pub plugins: Vec<serde_json::Value>,
}

impl MarketplaceManifest {
    pub fn description(&self) -> Option<&str> {
        self.description
            .as_deref()
            .or(self.metadata.description.as_deref())
    }

    /// Each entry of `plugins` with the result of parsing it.
    pub fn plugin_entries(
        &self,
    ) -> impl Iterator<Item = (&serde_json::Value, serde_json::Result<PluginEntry>)> {
        self.plugins
            .iter()
            .map(|value| (value, PluginEntry::deserialize(value)))
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Metadata {
    pub description: Option<String>,
    /// Directory that bare plugin source names, such as `"formatter"`, resolve under.
    pub plugin_root: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum Person {
    Named { name: String },
    Plain(String),
}

impl Person {
    pub fn name(&self) -> &str {
        match self {
            Self::Named { name } | Self::Plain(name) => name,
        }
    }
}

/// One entry of the marketplace's `plugins` array.
#[derive(Debug, Deserialize)]
pub(crate) struct PluginEntry {
    pub name: String,
    pub description: Option<String>,
    pub version: Option<String>,
    pub author: Option<Person>,
    pub category: Option<String>,
    pub source: PluginSourceSpec,
    /// Whether the entry may add components to a plugin that has its own `plugin.json`.
    pub strict: Option<bool>,
    #[serde(flatten)]
    pub components: ComponentPaths,
    #[serde(flatten)]
    pub links: Links,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum PluginSourceSpec {
    /// A path inside the marketplace, such as `./plugins/formatter`.
    Path(String),
    Remote(RemoteSource),
}

/// A plugin fetched from elsewhere when it is installed.
#[derive(Debug, Deserialize)]
pub(crate) struct RemoteSource {
    pub source: String,
    pub repo: Option<String>,
    pub url: Option<String>,
    pub path: Option<String>,
    pub package: Option<String>,
}

/// `.claude-plugin/plugin.json` in a plugin directory.
#[derive(Debug, Deserialize)]
pub(crate) struct PluginManifest {
    pub description: Option<String>,
    pub version: Option<String>,
    pub author: Option<Person>,
    #[serde(flatten)]
    pub components: ComponentPaths,
    #[serde(flatten)]
    pub links: Links,
}

/// Fields that a plugin's page shows, in `plugin.json` and in a marketplace entry.
///
/// They are read leniently: a value of an unexpected type is ignored, because Claude Code does not
/// need these fields to load the plugin, and a strict type would hide the whole plugin.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct Links {
    homepage: Option<Value>,
    repository: Option<Value>,
    license: Option<Value>,
    keywords: Option<Value>,
    /// Marketplace entries only.
    tags: Option<Value>,
}

impl Links {
    pub fn homepage(&self) -> Option<String> {
        url(self.homepage.as_ref())
    }

    /// A URL, or the `url` of an npm-style `{ "type": "git", "url": … }` object.
    pub fn repository(&self) -> Option<String> {
        url(self.repository.as_ref())
    }

    pub fn license(&self) -> Option<String> {
        text(self.license.as_ref())
    }

    /// `keywords`, then `tags`.
    pub fn keywords(&self) -> Vec<String> {
        [&self.keywords, &self.tags]
            .into_iter()
            .flat_map(|value| match value {
                Some(Value::Array(items)) => {
                    items.iter().filter_map(|item| text(Some(item))).collect()
                }
                value => Vec::from_iter(text(value.as_ref())),
            })
            .collect()
    }
}

fn text(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

fn url(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::Object(object)) => text(object.get("url")),
        value => text(value),
    }
}

/// Component locations that a manifest or a marketplace entry can declare.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct ComponentPaths {
    pub skills: Option<PathList>,
    pub agents: Option<PathList>,
    pub commands: Option<CommandSpec>,
}

impl ComponentPaths {
    pub fn is_empty(&self) -> bool {
        self.skills.is_none() && self.agents.is_none() && self.commands.is_none()
    }

    /// Adds `other`'s declarations after these, as Claude Code merges a strict entry into `plugin.json`.
    pub fn append(mut self, other: Self) -> Self {
        self.skills = PathList::concat(self.skills, other.skills);
        self.agents = PathList::concat(self.agents, other.agents);
        self.commands = match (self.commands, other.commands) {
            (Some(CommandSpec::Paths(a)), Some(CommandSpec::Paths(b))) => Some(CommandSpec::Paths(
                PathList::Many([a.into_vec(), b.into_vec()].concat()),
            )),
            (Some(CommandSpec::Map(mut a)), Some(CommandSpec::Map(b))) => {
                a.extend(b);
                Some(CommandSpec::Map(a))
            }
            (mine, theirs) => theirs.or(mine),
        };
        self
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum PathList {
    One(String),
    Many(Vec<String>),
}

impl PathList {
    pub fn into_vec(self) -> Vec<String> {
        match self {
            Self::One(path) => vec![path],
            Self::Many(paths) => paths,
        }
    }

    fn concat(a: Option<Self>, b: Option<Self>) -> Option<Self> {
        match (a, b) {
            (Some(a), Some(b)) => Some(Self::Many([a.into_vec(), b.into_vec()].concat())),
            (a, b) => a.or(b),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum CommandSpec {
    Paths(PathList),
    /// Command name to its definition.
    Map(BTreeMap<String, CommandDefinition>),
}

#[derive(Debug, Deserialize)]
pub(crate) struct CommandDefinition {
    /// Path to the command file. Absent when the definition has inline `content`.
    pub source: Option<String>,
    pub description: Option<String>,
}
