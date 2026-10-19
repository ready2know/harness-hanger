use std::fs;
use std::io;
use std::path::{Component as PathPart, Path, PathBuf};

use rusqlite::{Connection, ErrorCode, OptionalExtension, params};
use serde::Serialize;
use serde_json::{Map, Value};

use super::scan::{
    self, Component, ComponentKind, MarketplacePreview, PluginPreview, PluginSource,
};
use super::source::MarketplaceSource;
use crate::harness::claude_code::UserPlugins;
use crate::harness::{self, Discovery, HarnessAdapter, ImportItem, PluginStatus};
use crate::{Error, Result, frontmatter, git, paths, store};

/// The names a plugin's README can have, in the order they are tried.
const README: &[&str] = &["README.md", "readme.md", "Readme.md"];

/// The marketplaces a person added to harness-hanger, kept in its own database.
///
/// This catalog is separate from Claude Code: adding a marketplace here does not register it with
/// the harness. GitHub sources are cloned into the data directory; local ones are read in place.
/// The catalog can also hold a harness's own skills, agents, and commands, read in place.
pub struct Catalog {
    connection: Connection,
    data_dir: PathBuf,
    adapters: Vec<Box<dyn HarnessAdapter>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceSummary {
    pub id: i64,
    pub source: MarketplaceSource,
    /// When it was added, as an RFC 3339 UTC timestamp.
    pub added_at: String,
    /// Where the marketplace is read from: the clone of a GitHub source, the local directory, or
    /// the harness's directory.
    pub root: PathBuf,
    /// From the manifest. `None` when `error` is set.
    pub name: Option<String>,
    pub description: Option<String>,
    /// Always 0 for a harness's own components, which are not plugins.
    pub plugin_count: usize,
    /// Why the manifest cannot be read, for example because a local directory was moved.
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceDetails {
    pub summary: MarketplaceSummary,
    /// `None` when the manifest cannot be read. `summary.error` says why.
    pub preview: Option<MarketplacePreview>,
}

/// A plugin from one of the catalog's marketplaces, with its status in Claude Code.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPlugin {
    pub marketplace_id: i64,
    /// The marketplace's name. Claude Code identifies the plugin as `plugin@marketplace`.
    pub marketplace: String,
    pub status: PluginStatus,
    pub plugin: PluginPreview,
}

/// A plugin of a catalog marketplace, for its own page.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginDetails {
    pub marketplace: MarketplaceSummary,
    pub status: PluginStatus,
    pub plugin: PluginPreview,
    /// The README in the plugin's directory, when there is one that can be read.
    pub readme: Option<String>,
    /// The plugin in a browser: its directory on GitHub, or the repository it is installed from.
    pub web_url: Option<String>,
}

/// A skill, an agent, or a command of a catalog marketplace, for its own page.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentDetails {
    pub marketplace: MarketplaceSummary,
    /// The status of the plugin that provides it.
    pub status: PluginStatus,
    pub plugin: PluginPreview,
    pub kind: ComponentKind,
    pub component: Component,
    /// `None` for a command written inline in a manifest, or when `file_error` is set.
    pub file: Option<ComponentFile>,
    /// Why the file cannot be read.
    pub file_error: Option<String>,
    /// The file on GitHub, for a marketplace cloned from there.
    pub web_url: Option<String>,
}

/// The file that defines a component.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentFile {
    /// The whole file.
    pub text: String,
    /// The top-level frontmatter fields.
    pub frontmatter: Map<String, Value>,
    /// The Markdown after the frontmatter.
    pub body: String,
}

impl ComponentFile {
    fn new(text: String) -> Self {
        Self {
            frontmatter: frontmatter::fields(&text),
            body: frontmatter::parse(&text).body.to_owned(),
            text,
        }
    }
}

/// The plugins of the catalog's marketplaces, and the marketplaces that could not be read.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPlugins {
    pub plugins: Vec<CatalogPlugin>,
    /// Left out because they cannot be read, for example after a local directory was moved.
    /// Each one's `error` says why.
    pub unreadable: Vec<MarketplaceSummary>,
}

struct Row {
    id: i64,
    source: MarketplaceSource,
    added_at: String,
}

impl Catalog {
    /// Opens the catalog kept in `data_dir`, creating it on first use.
    pub fn open(data_dir: impl Into<PathBuf>) -> Result<Self> {
        let data_dir = data_dir.into();
        let connection = store::open(&data_dir)?;
        Ok(Self {
            connection,
            data_dir,
            adapters: harness::adapters(),
        })
    }

    /// Opens the catalog in the default [data directory](paths::data_dir).
    pub fn open_default() -> Result<Self> {
        Self::open(paths::data_dir()?)
    }

    /// Uses `adapters` instead of [`harness::adapters`], such as fixtures in tests.
    #[must_use]
    pub fn with_adapters(mut self, adapters: Vec<Box<dyn HarnessAdapter>>) -> Self {
        self.adapters = adapters;
        self
    }

    pub fn list(&self) -> Result<Vec<MarketplaceSummary>> {
        let mut statement = self
            .connection
            .prepare("SELECT id, kind, location, added_at FROM marketplace ORDER BY id")?;
        let rows = statement
            .query_map([], read_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows
            .into_iter()
            .flatten()
            .map(|row| self.summary(row))
            .collect())
    }

    pub fn get(&self, id: i64) -> Result<MarketplaceSummary> {
        let row = self
            .connection
            .query_row(
                "SELECT id, kind, location, added_at FROM marketplace WHERE id = ?1",
                [id],
                read_row,
            )
            .optional()?
            .flatten()
            .ok_or(Error::MarketplaceNotFound(id))?;
        Ok(self.summary(row))
    }

