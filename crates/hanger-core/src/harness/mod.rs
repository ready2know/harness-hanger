//! AI coding harnesses, such as Claude Code, and what the catalog can import from them.
//!
//! Each harness has an adapter. To support another harness, implement [`HarnessAdapter`] for it
//! and add it to [`adapters`]. The catalog, the CLI, and the desktop app then offer its imports
//! with no other change.

pub mod claude_code;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::marketplace::{MarketplacePreview, MarketplaceSource, standalone};

pub use claude_code::ClaudeCode;

/// Every harness that the catalog can import from.
#[must_use]
pub fn adapters() -> Vec<Box<dyn HarnessAdapter>> {
    vec![Box::new(ClaudeCode::from_env())]
}

pub trait HarnessAdapter {
    /// A stable ID, such as `claude-code`. Catalog entries and the CLI use it.
    fn id(&self) -> &'static str;

    /// The name that people know the harness by, such as `Claude Code`.
    fn name(&self) -> &'static str;

    /// Where the harness keeps the skills, agents, and commands that it loads for the user outside
    /// plugins, such as `~/.claude`. `None` when the harness is not on this machine.
    fn standalone_dir(&self) -> Option<PathBuf>;

    /// What the harness has that the catalog can import. Read only.
    fn discover(&self) -> Result<Discovery>;

    /// The skills, agents, and commands in [`standalone_dir`](Self::standalone_dir). The default
    /// reads them in the plugin layout: `skills/<name>/SKILL.md`, `agents/*.md`, and
    /// `commands/*.md`. A harness with another layout overrides this.
    fn standalone(&self) -> Option<Result<MarketplacePreview>> {
        self.standalone_dir()
            .map(|dir| standalone(&dir, self.id(), self.name()))
    }
}

/// Whether a harness loads a plugin for the user, in every project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PluginStatus {
    Enabled,
    /// Installed for the user, but turned off.
    Disabled,
    /// Not installed for the user. It can still be installed in a project.
    NotInstalled,
}

/// What one harness has that the catalog can import.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Discovery {
    /// The adapter's ID, such as `claude-code`.
    pub harness: &'static str,
    pub name: &'static str,
    /// Where the harness keeps its configuration. `None` when it is not on this machine.
    pub config_dir: Option<PathBuf>,
    /// The marketplaces that the harness knows, by name.
    pub marketplaces: Vec<FoundMarketplace>,
    /// Skills, agents, and commands outside plugins. `None` when there are none.
    pub standalone: Option<FoundStandalone>,
    /// Installed plugins, as `plugin@marketplace`, whose marketplace the harness does not
    /// describe, so they cannot be imported. Plugins of the other marketplaces come with them.
    pub unmatched_plugins: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundMarketplace {
    pub name: String,
    /// How the catalog would add it. `None` when the catalog cannot use its kind of source.
    pub source: Option<MarketplaceSource>,
    /// Why it cannot be imported.
    pub problem: Option<String>,
    /// A copy that the harness keeps. Importing clones from it instead of the network.
    #[serde(skip)]
    pub local_copy: Option<PathBuf>,
    /// The plugins installed from it, in any scope.
    pub installed_plugins: Vec<String>,
    /// Whether the catalog already has this source. Set by the catalog.
    pub in_catalog: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundStandalone {
    pub source: MarketplaceSource,
    pub path: PathBuf,
    pub skills: usize,
    pub agents: usize,
    pub commands: usize,
    /// Whether the catalog already has this source. Set by the catalog.
    pub in_catalog: bool,
}

/// One thing to import from a harness.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ImportItem {
    /// A marketplace, by the name the harness knows it by. Its plugins come with it.
    Marketplace { name: String },
    /// The harness's skills, agents, and commands outside plugins.
    Standalone,
}
