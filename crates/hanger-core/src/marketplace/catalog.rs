use std::fs;
use std::io;
use std::path::{Component as PathPart, Path, PathBuf};

use rusqlite::{Connection, ErrorCode, OptionalExtension, params};
use serde::Serialize;

use super::scan::{self, MarketplacePreview, PluginPreview};
use super::source::MarketplaceSource;
use crate::claude_code::{PluginStatus, UserPlugins};
use crate::{Error, Result, git, paths, store};

/// The marketplaces a person added to harness-hanger, kept in its own database.
///
/// This catalog is separate from Claude Code: adding a marketplace here does not register it with
/// the harness. GitHub sources are cloned into the data directory; local ones are read in place.
pub struct Catalog {
    connection: Connection,
    data_dir: PathBuf,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceSummary {
    pub id: i64,
    pub source: MarketplaceSource,
    /// When it was added, as an RFC 3339 UTC timestamp.
    pub added_at: String,
    /// Where the marketplace is read from: the clone of a GitHub source, or the local directory.
    pub root: PathBuf,
    /// From the manifest. `None` when `error` is set.
    pub name: Option<String>,
    pub description: Option<String>,
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
        })
    }

    /// Opens the catalog in the default [data directory](paths::data_dir).
    pub fn open_default() -> Result<Self> {
        Self::open(paths::data_dir()?)
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

        let fetched = match source {
            MarketplaceSource::GitHub { owner, repo } => clone(
                &format!("https://github.com/{owner}/{repo}.git"),
                &self.clones_dir().join(format!(".adding-{id}")),
                &self.root(source),
            ),
            MarketplaceSource::Local { path } => scan::read_manifest(path).map(drop),
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
        let preview = scan::preview(&summary.root).ok();
        Ok(MarketplaceDetails { summary, preview })
    }

    /// Every plugin of the catalog's marketplaces, with its status in Claude Code.
    ///
    /// The status matches plugins by name and marketplace name, as Claude Code does.
    pub fn plugins(&self, claude: &UserPlugins) -> Result<CatalogPlugins> {
        let mut plugins = Vec::new();
        let mut unreadable = Vec::new();
        for mut summary in self.list()? {
            if summary.error.is_some() {
                unreadable.push(summary);
                continue;
            }
            match scan::preview(&summary.root) {
                Ok(preview) => {
                    plugins.extend(preview.plugins.into_iter().map(|plugin| CatalogPlugin {
                        marketplace_id: summary.id,
                        marketplace: preview.name.clone(),
                        status: claude.status(&plugin.name, &preview.name),
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

    /// Removes a marketplace and, for a GitHub source, its clone. A local directory is left as is.
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
        match scan::read_manifest(&summary.root) {
            Ok(manifest) => {
                summary.description = manifest.description().map(str::to_owned);
                summary.plugin_count = manifest
                    .plugin_entries()
                    .filter(|(_, entry)| entry.is_ok())
                    .count();
                summary.name = Some(manifest.name);
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

/// Clones `url` into `staging`, checks that it is a marketplace, and moves it to `dest`.
///
/// The caller holds the catalog entry for `dest`, so nothing else uses that directory. Nothing is
/// left behind on failure.
fn clone(url: &str, staging: &Path, dest: &Path) -> Result<()> {
    let result = (|| {
        // Leftovers of an add that was interrupted.
        remove_dir_if_exists(staging)?;
        remove_dir_if_exists(dest)?;
        for dir in [staging.parent(), dest.parent()].into_iter().flatten() {
            fs::create_dir_all(dir).map_err(|source| Error::io(dir, source))?;
        }
        git::shallow_clone(url, staging)?;
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
            let status = Command::new("git")
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

        clone(origin.to_str().unwrap(), &staging, &dest).unwrap();

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

        let error = clone(origin.to_str().unwrap(), &staging, &owner.join("tools")).unwrap_err();

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