    /// Adds a marketplace. A GitHub source is cloned, which can take a while.
    pub fn add(&self, source: &MarketplaceSource) -> Result<MarketplaceSummary> {
        self.add_from(source, None)
    }

    /// What the harnesses have that the catalog can import, and which of it the catalog has.
    pub fn import_candidates(&self) -> Result<Vec<Discovery>> {
        self.adapters
            .iter()
            .map(|adapter| {
                let mut discovery = adapter.discover()?;
                for marketplace in &mut discovery.marketplaces {
                    let Some(source) = &marketplace.source else {
                        continue;
                    };
                    // Someone may have added the harness's copy as a local directory before.
                    let copy_in_catalog = match &marketplace.local_copy {
                        Some(path) => {
                            self.contains(&MarketplaceSource::Local { path: path.clone() })?
                        }
                        None => false,
                    };
                    marketplace.in_catalog = self.contains(source)? || copy_in_catalog;
                }
                if let Some(standalone) = &mut discovery.standalone {
                    standalone.in_catalog = self.contains(&standalone.source)?;
                }
                Ok(discovery)
            })
            .collect()
    }

    /// Imports one thing that the harness `harness` has. A GitHub marketplace is cloned from the
    /// copy that the harness keeps, when there is one, so that no network is needed.
    pub fn import(&self, harness: &str, item: &ImportItem) -> Result<MarketplaceSummary> {
        let adapter = self.adapter(harness)?;
        let discovery = adapter.discover()?;
        let not_importable = |item: &str, reason: String| Error::NotImportable {
            item: item.to_owned(),
            reason,
        };
        match item {
            ImportItem::Standalone => {
                let found = discovery.standalone.ok_or_else(|| {
                    not_importable(
                        &format!("{}'s own skills, agents, and commands", adapter.name()),
                        "there are none".into(),
                    )
                })?;
                self.add(&found.source)
            }
            ImportItem::Marketplace { name } => {
                let found = discovery
                    .marketplaces
                    .into_iter()
                    .find(|marketplace| &marketplace.name == name)
                    .ok_or_else(|| {
                        not_importable(name, format!("{} does not know it", adapter.name()))
                    })?;
                let source = found
                    .source
                    .ok_or_else(|| not_importable(name, found.problem.unwrap_or_default()))?;
                self.add_from(&source, found.local_copy.as_deref())
            }
        }
    }

    fn add_from(
        &self,
        source: &MarketplaceSource,
        local_copy: Option<&Path>,
    ) -> Result<MarketplaceSummary> {
        // The entry is claimed before the clone, so that the CLI and the app cannot both clone
        // the same source into one directory.
        let inserted = self.connection.execute(
            "INSERT INTO marketplace (kind, location) VALUES (?1, ?2)",
            params![source.kind(), source.location()?],
        );
        match inserted {
            Err(rusqlite::Error::SqliteFailure(error, _))
                if error.code == ErrorCode::ConstraintViolation =>
            {
                return Err(Error::AlreadyAdded(source.to_string()));
            }
            inserted => inserted?,
        };
        let id = self.connection.last_insert_rowid();

        let root = self.root(source);
        let fetched = match source {
            MarketplaceSource::GitHub { owner, repo } => clone(
                local_copy,
                &format!("https://github.com/{owner}/{repo}.git"),
                &self.clones_dir().join(format!(".adding-{id}")),
                &root,
            ),
            MarketplaceSource::Local { .. } | MarketplaceSource::Harness { .. } => {
                self.read(source, &root).map(drop)
            }
        };
        if let Err(error) = fetched {
            self.connection
                .execute("DELETE FROM marketplace WHERE id = ?1", [id])?;
            return Err(error);
        }
        self.get(id)
    }

    /// The marketplace and everything it contains.
    pub fn details(&self, id: i64) -> Result<MarketplaceDetails> {
        let summary = self.get(id)?;
        // Fails only when the manifest cannot be read, which `summary.error` already reports.
        let preview = self.read(&summary.source, &summary.root).ok();
        Ok(MarketplaceDetails { summary, preview })
    }

    /// Every plugin of the catalog's marketplaces, with its status in Claude Code.
    ///
    /// The status matches plugins by name and marketplace name, as Claude Code does. A harness's
    /// own skills, agents, and commands are always enabled: it loads them for the user.
    pub fn plugins(&self, claude: &UserPlugins) -> Result<CatalogPlugins> {
        let mut plugins = Vec::new();
        let mut unreadable = Vec::new();
        for mut summary in self.list()? {
            if summary.error.is_some() {
                unreadable.push(summary);
                continue;
            }
            match self.read(&summary.source, &summary.root) {
                Ok(preview) => {
                    plugins.extend(preview.plugins.into_iter().map(|plugin| CatalogPlugin {
                        marketplace_id: summary.id,
                        marketplace: preview.name.clone(),
                        status: status(&plugin, &preview.name, claude),
                        plugin,
                    }));
                }
                // The manifest changed since `list` read it.
                Err(error) => {
                    summary.error = Some(error.to_string());
                    unreadable.push(summary);
                }
            }
        }
        Ok(CatalogPlugins {
            plugins,
            unreadable,
        })
    }

    /// A plugin of the marketplace `id`, with its status in Claude Code and its README.
    pub fn plugin_details(
        &self,
        id: i64,
        name: &str,
        claude: &UserPlugins,
    ) -> Result<PluginDetails> {
        let (marketplace, status, plugin) = self.find_plugin(id, name, claude)?;
        let (readme, web_url) = match &plugin.source {
            PluginSource::Local { path } => {
                (readme(&marketplace, path), github_url(&marketplace, path))
            }
            PluginSource::External { source, location } => {
                (None, external_url(source, location.as_deref()))
            }
            PluginSource::Harness { .. } => (None, None),
        };
        Ok(PluginDetails {
            marketplace,
            status,
            plugin,
            readme,
            web_url,
        })
    }

