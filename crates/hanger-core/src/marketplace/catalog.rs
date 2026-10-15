use std::fs;
use std::io;
use std::path::{Component as PathPart, Path, PathBuf};

use rusqlite::{Connection, ErrorCode, OptionalExtension, params};
use serde::Serialize;

use super::scan::{self, MarketplacePreview, PluginPreview, PluginSource};
use super::source::MarketplaceSource;
use crate::harness::claude_code::UserPlugins;
use crate::harness::{self, Discovery, HarnessAdapter, ImportItem, PluginStatus};
use crate::{Error, Result, git, paths, store};

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
                        status: match plugin.source {
                            PluginSource::Harness { .. } => PluginStatus::Enabled,
                            _ => claude.status(&plugin.name, &preview.name),
                        },
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