    /// A skill, an agent, or a command of the plugin `plugin` in the marketplace `id`, with its file.
    ///
    /// The file is read as the scan reads it: one that links outside a marketplace is not read.
    pub fn component_details(
        &self,
        id: i64,
        plugin: &str,
        kind: ComponentKind,
        name: &str,
        claude: &UserPlugins,
    ) -> Result<ComponentDetails> {
        let (marketplace, status, plugin) = self.find_plugin(id, plugin, claude)?;
        let component = plugin
            .components(kind)
            .iter()
            .find(|component| component.name == name)
            .cloned()
            .ok_or_else(|| Error::ComponentNotFound {
                plugin: plugin.name.clone(),
                kind: kind.noun(),
                name: name.to_owned(),
            })?;
        let (file, file_error) = match &component.path {
            None => (None, None),
            Some(path) => match read_file(&marketplace, path) {
                Ok(text) => (Some(ComponentFile::new(text)), None),
                Err(error) => (None, Some(error.to_string())),
            },
        };
        let web_url = component
            .path
            .as_deref()
            .and_then(|path| github_url(&marketplace, path));
        Ok(ComponentDetails {
            marketplace,
            status,
            plugin,
            kind,
            component,
            file,
            file_error,
            web_url,
        })
    }

    fn find_plugin(
        &self,
        id: i64,
        name: &str,
        claude: &UserPlugins,
    ) -> Result<(MarketplaceSummary, PluginStatus, PluginPreview)> {
        let marketplace = self.get(id)?;
        let preview = self.read(&marketplace.source, &marketplace.root)?;
        let plugin = preview
            .plugins
            .into_iter()
            .find(|plugin| plugin.name == name)
            .ok_or_else(|| Error::PluginNotFound {
                marketplace: preview.name.clone(),
                plugin: name.to_owned(),
            })?;
        let status = status(&plugin, &preview.name, claude);
        Ok((marketplace, status, plugin))
    }

    /// Removes a marketplace and, for a GitHub source, its clone. A local directory, and a
    /// harness's directory, are left as they are.
    pub fn remove(&self, id: i64) -> Result<()> {
        let summary = self.get(id)?;
        // The clone goes first, so that a failure keeps the entry and removing can be retried.
        if let MarketplaceSource::GitHub { .. } = summary.source {
            self.remove_clone(&summary.root)?;
        }
        self.connection
            .execute("DELETE FROM marketplace WHERE id = ?1", [id])?;
        Ok(())
    }

    fn contains(&self, source: &MarketplaceSource) -> Result<bool> {
        Ok(self
            .connection
            .query_row(
                "SELECT 1 FROM marketplace WHERE kind = ?1 AND location = ?2",
                params![source.kind(), source.location()?],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    fn adapter(&self, id: &str) -> Result<&dyn HarnessAdapter> {
        self.adapters
            .iter()
            .map(Box::as_ref)
            .find(|adapter| adapter.id() == id)
            .ok_or_else(|| Error::UnknownHarness(id.to_owned()))
    }

    /// Everything that an entry contains.
    fn read(&self, source: &MarketplaceSource, root: &Path) -> Result<MarketplacePreview> {
        match source {
            MarketplaceSource::Harness { harness } => {
                let adapter = self.adapter(harness)?;
                adapter
                    .standalone()
                    .ok_or_else(|| Error::HarnessMissing(adapter.name().to_owned()))?
            }
            MarketplaceSource::GitHub { .. } | MarketplaceSource::Local { .. } => {
                scan::preview(root)
            }
        }
    }

    fn clones_dir(&self) -> PathBuf {
        self.data_dir.join("marketplaces").join("github")
    }

    /// Removes a clone, and its owner's directory once no other clone is left in it.
    fn remove_clone(&self, dir: &Path) -> Result<()> {
        // Rows are checked when they are read. This guards against any other way of deleting a
        // directory that harness-hanger did not create.
        let is_clone = dir.strip_prefix(self.clones_dir()).is_ok_and(|rest| {
            rest.components().count() == 2
                && rest
                    .components()
                    .all(|part| matches!(part, PathPart::Normal(_)))
        });
        if !is_clone {
            return Err(Error::io(
                dir,
                io::Error::other("not a clone that harness-hanger made, so it is not deleted"),
            ));
        }
        remove_dir_if_exists(dir)?;
        if let Some(owner) = dir.parent() {
            // Fails, as intended, while the directory still holds another clone.
            let _ = fs::remove_dir(owner);
        }
        Ok(())
    }

    fn root(&self, source: &MarketplaceSource) -> PathBuf {
        match source {
            MarketplaceSource::GitHub { owner, repo } => self.clones_dir().join(owner).join(repo),
            MarketplaceSource::Local { path } => path.clone(),
            MarketplaceSource::Harness { harness } => self
                .adapter(harness)
                .ok()
                .and_then(HarnessAdapter::standalone_dir)
                .unwrap_or_default(),
        }
    }

    fn summary(&self, row: Row) -> MarketplaceSummary {
        let root = self.root(&row.source);
        let mut summary = MarketplaceSummary {
            id: row.id,
            source: row.source,
            added_at: row.added_at,
            root,
            name: None,
            description: None,
            plugin_count: 0,
            error: None,
        };
        let read = match &summary.source {
            // A harness's directory has no manifest. Its preview is small, so it is read whole.
            MarketplaceSource::Harness { .. } => self
                .read(&summary.source, &summary.root)
                .map(|preview| (preview.name, preview.description, 0)),
            MarketplaceSource::GitHub { .. } | MarketplaceSource::Local { .. } => {
                scan::read_manifest(&summary.root).map(|manifest| {
                    let description = manifest.description().map(str::to_owned);
                    let plugin_count = manifest
                        .plugin_entries()
                        .filter(|(_, entry)| entry.is_ok())
                        .count();
                    (manifest.name, description, plugin_count)
                })
            }
        };
        match read {
            Ok((name, description, plugin_count)) => {
                summary.name = Some(name);
                summary.description = description;
                summary.plugin_count = plugin_count;
            }
            Err(error) => summary.error = Some(error.to_string()),
        }
        summary
    }
}

/// Reads a file that the scan of `marketplace` found, following symlinks only where the scan
/// does.
fn read_file(marketplace: &MarketplaceSummary, path: &Path) -> Result<String> {
    let follow_links_out = matches!(marketplace.source, MarketplaceSource::Harness { .. });
    scan::read_inside(&marketplace.root, path, follow_links_out)
}

/// The README in `dir`. A plugin without one, or with one that cannot be read, shows none.
fn readme(marketplace: &MarketplaceSummary, dir: &Path) -> Option<String> {
    README
        .iter()
        .map(|name| dir.join(name))
        .find(|path| path.is_file())
        .and_then(|path| read_file(marketplace, &path).ok())
}

/// `path` on GitHub, at the commit of the clone, for a marketplace cloned from there.
fn github_url(marketplace: &MarketplaceSummary, path: &Path) -> Option<String> {
    let MarketplaceSource::GitHub { owner, repo } = &marketplace.source else {
        return None;
    };
    let relative = path.strip_prefix(&marketplace.root).ok()?;
    // A path that leaves the clone, such as a plugin source with `..`, is not in the repository.
    let inside = relative
        .components()
        .all(|part| matches!(part, PathPart::Normal(_)));
    if !inside || !path.exists() {
        return None;
    }
    // The default branch, when the clone cannot say which commit it has.
    let commit = git::head(&marketplace.root).unwrap_or_else(|_| "HEAD".to_owned());
    Some(github_page(owner, repo, &commit, relative, path.is_dir()))
}

/// A plugin's status in Claude Code. A harness's own skills, agents, and commands are always
/// enabled: it loads them for the user.
fn status(plugin: &PluginPreview, marketplace: &str, claude: &UserPlugins) -> PluginStatus {
    match plugin.source {
        PluginSource::Harness { .. } => PluginStatus::Enabled,
        _ => claude.status(&plugin.name, marketplace),
    }
}

/// The page of `relative` in a GitHub repository at `commit`: a directory, or a file.
fn github_page(owner: &str, repo: &str, commit: &str, relative: &Path, is_dir: bool) -> String {
    let parts: Vec<_> = relative
        .components()
        .filter_map(|part| match part {
            PathPart::Normal(name) => Some(encode_segment(&name.to_string_lossy())),
            _ => None,
        })
        .collect();
    let view = if is_dir || parts.is_empty() {
        "tree"
    } else {
        "blob"
    };
    let base = format!("https://github.com/{owner}/{repo}/{view}/{commit}");
    if parts.is_empty() {
        base
    } else {
        format!("{base}/{}", parts.join("/"))
    }
}

/// Percent-encodes one segment of a URL path.
pub(crate) fn encode_segment(segment: &str) -> String {
    segment
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
                char::from(byte).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

/// The repository of a plugin that is installed from elsewhere, when a browser can show it.
fn external_url(source: &str, location: Option<&str>) -> Option<String> {
    let location = location?;
    match source {
        "github" => Some(format!("https://github.com/{location}")),
        "url" if location.starts_with("https://") => {
            Some(location.strip_suffix(".git").unwrap_or(location).to_owned())
        }
        _ => None,
    }
}

/// Reads a row, or `None` when its kind is unknown, such as one written by a newer version.
fn read_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Option<Row>> {
    let id = row.get(0)?;
    let kind: String = row.get(1)?;
    let location = row.get(2)?;
    let added_at = row.get(3)?;
    Ok(
        MarketplaceSource::from_row(&kind, location).map(|source| Row {
            id,
            source,
            added_at,
        }),
    )
}

/// Clones a GitHub marketplace into `staging`, checks it, and moves it to `dest`.
///
/// `local_copy` is a copy that a harness keeps: cloning from it is fast and works offline, and
/// `origin` then points at `url`. Without it, or when it fails, the clone comes from `url`. The
/// caller holds the catalog entry for `dest`, so nothing else uses that directory. Nothing is left
/// behind on failure.
fn clone(local_copy: Option<&Path>, url: &str, staging: &Path, dest: &Path) -> Result<()> {
    let result = (|| {
        // Leftovers of an add that was interrupted.
        remove_dir_if_exists(staging)?;
        remove_dir_if_exists(dest)?;
        for dir in [staging.parent(), dest.parent()].into_iter().flatten() {
            fs::create_dir_all(dir).map_err(|source| Error::io(dir, source))?;
        }
        let from_copy = local_copy
            .and_then(Path::to_str)
            .is_some_and(|copy| git::shallow_clone(copy, staging).is_ok());
        if from_copy {
            git::set_origin(staging, url)?;
        } else {
            // A failed clone from the copy can leave a partial directory.
            remove_dir_if_exists(staging)?;
            git::shallow_clone(url, staging)?;
        }
        scan::read_manifest(staging)?;
        fs::rename(staging, dest).map_err(|source| Error::io(dest, source))
    })();
    if result.is_err() {
        // The error being returned matters more than a failed cleanup.
        let _ = remove_dir_if_exists(staging);
        if let Some(owner) = dest.parent() {
            let _ = fs::remove_dir(owner);
        }
    }
    result
}

fn remove_dir_if_exists(dir: &Path) -> Result<()> {
    match fs::remove_dir_all(dir) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(Error::io(dir, error)),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/marketplace")
    }

    fn local(path: &Path) -> MarketplaceSource {
        MarketplaceSource::Local {
            path: path.to_owned(),
        }
    }

    fn write_marketplace(dir: &Path, name: &str) {
        fs::create_dir_all(dir.join(".claude-plugin")).unwrap();
        fs::write(
            dir.join(".claude-plugin/marketplace.json"),
            format!(r#"{{ "name": "{name}", "plugins": [] }}"#),
        )
        .unwrap();
    }

    #[test]
    fn adds_lists_and_previews_a_local_marketplace() {
        let data = tempfile::tempdir().unwrap();
        let catalog = Catalog::open(data.path()).unwrap();

        let added = catalog.add(&local(&fixture())).unwrap();

        assert_eq!(added.name.as_deref(), Some("fixture-market"));
        assert_eq!(
            added.description.as_deref(),
            Some("A marketplace for tests.")
        );
        // The entry without a name or a source is not counted.
        assert_eq!(added.plugin_count, 12);
        assert_eq!(added.root, fixture());
        assert_eq!(added.error, None);
        let listed = catalog.list().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, added.id);
        let details = catalog.details(added.id).unwrap();
        assert_eq!(details.preview.unwrap().plugins.len(), 12);
    }

    #[test]
    fn lists_the_plugins_of_every_marketplace_with_their_claude_code_status() {
        let data = tempfile::tempdir().unwrap();
        let moved = tempfile::tempdir().unwrap();
        write_marketplace(moved.path(), "moved");
        let claude = tempfile::tempdir().unwrap();
        fs::create_dir_all(claude.path().join("plugins")).unwrap();
        fs::write(
            claude.path().join("plugins/installed_plugins.json"),
            r#"{ "version": 2, "plugins": {
                "standard@fixture-market": [{ "scope": "user" }],
                "custom@fixture-market": [{ "scope": "user" }],
                "bare@fixture-market": [{ "scope": "project", "projectPath": "/p" }]
            } }"#,
        )
        .unwrap();
        fs::write(
            claude.path().join("settings.json"),
            r#"{ "enabledPlugins": { "standard@fixture-market": true, "bare@fixture-market": true } }"#,
        )
        .unwrap();
        let catalog = Catalog::open(data.path()).unwrap();
        let fixture_id = catalog.add(&local(&fixture())).unwrap().id;
        catalog.add(&local(moved.path())).unwrap();
        fs::remove_dir_all(moved.path()).unwrap();

        let CatalogPlugins {
            plugins,
            unreadable,
        } = catalog
            .plugins(&UserPlugins::read(claude.path()).unwrap())
            .unwrap();

        // The moved marketplace is reported apart, and the fixture's 12 plugins are listed.
        assert_eq!(plugins.len(), 12);
        assert_eq!(unreadable.len(), 1);
        assert!(
            unreadable[0]
                .error
                .as_deref()
                .is_some_and(|error| error.contains("the directory does not exist"))
        );
        let status = |name: &str| {
            let found = plugins.iter().find(|p| p.plugin.name == name).unwrap();
            assert_eq!(found.marketplace, "fixture-market");
            assert_eq!(found.marketplace_id, fixture_id);
            found.status
        };
        assert_eq!(status("standard"), PluginStatus::Enabled);
        assert_eq!(status("custom"), PluginStatus::Disabled);
        assert_eq!(status("bare"), PluginStatus::NotInstalled);
        assert_eq!(status("remote"), PluginStatus::NotInstalled);
    }

    fn nothing_installed() -> UserPlugins {
        UserPlugins::read(tempfile::tempdir().unwrap().path()).unwrap()
    }

    #[test]
    fn shows_a_plugin_with_its_readme() {
        let data = tempfile::tempdir().unwrap();
        let catalog = Catalog::open(data.path()).unwrap();
        let id = catalog.add(&local(&fixture())).unwrap().id;

        let details = catalog
            .plugin_details(id, "standard", &nothing_installed())
            .unwrap();

        assert_eq!(details.marketplace.id, id);
        assert_eq!(details.status, PluginStatus::NotInstalled);
        assert_eq!(details.plugin.version.as_deref(), Some("1.2.0"));
        assert_eq!(
            details.readme.as_deref(),
            Some("# Standard\n\nA plugin for tests.\n")
        );
        // Only a GitHub marketplace has a page in a browser.
        assert_eq!(details.web_url, None);
        assert_eq!(
            catalog
                .plugin_details(id, "remote", &nothing_installed())
                .unwrap()
                .web_url
                .as_deref(),
            Some("https://github.com/acme/remote")
        );
    }

    #[test]
    fn shows_a_component_with_its_file() {
        let data = tempfile::tempdir().unwrap();
        let catalog = Catalog::open(data.path()).unwrap();
        let id = catalog.add(&local(&fixture())).unwrap().id;

        let details = catalog
            .component_details(
                id,
                "standard",
                ComponentKind::Skill,
                "alpha-skill",
                &nothing_installed(),
            )
            .unwrap();

        assert_eq!(details.plugin.name, "standard");
        assert_eq!(details.kind, ComponentKind::Skill);
        assert_eq!(
            details.component.path,
            Some(fixture().join("plugins/standard/skills/alpha/SKILL.md"))
        );
        assert_eq!(details.file_error, None);
        let file = details.file.unwrap();
        assert!(file.text.starts_with("---\nname: alpha-skill\n"));
        assert_eq!(file.body, "\n# Alpha\n\nDo alpha things.\n");
        assert_eq!(
            file.frontmatter.get("allowed-tools"),
            Some(&serde_json::json!(["Read", "Grep"]))
        );
    }

    #[test]
    fn an_inline_command_has_no_file() {
        let data = tempfile::tempdir().unwrap();
        let catalog = Catalog::open(data.path()).unwrap();
        let id = catalog.add(&local(&fixture())).unwrap().id;

        let details = catalog
            .component_details(
                id,
                "inline",
                ComponentKind::Command,
                "hello",
                &nothing_installed(),
            )
            .unwrap();

        assert_eq!(
            details.component.description.as_deref(),
            Some("Greets you.")
        );
        assert!(details.file.is_none() && details.file_error.is_none());
    }

    #[test]
    fn names_the_plugin_or_component_that_is_not_there() {
        let data = tempfile::tempdir().unwrap();
        let catalog = Catalog::open(data.path()).unwrap();
        let id = catalog.add(&local(&fixture())).unwrap().id;
        let claude = nothing_installed();

        let error = catalog.plugin_details(id, "nope", &claude).unwrap_err();
        assert_eq!(error.to_string(), "fixture-market has no plugin named nope");

        let error = catalog
            .component_details(id, "standard", ComponentKind::Agent, "alpha-skill", &claude)
            .unwrap_err();
        assert_eq!(error.to_string(), "standard has no agent named alpha-skill");
    }

    #[cfg(unix)]
    #[test]
    fn does_not_read_a_component_file_that_links_outside_the_marketplace() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("secret"), "Secret.").unwrap();
        let root = tmp.path().join("market");
        fs::create_dir_all(root.join(".claude-plugin")).unwrap();
        fs::write(
            root.join(".claude-plugin/marketplace.json"),
            r#"{ "name": "links", "plugins": [{ "name": "p", "source": "./" }] }"#,
        )
        .unwrap();
        fs::create_dir_all(root.join("skills/evil")).unwrap();
        std::os::unix::fs::symlink(tmp.path().join("secret"), root.join("skills/evil/SKILL.md"))
            .unwrap();
        std::os::unix::fs::symlink(tmp.path().join("secret"), root.join("README.md")).unwrap();
        let data = tempfile::tempdir().unwrap();
        let catalog = Catalog::open(data.path()).unwrap();
        let id = catalog.add(&local(&root)).unwrap().id;
        let claude = nothing_installed();

        let details = catalog
            .component_details(id, "p", ComponentKind::Skill, "evil", &claude)
            .unwrap();

        assert!(details.file.is_none());
        assert!(
            details
                .file_error
                .is_some_and(|error| error.contains("links outside the marketplace")),
        );
        assert_eq!(
            catalog.plugin_details(id, "p", &claude).unwrap().readme,
            None
        );
    }

    #[test]
    fn links_to_files_and_directories_on_github() {
        let page =
            |path: &str, is_dir| github_page("acme", "tools", "abc123", Path::new(path), is_dir);

        assert_eq!(
            page("plugins/review/skills/my skill/SKILL.md", false),
            "https://github.com/acme/tools/blob/abc123/plugins/review/skills/my%20skill/SKILL.md"
        );
        assert_eq!(
            page("plugins/review", true),
            "https://github.com/acme/tools/tree/abc123/plugins/review"
        );
        assert_eq!(page("", true), "https://github.com/acme/tools/tree/abc123");
    }

    #[test]
    fn links_only_to_external_sources_that_a_browser_can_show() {
        assert_eq!(
            external_url("url", Some("https://example.com/mono.git")).as_deref(),
            Some("https://example.com/mono")
        );
        assert_eq!(external_url("url", Some("git@example.com:mono.git")), None);
        assert_eq!(external_url("npm", Some("@acme/plugin")), None);
        assert_eq!(external_url("github", None), None);
    }

    #[test]
    fn keeps_the_catalog_between_openings() {
        let data = tempfile::tempdir().unwrap();
        Catalog::open(data.path())
            .unwrap()
            .add(&local(&fixture()))
            .unwrap();

        assert_eq!(Catalog::open(data.path()).unwrap().list().unwrap().len(), 1);
    }

    #[test]
    fn refuses_to_add_the_same_source_twice() {
        let data = tempfile::tempdir().unwrap();
        let catalog = Catalog::open(data.path()).unwrap();
        catalog.add(&local(&fixture())).unwrap();

        let error = catalog.add(&local(&fixture())).unwrap_err();

        assert!(matches!(error, Error::AlreadyAdded(_)), "{error}");
    }

    #[test]
    fn refuses_a_directory_that_is_not_a_marketplace() {
        let data = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let catalog = Catalog::open(data.path()).unwrap();

        let error = catalog.add(&local(other.path())).unwrap_err();

        assert!(matches!(error, Error::NotAMarketplace { .. }), "{error}");
        assert!(catalog.list().unwrap().is_empty());
    }

    #[test]
    fn reports_a_local_marketplace_that_was_moved() {
        let data = tempfile::tempdir().unwrap();
        let market = tempfile::tempdir().unwrap();
        write_marketplace(market.path(), "moving");
        let catalog = Catalog::open(data.path()).unwrap();
        let added = catalog.add(&local(market.path())).unwrap();

        fs::remove_dir_all(market.path()).unwrap();

        let details = catalog.details(added.id).unwrap();
        assert_eq!(details.summary.name, None);
        assert!(details.preview.is_none());
        assert!(
            details
                .summary
                .error
                .is_some_and(|error| error.contains("the directory does not exist"))
        );
    }

    #[test]
    fn removing_a_local_marketplace_leaves_its_directory() {
        let data = tempfile::tempdir().unwrap();
        let market = tempfile::tempdir().unwrap();
        write_marketplace(market.path(), "kept");
        let catalog = Catalog::open(data.path()).unwrap();
        let added = catalog.add(&local(market.path())).unwrap();

        catalog.remove(added.id).unwrap();

        assert!(catalog.list().unwrap().is_empty());
        assert!(
            market
                .path()
                .join(".claude-plugin/marketplace.json")
                .is_file()
        );
        assert!(matches!(
            catalog.remove(added.id),
            Err(Error::MarketplaceNotFound(_))
        ));
    }

    #[test]
    fn never_reuses_the_id_of_a_removed_marketplace() {
        let data = tempfile::tempdir().unwrap();
        let market = tempfile::tempdir().unwrap();
        write_marketplace(market.path(), "again");
        let catalog = Catalog::open(data.path()).unwrap();
        let first = catalog.add(&local(market.path())).unwrap();
        catalog.remove(first.id).unwrap();

        let second = catalog.add(&local(market.path())).unwrap();

        assert_ne!(second.id, first.id);
    }

    /// A Claude Code config directory with a GitHub marketplace that it keeps a copy of, a local
    /// marketplace, one it cannot describe, and standalone skills and agents.
    fn claude_home(tmp: &Path) -> PathBuf {
        let config = tmp.join("claude");
        let copy = config.join("plugins/marketplaces/acme");
        write_marketplace(&copy, "acme");
        git_repo(&copy);
        let local = tmp.join("local-market");
        write_marketplace(&local, "local-market");
        fs::create_dir_all(config.join("skills/one")).unwrap();
        fs::write(config.join("skills/one/SKILL.md"), "---\nname: one\n---\n").unwrap();
        fs::create_dir_all(config.join("agents")).unwrap();
        fs::write(
            config.join("agents/reviewer.md"),
            "---\nname: reviewer\n---\n",
        )
        .unwrap();
        let known = serde_json::json!({
            // A repository that does not exist: the clone must come from the copy.
            "acme": {
                "source": { "source": "github", "repo": "hanger-test-no-such-owner/acme" },
                "installLocation": copy
            },
            "local-market": { "source": { "source": "directory", "path": local } },
            "packaged": { "source": { "source": "npm", "package": "x" } }
        });
        fs::write(
            config.join("plugins/known_marketplaces.json"),
            known.to_string(),
        )
        .unwrap();
        config
    }

    fn import_catalog(data: &Path, claude: &Path) -> Catalog {
        Catalog::open(data)
            .unwrap()
            .with_adapters(vec![Box::new(harness::ClaudeCode::new(claude))])
    }

    fn marketplace(name: &str) -> ImportItem {
        ImportItem::Marketplace { name: name.into() }
    }

    #[test]
    fn imports_a_github_marketplace_from_the_copy_the_harness_keeps() {
        let tmp = tempfile::tempdir().unwrap();
        let claude = claude_home(tmp.path());
        let catalog = import_catalog(&tmp.path().join("data"), &claude);

        let imported = catalog.import("claude-code", &marketplace("acme")).unwrap();

        assert_eq!(
            imported.source,
            MarketplaceSource::GitHub {
                owner: "hanger-test-no-such-owner".into(),
                repo: "acme".into()
            }
        );
        assert_eq!(imported.name.as_deref(), Some("acme"));
        let origin = git::detach(&mut Command::new("git"))
            .args(["remote", "get-url", "origin"])
            .current_dir(&imported.root)
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&origin.stdout).trim(),
            "https://github.com/hanger-test-no-such-owner/acme.git"
        );
    }

    #[test]
    fn links_a_component_of_a_github_marketplace_to_its_file_at_the_cloned_commit() {
        let tmp = tempfile::tempdir().unwrap();
        let claude = claude_home(tmp.path());
        let copy = claude.join("plugins/marketplaces/acme");
        fs::write(
            copy.join(".claude-plugin/marketplace.json"),
            r#"{ "name": "acme", "plugins": [{ "name": "p", "source": "./p" }] }"#,
        )
        .unwrap();
        fs::create_dir_all(copy.join("p/skills/my skill")).unwrap();
        fs::write(
            copy.join("p/skills/my skill/SKILL.md"),
            "---\nname: s\n---\n",
        )
        .unwrap();
        git_repo(&copy);
        let catalog = import_catalog(&tmp.path().join("data"), &claude);
        let id = catalog
            .import("claude-code", &marketplace("acme"))
            .unwrap()
            .id;
        let commit = git::head(&copy).unwrap();
        let claude_plugins = nothing_installed();

        let skill = catalog
            .component_details(id, "p", ComponentKind::Skill, "s", &claude_plugins)
            .unwrap();
        let plugin = catalog.plugin_details(id, "p", &claude_plugins).unwrap();

        let repo = "https://github.com/hanger-test-no-such-owner/acme";
        assert_eq!(
            skill.web_url.unwrap(),
            format!("{repo}/blob/{commit}/p/skills/my%20skill/SKILL.md")
        );
        assert_eq!(plugin.web_url.unwrap(), format!("{repo}/tree/{commit}/p"));
    }

    #[test]
    fn imports_a_local_marketplace_and_the_standalone_components() {
        let tmp = tempfile::tempdir().unwrap();
        let claude = claude_home(tmp.path());
        let catalog = import_catalog(&tmp.path().join("data"), &claude);

        let local = catalog
            .import("claude-code", &marketplace("local-market"))
            .unwrap();
        let standalone = catalog
            .import("claude-code", &ImportItem::Standalone)
            .unwrap();

        assert_eq!(local.root, tmp.path().join("local-market"));
        assert_eq!(standalone.name.as_deref(), Some("Claude Code"));
        assert_eq!(standalone.root, claude);
        assert_eq!(standalone.plugin_count, 0);
        let plugins = catalog.plugins(&UserPlugins::default()).unwrap().plugins;
        let own = plugins
            .iter()
            .find(|entry| entry.marketplace_id == standalone.id)
            .unwrap();
        // The harness loads its own components for the user, so they are enabled.
        assert_eq!(own.status, PluginStatus::Enabled);
        assert_eq!(own.plugin.skills[0].name, "one");
        assert_eq!(own.plugin.agents[0].name, "reviewer");

        catalog.remove(standalone.id).unwrap();
        assert!(claude.join("skills/one/SKILL.md").is_file());
    }

    #[test]
    fn marks_what_the_catalog_already_has() {
        let tmp = tempfile::tempdir().unwrap();
        let claude = claude_home(tmp.path());
        let catalog = import_catalog(&tmp.path().join("data"), &claude);
        catalog
            .import("claude-code", &marketplace("local-market"))
            .unwrap();

        let discoveries = catalog.import_candidates().unwrap();

        let discovery = &discoveries[0];
        let in_catalog: Vec<_> = discovery
            .marketplaces
            .iter()
            .map(|m| (m.name.as_str(), m.in_catalog))
            .collect();
        assert_eq!(
            in_catalog,
            [("acme", false), ("local-market", true), ("packaged", false)]
        );
        assert!(!discovery.standalone.as_ref().unwrap().in_catalog);
    }

    #[test]
    fn counts_the_harness_copy_added_as_a_local_directory_as_in_the_catalog() {
        let tmp = tempfile::tempdir().unwrap();
        let claude = claude_home(tmp.path());
        let catalog = import_catalog(&tmp.path().join("data"), &claude);
        catalog
            .add(&local(&claude.join("plugins/marketplaces/acme")))
            .unwrap();

        let discoveries = catalog.import_candidates().unwrap();

        let acme = &discoveries[0].marketplaces[0];
        assert_eq!(acme.name, "acme");
        assert!(acme.in_catalog);
    }

    #[test]
    fn explains_what_cannot_be_imported() {
        let tmp = tempfile::tempdir().unwrap();
        let claude = claude_home(tmp.path());
        let catalog = import_catalog(&tmp.path().join("data"), &claude);

        for (harness, item, expected) in [
            ("claude-code", marketplace("packaged"), "from a npm source"),
            (
                "claude-code",
                marketplace("missing"),
                "Claude Code does not know it",
            ),
            (
                "cursor",
                ImportItem::Standalone,
                "cannot import from cursor",
            ),
        ] {
            let error = catalog.import(harness, &item).unwrap_err();
            assert!(error.to_string().contains(expected), "{error}");
        }
        assert!(catalog.list().unwrap().is_empty());
    }

    #[test]
    fn keeps_github_clones_in_the_data_directory() {
        let data = tempfile::tempdir().unwrap();
        let catalog = Catalog::open(data.path()).unwrap();

        let root = catalog.root(&MarketplaceSource::GitHub {
            owner: "acme".into(),
            repo: "tools".into(),
        });

        assert_eq!(root, data.path().join("marketplaces/github/acme/tools"));
    }

    fn git_repo(dir: &Path) {
        for args in [
            &["init", "--quiet"][..],
            &["add", "."],
            &["commit", "--quiet", "-m", "init"],
        ] {
            let status = git::detach(&mut Command::new("git"))
                .args(["-c", "user.name=test", "-c", "user.email=test@example.com"])
                .args(args)
                .current_dir(dir)
                .status()
                .unwrap();
            assert!(status.success());
        }
    }

    #[test]
    fn clones_a_marketplace_repository() {
        let tmp = tempfile::tempdir().unwrap();
        let origin = tmp.path().join("origin");
        write_marketplace(&origin, "cloned");
        git_repo(&origin);
        let staging = tmp.path().join("clones/.adding-1");
        let dest = tmp.path().join("clones/acme/tools");

        clone(None, origin.to_str().unwrap(), &staging, &dest).unwrap();

        assert_eq!(scan::read_manifest(&dest).unwrap().name, "cloned");
        assert!(!staging.exists());
    }

    #[test]
    fn removes_a_clone_that_is_not_a_marketplace() {
        let tmp = tempfile::tempdir().unwrap();
        let origin = tmp.path().join("origin");
        fs::create_dir_all(&origin).unwrap();
        fs::write(origin.join("README.md"), "not a marketplace").unwrap();
        git_repo(&origin);
        let staging = tmp.path().join("clones/.adding-1");
        let owner = tmp.path().join("clones/acme");

        let error = clone(
            None,
            origin.to_str().unwrap(),
            &staging,
            &owner.join("tools"),
        )
        .unwrap_err();

        assert!(matches!(error, Error::NotAMarketplace { .. }), "{error}");
        assert!(!staging.exists());
        assert!(!owner.exists());
    }

    #[test]
    fn refuses_to_delete_anything_but_a_clone() {
        let data = tempfile::tempdir().unwrap();
        let catalog = Catalog::open(data.path()).unwrap();
        let clones = catalog.clones_dir();
        let elsewhere = data.path().join("elsewhere/acme/tools");
        fs::create_dir_all(&elsewhere).unwrap();

        for dir in [
            elsewhere.clone(),
            clones.join("acme"),
            clones.join("acme/tools/nested"),
            clones.join("acme/../../elsewhere"),
        ] {
            assert!(catalog.remove_clone(&dir).is_err(), "{}", dir.display());
        }
        assert!(elsewhere.is_dir());
    }
}
