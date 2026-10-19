//! Discover: public marketplaces on GitHub, found without cloning them, that a person can add to
//! the catalog.
//!
//! Nothing here reaches the network until a search, or the curated list, is asked for. GitHub
//! gets the search query and the names of the repositories that it returned, and nothing else.
//! What it answers is kept in harness-hanger's database: the searches for an hour, and the
//! marketplaces, with their plugins, as a local index that searches also match.

mod cache;

use std::cell::OnceCell;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use self::cache::{Indexed, Search};
use super::manifest::{MarketplaceManifest, PluginEntry};
use crate::github::{GitHub, Partial, RepoInfo, RepoRef, in_parallel, successes};
pub use crate::github::{Problem, RateLimit, TOPIC};
use crate::time::{now, rfc3339};
use crate::{Result, paths, store};

/// Well-known marketplaces, shown before a search. A change ships with a release.
const CURATED: &[&str] = &[
    "anthropics/claude-plugins-official",
    "anthropics/claude-code",
    "anthropics/skills",
    "anthropics/knowledge-work-plugins",
    "wshobson/agents",
    "obra/superpowers-marketplace",
    "EveryInc/compound-engineering-plugin",
];
/// GitHub is asked again for the same search only after this long.
const SEARCH_TTL: i64 = 60 * 60;
/// The curated marketplaces' details are fetched again after this long.
const CURATED_TTL: i64 = 24 * 60 * 60;

/// How a search found its results on GitHub.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SearchMethod {
    /// GitHub's code search, which finds every repository with a marketplace manifest. It needs a
    /// GitHub token.
    Code,
    /// The repositories with the topic [`TOPIC`]. It works without a token, and finds fewer.
    Topic,
    /// No search: the curated list.
    Curated,
}

impl SearchMethod {
    fn as_str(self) -> &'static str {
        match self {
            Self::Code => "code",
            Self::Topic => "topic",
            Self::Curated => "curated",
        }
    }
}

/// When Discover may ask GitHub.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Fetch {
    /// Unless the same search ran within the hour, or the curated details are less than a day old.
    #[default]
    WhenStale,
    /// Even then.
    Refresh,
    /// Never: only what was found before.
    Offline,
}

/// A plugin as a marketplace manifest lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredPlugin {
    pub name: String,
    pub description: Option<String>,
    pub version: Option<String>,
    pub category: Option<String>,
    /// The entry's `keywords` and `tags`.
    #[serde(default)]
    pub keywords: Vec<String>,
}

impl DiscoveredPlugin {
    fn new(entry: PluginEntry) -> Self {
        Self {
            keywords: entry.links.keywords(),
            name: entry.name,
            description: entry.description,
            version: entry.version,
            category: entry.category,
        }
    }

    fn text(&self) -> String {
        let mut parts = vec![self.name.as_str()];
        parts.extend(self.description.as_deref());
        parts.extend(self.category.as_deref());
        parts.extend(self.keywords.iter().map(String::as_str));
        parts.join("\n").to_lowercase()
    }
}

/// What the index keeps about a marketplace.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    owner: String,
    repo: String,
    description: Option<String>,
    stars: u64,
    pushed_at: Option<String>,
    license: Option<String>,
    archived: bool,
    name: String,
    plugins: Vec<DiscoveredPlugin>,
}

impl Record {
    /// `None` when `manifest` is not a marketplace manifest.
    fn new(info: RepoInfo, manifest: &str) -> Option<Self> {
        let manifest: MarketplaceManifest = serde_json::from_str(manifest).ok()?;
        let description = info
            .description
            .or_else(|| manifest.description().map(str::to_owned))
            .map(|text| text.trim().to_owned())
            .filter(|text| !text.is_empty());
        let plugins = manifest
            .plugin_entries()
            .filter_map(|(_, entry)| entry.ok())
            .map(DiscoveredPlugin::new)
            .collect();
        Some(Self {
            owner: info.repo.owner,
            repo: info.repo.repo,
            description,
            stars: info.stars,
            pushed_at: info.pushed_at,
            license: info.license,
            archived: info.archived,
            name: manifest.name,
            plugins,
        })
    }

    fn key(&self) -> String {
        format!("{}/{}", self.owner, self.repo).to_ascii_lowercase()
    }

    /// Whether every term appears in the repository or in its plugins.
    fn matches(&self, terms: &[String]) -> bool {
        let mut parts = vec![
            format!("{}/{}", self.owner, self.repo),
            self.name.clone(),
            self.description.clone().unwrap_or_default(),
        ];
        parts.extend(self.plugins.iter().map(DiscoveredPlugin::text));
        let text = parts.join("\n").to_lowercase();
        terms.iter().all(|term| text.contains(term.as_str()))
    }
}

/// A public marketplace on GitHub.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredMarketplace {
    pub owner: String,
    pub repo: String,
    /// The repository on GitHub.
    pub url: String,
    /// The repository's description, else the manifest's.
    pub description: Option<String>,
    pub stars: u64,
    /// The last push, as an RFC 3339 timestamp.
    pub pushed_at: Option<String>,
    /// An SPDX ID, such as `MIT`, or the license's name.
    pub license: Option<String>,
    pub archived: bool,
    /// The marketplace's name in its manifest, which Claude Code uses in `plugin@marketplace`.
    pub name: String,
    pub plugin_count: usize,
    pub plugins: Vec<DiscoveredPlugin>,
    /// When GitHub was asked about it, as an RFC 3339 timestamp.
    pub fetched_at: String,
    /// Whether it is on the curated list.
    pub curated: bool,
    /// Its ID in the catalog, when the catalog has it.
    pub catalog_id: Option<i64>,
    /// The plugins whose name, description, category, or keywords hold every word of the search.
    pub matching_plugins: Vec<String>,
    /// Other repositories with the same manifest name and plugins, as `owner/repo`. The results
    /// leave them out.
    pub copies: Vec<String>,
}

/// What Discover found.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverResults {
    /// The search, or `None` for the curated list.
    pub query: Option<String>,
    /// How GitHub found the results.
    pub method: SearchMethod,
    /// Whether GitHub is asked with a token, which code search needs.
    pub signed_in: bool,
    /// The GitHub account that GitHub is asked as, when it is known without asking GitHub.
    pub account: Option<String>,
    /// The most starred first.
    pub results: Vec<DiscoveredMarketplace>,
    /// How many repositories GitHub matched. It returns the first 50, and `results` adds those
    /// that earlier searches found and that match.
    pub total_count: Option<u64>,
    /// When GitHub was searched, as an RFC 3339 timestamp. `None` when it has not been.
    pub searched_at: Option<String>,
    /// Whether the GitHub results come from an earlier search instead of a new request.
    pub from_cache: bool,
    /// Why GitHub could not be asked. The results then hold only what was found before.
    pub problem: Option<Problem>,
    /// The rate limit that GitHub reported for the last request, when one was made.
    pub rate_limit: Option<RateLimit>,
}

/// Finds public marketplaces on GitHub.
pub struct Discover {
    connection: Connection,
    /// Made on first use, because finding the token can run `gh`.
    github: OnceCell<GitHub>,
    /// The current time in Unix seconds.
    clock: Box<dyn Fn() -> i64 + Send>,
}

impl Discover {
    /// Opens the index kept in `data_dir`. Nothing is sent to GitHub yet.
    pub fn open(data_dir: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            connection: store::open(data_dir.as_ref())?,
            github: OnceCell::new(),
            clock: Box::new(now),
        })
    }

    /// Opens the index in the default [data directory](paths::data_dir).
    pub fn open_default() -> Result<Self> {
        Self::open(paths::data_dir()?)
    }

    /// The curated marketplaces. Their details are fetched once a day.
    pub fn curated(&self, fetch: Fetch) -> Result<DiscoverResults> {
        let github = self.github();
        let now = (self.clock)();
        let curated: Vec<RepoRef> = CURATED
            .iter()
            .filter_map(|name| RepoRef::parse(name))
            .collect();
        // The last fetch of the curated details, kept as a search with no query. A repository
        // that GitHub does not describe, such as a deleted one, is not asked for again until the
        // next day, and a list that a release changed is fetched at once.
        let marker = Search {
            method: SearchMethod::Curated,
            repos: curated.iter().map(RepoRef::key).collect(),
            total_count: None,
            searched_at: now,
        };
        let ask = match fetch {
            Fetch::Offline => false,
            Fetch::Refresh => true,
            Fetch::WhenStale => cache::search(&self.connection, SearchMethod::Curated, "")?
                .is_none_or(|last| {
                    last.repos != marker.repos || now - last.searched_at >= CURATED_TTL
                }),
        };
        let mut problem = None;
        let mut fetched = false;
        if ask {
            match fetch_details(github, &curated) {
                Ok((records, partial)) => {
                    let complete = partial.is_none().then_some(("", &marker));
                    cache::save(&self.connection, &records, now, complete)?;
                    problem = partial;
                    fetched = true;
                }
                Err(error) => problem = Some(error),
            }
        }
        let index = cache::repos(&self.connection)?;
        let found = curated.iter().filter_map(|repo| index.get(&repo.key()));
        Ok(DiscoverResults {
            query: None,
            method: SearchMethod::Curated,
            signed_in: github.signed_in(),
            account: github.login().map(str::to_owned),
            results: self.results(found, &[])?,
            total_count: None,
            searched_at: None,
            from_cache: !fetched,
            problem,
            rate_limit: github.rate_limit(if github.signed_in() {
                "graphql"
            } else {
                "core"
            }),
        })
    }

    /// Searches GitHub for marketplaces whose manifest matches `query`, and the index for those
    /// whose repository or plugins hold every word of it. An empty query gives the curated list.
    pub fn search(&self, query: &str, fetch: Fetch) -> Result<DiscoverResults> {
        let query = query.split_whitespace().collect::<Vec<_>>().join(" ");
        if query.is_empty() {
            return self.curated(fetch);
        }
        let key = query.to_lowercase();
        let github = self.github();
        let now = (self.clock)();
        let method = if github.signed_in() {
            SearchMethod::Code
        } else {
            SearchMethod::Topic
        };

        let mut found = cache::search(&self.connection, method, &key)?;
        let ask = match fetch {
            Fetch::Offline => false,
            Fetch::Refresh => true,
            Fetch::WhenStale => found
                .as_ref()
                .is_none_or(|earlier| now - earlier.searched_at >= SEARCH_TTL),
        };
        let mut problem = None;
        let mut asked = false;
        if ask {
            match search_github(github, method, &query) {
                Ok(((records, partial), total_count)) => {
                    let search = Search {
                        method,
                        repos: records.iter().map(Record::key).collect(),
                        total_count: Some(total_count),
                        searched_at: now,
                    };
                    // An incomplete search is shown but not kept, so that the next one asks again.
                    let complete = partial.is_none().then_some((key.as_str(), &search));
                    cache::save(&self.connection, &records, now, complete)?;
                    found = Some(search);
                    problem = partial;
                    asked = true;
                }
                Err(error) => problem = Some(error),
            }
        }
        if found.is_none() {
            // The same search, made earlier with or without a token.
            let other = match method {
                SearchMethod::Code => SearchMethod::Topic,
                _ => SearchMethod::Code,
            };
            found = cache::search(&self.connection, other, &key)?;
        }

        let index = cache::repos(&self.connection)?;
        let terms: Vec<String> = key.split(' ').map(str::to_owned).collect();
        let hits: Vec<&String> = found.iter().flat_map(|search| &search.repos).collect();
        let hit_keys: HashSet<&String> = hits.iter().copied().collect();
        // What GitHub found, which its search may have matched in ways the index does not, and
        // what earlier searches found that matches.
        let matching = hits.iter().filter_map(|key| index.get(*key)).chain(
            index
                .iter()
                .filter(|(key, found)| !hit_keys.contains(key) && found.record.matches(&terms))
                .map(|(_, found)| found),
        );
        let resource = match method {
            SearchMethod::Code => "code_search",
            _ => "search",
        };
        Ok(DiscoverResults {
            query: Some(query),
            method: found.as_ref().map_or(method, |search| search.method),
            signed_in: github.signed_in(),
            account: github.login().map(str::to_owned),
            results: self.results(matching, &terms)?,
            total_count: found.as_ref().and_then(|search| search.total_count),
            searched_at: found.as_ref().map(|search| rfc3339(search.searched_at)),
            from_cache: found.is_some() && !asked,
            problem,
            rate_limit: github.rate_limit(resource),
        })
    }

    fn github(&self) -> &GitHub {
        self.github.get_or_init(GitHub::connect)
    }

    /// The results for `found`: with their place in the catalog and on the curated list, copies
    /// left out, and the most starred first.
    fn results<'a>(
        &self,
        found: impl Iterator<Item = &'a Indexed>,
        terms: &[String],
    ) -> Result<Vec<DiscoveredMarketplace>> {
        let catalog = cache::catalog_ids(&self.connection)?;
        let curated: HashSet<String> = CURATED
            .iter()
            .map(|name| name.to_ascii_lowercase())
            .collect();
        let mut results: Vec<DiscoveredMarketplace> = found
            .map(|found| {
                let record = &found.record;
                let key = record.key();
                let matching_plugins = if terms.is_empty() {
                    Vec::new()
                } else {
                    record
                        .plugins
                        .iter()
                        .filter(|plugin| {
                            let text = plugin.text();
                            terms.iter().all(|term| text.contains(term.as_str()))
                        })
                        .map(|plugin| plugin.name.clone())
                        .collect()
                };
                DiscoveredMarketplace {
                    url: format!("https://github.com/{}/{}", record.owner, record.repo),
                    owner: record.owner.clone(),
                    repo: record.repo.clone(),
                    description: record.description.clone(),
                    stars: record.stars,
                    pushed_at: record.pushed_at.clone(),
                    license: record.license.clone(),
                    archived: record.archived,
                    name: record.name.clone(),
                    plugin_count: record.plugins.len(),
                    plugins: record.plugins.clone(),
                    fetched_at: rfc3339(found.fetched_at),
                    curated: curated.contains(&key),
                    catalog_id: catalog.get(&key).copied(),
                    matching_plugins,
                    copies: Vec::new(),
                }
            })
            .collect();
        results = without_copies(results);
        results.sort_by(|a, b| b.stars.cmp(&a.stars).then_with(|| by_name(a, b)));
        Ok(results)
    }
}

/// Searches GitHub, reads the manifest of each repository it found, and says how many matched in
/// all. Repositories without a manifest that parses are left out.
fn search_github(
    github: &GitHub,
    method: SearchMethod,
    query: &str,
) -> Result<(Partial<Record>, u64), Problem> {
    if method == SearchMethod::Code {
        let (repos, total_count) = github.search_code(query)?;
        let (fetched, problem) = github.repos(&repos)?;
        return Ok(((records(fetched), problem), total_count));
    }
    let (repos, total_count) = github.search_topic(query)?;
    let (fetched, problem) = successes(in_parallel(&repos, |info| {
        github
            .manifest(&info.repo)
            .map(|manifest| (info.clone(), manifest))
    }))?;
    Ok(((records(fetched), problem), total_count))
}

/// The details and manifest of each repository: through GraphQL with a token, else through the
/// REST API and the raw file of each one.
fn fetch_details(github: &GitHub, repos: &[RepoRef]) -> Result<Partial<Record>, Problem> {
    if github.signed_in() {
        let (fetched, problem) = github.repos(repos)?;
        return Ok((records(fetched), problem));
    }
    let (fetched, problem) = successes(in_parallel(repos, |repo| -> Result<_, Problem> {
        let Some(info) = github.repo(repo)? else {
            return Ok(None);
        };
        Ok(Some((info, github.manifest(repo)?)))
    }))?;
    Ok((records(fetched.into_iter().flatten().collect()), problem))
}

/// The marketplaces among `fetched`: repositories that are not forks, with a manifest that
/// parses.
fn records(fetched: Vec<(RepoInfo, Option<String>)>) -> Vec<Record> {
    fetched
        .into_iter()
        .filter(|(info, _)| !info.fork)
        .filter_map(|(info, manifest)| Record::new(info, &manifest?))
        .collect()
}

/// Keeps one of each set of marketplaces with the same name and plugins, such as copies of a
/// well-known one: the curated one, else the one in the catalog, else the most starred, else the
/// first by name.
fn without_copies(mut results: Vec<DiscoveredMarketplace>) -> Vec<DiscoveredMarketplace> {
    results.sort_by(|a, b| {
        (b.curated, b.catalog_id.is_some(), b.stars)
            .cmp(&(a.curated, a.catalog_id.is_some(), a.stars))
            .then_with(|| by_name(a, b))
    });
    let mut kept: Vec<DiscoveredMarketplace> = Vec::new();
    let mut originals: HashMap<(String, Vec<String>), usize> = HashMap::new();
    for found in results {
        if found.plugins.is_empty() {
            kept.push(found);
            continue;
        }
        let mut plugins: Vec<String> = found.plugins.iter().map(|p| p.name.clone()).collect();
        plugins.sort();
        let fingerprint = (found.name.clone(), plugins);
        if let Some(&original) = originals.get(&fingerprint) {
            let copy = format!("{}/{}", found.owner, found.repo);
            kept[original].copies.push(copy);
        } else {
            originals.insert(fingerprint, kept.len());
            kept.push(found);
        }
    }
    kept
}

/// Orders by `owner/repo`, ignoring case.
fn by_name(a: &DiscoveredMarketplace, b: &DiscoveredMarketplace) -> std::cmp::Ordering {
    let name =
        |found: &DiscoveredMarketplace| format!("{}/{}", found.owner, found.repo).to_lowercase();
    name(a).cmp(&name(b))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI64, Ordering};

    use serde_json::json;

    use super::*;
    use crate::github::fake::Fake;

    const START: i64 = 1_792_152_351;
    const CODE_SEARCH: &str = "https://api.github.com/search/code";
    const TOPIC_SEARCH: &str = "https://api.github.com/search/repositories";
    const GRAPHQL: &str = "https://api.github.com/graphql";

    fn acme_manifest() -> String {
        json!({ "name": "acme-tools", "description": "Tools by Acme", "plugins": [
            { "name": "reviewer", "description": "Reviews code", "source": "./reviewer", "keywords": ["review"] },
            { "name": "formatter", "description": "Formats Python files", "source": "./formatter", "category": "style" },
            { "description": "Has no name, so it is not a plugin" }
        ] })
        .to_string()
    }

    fn beta_manifest() -> String {
        json!({ "name": "beta", "plugins": [
            { "name": "deploy", "description": "Deploys apps", "source": { "source": "github", "repo": "beta/deploy" } }
        ] })
        .to_string()
    }

    fn graph_repo(full_name: &str, stars: u64, manifest: Option<&str>) -> Value {
        json!({
            "nameWithOwner": full_name, "description": null, "stargazerCount": stars,
            "pushedAt": "2026-10-01T00:00:00Z", "isArchived": false, "isFork": false,
            "licenseInfo": { "spdxId": "MIT", "name": "MIT License" },
            "manifest": manifest.map(|text| json!({ "text": text, "isTruncated": false, "byteSize": text.len() }))
        })
    }

    fn code_hits(repos: &[&str]) -> String {
        let items: Vec<_> = repos
            .iter()
            .map(|name| json!({ "path": ".claude-plugin/marketplace.json", "repository": { "full_name": name } }))
            .collect();
        json!({ "total_count": 120, "items": items }).to_string()
    }

    fn graph_answer(repos: &[Value]) -> String {
        let data: Map<String, Value> = repos
            .iter()
            .enumerate()
            .map(|(i, repo)| (format!("r{i}"), repo.clone()))
            .collect();
        json!({ "data": data }).to_string()
    }

    use serde_json::{Map, Value};

    struct Setup {
        _data: tempfile::TempDir,
        discover: Discover,
        fake: Fake,
        clock: Arc<AtomicI64>,
    }

    impl Setup {
        fn new(token: Option<&str>) -> Self {
            let data = tempfile::tempdir().unwrap();
            let fake = Fake::default();
            let clock = Arc::new(AtomicI64::new(START));
            let mut discover = Discover::open(data.path()).unwrap();
            let _ = discover.github.set(fake.github(token));
            let time = Arc::clone(&clock);
            discover.clock = Box::new(move || time.load(Ordering::Relaxed));
            Self {
                _data: data,
                discover,
                fake,
                clock,
            }
        }

        fn later(&self, seconds: i64) {
            self.clock.fetch_add(seconds, Ordering::Relaxed);
        }

        /// A token, and GitHub finding Acme's and Beta's marketplaces.
        fn signed_in() -> Self {
            let setup = Self::new(Some("secret"));
            setup
                .fake
                .on(CODE_SEARCH, 200, &code_hits(&["Acme/Tools", "beta/market"]))
                .on(
                    GRAPHQL,
                    200,
                    &graph_answer(&[
                        graph_repo("Acme/Tools", 10, Some(&acme_manifest())),
                        graph_repo("beta/market", 99, Some(&beta_manifest())),
                    ]),
                );
            setup
        }
    }

    fn names(results: &DiscoverResults) -> Vec<String> {
        results
            .results
            .iter()
            .map(|found| format!("{}/{}", found.owner, found.repo))
            .collect()
    }

    #[test]
    fn searches_every_manifest_with_a_token() {
        let setup = Setup::signed_in();

        let results = setup
            .discover
            .search("  Code   review ", Fetch::WhenStale)
            .unwrap();

        assert_eq!(results.query.as_deref(), Some("Code review"));
        assert_eq!(results.method, SearchMethod::Code);
        assert!(results.signed_in && !results.from_cache);
        assert_eq!(results.total_count, Some(120));
        assert_eq!(results.searched_at.as_deref(), Some("2026-10-16T12:05:51Z"));
        // The most starred first.
        assert_eq!(names(&results), ["beta/market", "Acme/Tools"]);
        let acme = &results.results[1];
        assert_eq!(acme.url, "https://github.com/Acme/Tools");
        assert_eq!(acme.name, "acme-tools");
        assert_eq!(acme.description.as_deref(), Some("Tools by Acme"));
        assert_eq!(acme.plugin_count, 2);
        assert_eq!(acme.license.as_deref(), Some("MIT"));
        assert_eq!(acme.matching_plugins, ["reviewer"]);
        assert!(!acme.curated && acme.catalog_id.is_none());
        // The query is what GitHub gets, with the qualifiers that find manifests.
        assert!(
            setup.fake.sent()[0]
                .url
                .contains("q=Code%20review%20filename")
        );

        let json = serde_json::to_value(&results).unwrap();
        assert_eq!(json["results"][1]["pluginCount"], 2);
        assert_eq!(json["results"][1]["matchingPlugins"], json!(["reviewer"]));
    }

    #[test]
    fn searches_by_topic_without_a_token() {
        let setup = Setup::new(None);
        setup.fake.on(
            TOPIC_SEARCH,
            200,
            &json!({ "total_count": 2, "items": [
                { "full_name": "Acme/Tools", "description": "Acme's tools", "stargazers_count": 5,
                  "license": { "spdx_id": "Apache-2.0", "name": "Apache License 2.0" } },
                { "full_name": "acme/plugin-only", "stargazers_count": 50 }
            ] })
            .to_string(),
        );
        setup.fake.on(
            "https://raw.githubusercontent.com/Acme/Tools/",
            200,
            &acme_manifest(),
        );

        let results = setup.discover.search("format", Fetch::WhenStale).unwrap();

        assert_eq!(results.method, SearchMethod::Topic);
        assert!(!results.signed_in);
        // A repository with the topic but without a manifest is not a marketplace.
        assert_eq!(names(&results), ["Acme/Tools"]);
        assert_eq!(
            results.results[0].description.as_deref(),
            Some("Acme's tools")
        );
        assert_eq!(results.results[0].license.as_deref(), Some("Apache-2.0"));
        assert_eq!(results.results[0].matching_plugins, ["formatter"]);
        let sent = setup.fake.sent();
        assert!(
            sent[0]
                .url
                .contains("q=format%20topic%3Aclaude-code-marketplace")
        );
        assert!(sent.iter().all(|sent| !sent.authorized));
    }

    #[test]
    fn repeats_a_search_from_the_cache_within_the_hour() {
        let setup = Setup::signed_in();
        setup.discover.search("review", Fetch::WhenStale).unwrap();
        let asked = setup.fake.sent().len();

        let again = setup.discover.search("REVIEW", Fetch::WhenStale).unwrap();
        assert_eq!(setup.fake.sent().len(), asked);
        assert!(again.from_cache);
        assert_eq!(names(&again), ["beta/market", "Acme/Tools"]);

        setup.discover.search("review", Fetch::Refresh).unwrap();
        assert_eq!(setup.fake.sent().len(), asked * 2);

        setup.later(SEARCH_TTL);
        let stale = setup.discover.search("review", Fetch::WhenStale).unwrap();
        assert_eq!(setup.fake.sent().len(), asked * 3);
        assert!(!stale.from_cache);
    }

    #[test]
    fn sends_nothing_offline_and_finds_earlier_marketplaces_by_their_plugins() {
        let setup = Setup::signed_in();
        setup.discover.search("anything", Fetch::WhenStale).unwrap();
        let asked = setup.fake.sent().len();

        let results = setup
            .discover
            .search("python formats", Fetch::Offline)
            .unwrap();

        assert_eq!(setup.fake.sent().len(), asked);
        assert_eq!(names(&results), ["Acme/Tools"]);
        assert_eq!(results.results[0].matching_plugins, ["formatter"]);
        assert_eq!(results.searched_at, None);
        assert!(!results.from_cache);
    }

    #[test]
    fn adds_earlier_marketplaces_that_match_to_what_github_found() {
        let setup = Setup::signed_in();
        setup.discover.search("anything", Fetch::WhenStale).unwrap();
        setup
            .fake
            .on(CODE_SEARCH, 200, &code_hits(&["Acme/Tools"]))
            .on(
                GRAPHQL,
                200,
                &graph_answer(&[graph_repo("Acme/Tools", 10, Some(&acme_manifest()))]),
            );

        let results = setup.discover.search("deploy", Fetch::WhenStale).unwrap();

        // GitHub's own match stays, though its plugins do not say "deploy".
        assert_eq!(names(&results), ["beta/market", "Acme/Tools"]);
        assert_eq!(results.results[0].matching_plugins, ["deploy"]);
    }

    #[test]
    fn keeps_earlier_results_when_github_cannot_be_reached() {
        let setup = Setup::signed_in();
        setup.discover.search("review", Fetch::WhenStale).unwrap();
        setup.later(2 * SEARCH_TTL);
        setup.fake.go_offline();

        let results = setup.discover.search("review", Fetch::WhenStale).unwrap();

        assert!(matches!(results.problem, Some(Problem::Offline { .. })));
        assert!(results.from_cache);
        assert_eq!(names(&results), ["beta/market", "Acme/Tools"]);
    }

    #[test]
    fn reports_a_rate_limit_with_its_reset_time() {
        let setup = Setup::new(Some("secret"));
        setup.fake.on_with(
            CODE_SEARCH,
            403,
            &[
                ("x-ratelimit-resource", "code_search"),
                ("x-ratelimit-limit", "10"),
                ("x-ratelimit-remaining", "0"),
                ("x-ratelimit-reset", "1792152400"),
            ],
            r#"{ "message": "API rate limit exceeded" }"#,
        );

        let results = setup.discover.search("review", Fetch::WhenStale).unwrap();

        assert_eq!(
            results.problem,
            Some(Problem::RateLimited {
                reset_at: Some("2026-10-16T12:06:40Z".to_owned())
            })
        );
        assert_eq!(results.rate_limit.unwrap().remaining, 0);
        assert!(results.results.is_empty());
    }

    #[test]
    fn marks_marketplaces_in_the_catalog() {
        let setup = Setup::signed_in();
        setup
            .discover
            .connection
            .execute(
                "INSERT INTO marketplace (kind, location) VALUES ('github', 'acme/tools')",
                [],
            )
            .unwrap();

        let results = setup.discover.search("review", Fetch::WhenStale).unwrap();

        let acme = results
            .results
            .iter()
            .find(|found| found.repo == "Tools")
            .unwrap();
        assert!(acme.catalog_id.is_some());
    }

    #[test]
    fn leaves_out_copies_and_forks() {
        let setup = Setup::new(Some("secret"));
        let mut fork = graph_repo("acme/fork", 500, Some(&acme_manifest()));
        fork["isFork"] = json!(true);
        setup
            .fake
            .on(
                CODE_SEARCH,
                200,
                &code_hits(&["Acme/Tools", "copier/tools", "acme/fork"]),
            )
            .on(
                GRAPHQL,
                200,
                &graph_answer(&[
                    graph_repo("Acme/Tools", 10, Some(&acme_manifest())),
                    graph_repo("copier/tools", 2, Some(&acme_manifest())),
                    fork,
                ]),
            );

        let results = setup.discover.search("review", Fetch::WhenStale).unwrap();

        assert_eq!(names(&results), ["Acme/Tools"]);
        assert_eq!(results.results[0].copies, ["copier/tools"]);
    }

    #[test]
    fn shows_the_curated_marketplaces_for_an_empty_search() {
        let setup = Setup::new(Some("secret"));
        setup.fake.on(
            GRAPHQL,
            200,
            &graph_answer(&[graph_repo(
                "anthropics/claude-plugins-official",
                40_000,
                Some(&acme_manifest()),
            )]),
        );

        let results = setup.discover.search("   ", Fetch::WhenStale).unwrap();

        assert_eq!(results.method, SearchMethod::Curated);
        assert_eq!(results.query, None);
        assert_eq!(names(&results), ["anthropics/claude-plugins-official"]);
        assert!(results.results[0].curated);
        // Every curated repository goes in one request.
        assert_eq!(setup.fake.sent().len(), 1);

        // Those that GitHub did not describe, such as a deleted one, wait for the next day too.
        let again = setup.discover.curated(Fetch::WhenStale).unwrap();
        assert!(again.from_cache);
        assert_eq!(setup.fake.sent().len(), 1);
    }

    #[test]
    fn shows_an_incomplete_search_without_keeping_it() {
        let setup = Setup::new(None);
        setup.fake.on(
            TOPIC_SEARCH,
            200,
            &json!({ "total_count": 2, "items": [
                { "full_name": "Acme/Tools", "stargazers_count": 5 },
                { "full_name": "beta/market", "stargazers_count": 9 }
            ] })
            .to_string(),
        );
        setup
            .fake
            .on(
                "https://raw.githubusercontent.com/Acme/Tools/",
                200,
                &acme_manifest(),
            )
            .on("https://raw.githubusercontent.com/beta/market/", 500, "");

        let results = setup.discover.search("review", Fetch::WhenStale).unwrap();

        assert_eq!(names(&results), ["Acme/Tools"]);
        assert!(matches!(
            results.problem,
            Some(Problem::GitHub { status: 500, .. })
        ));
        let asked = setup.fake.sent().len();
        setup.discover.search("review", Fetch::WhenStale).unwrap();
        assert!(setup.fake.sent().len() > asked);
    }

    #[test]
    fn keeps_the_same_copy_every_time() {
        let setup = Setup::new(Some("secret"));
        let copies = ["zeta/tools", "Acme/Tools", "mid/tools"];
        setup.fake.on(CODE_SEARCH, 200, &code_hits(&copies)).on(
            GRAPHQL,
            200,
            &graph_answer(&copies.map(|name| graph_repo(name, 0, Some(&acme_manifest())))),
        );

        let results = setup.discover.search("review", Fetch::WhenStale).unwrap();

        assert_eq!(names(&results), ["Acme/Tools"]);
        assert_eq!(results.results[0].copies, ["mid/tools", "zeta/tools"]);
    }

    #[test]
    fn keeps_the_curated_details_for_a_day() {
        let setup = Setup::new(Some("secret"));
        let curated: Vec<_> = CURATED
            .iter()
            .map(|name| {
                let manifest = json!({ "name": name, "plugins": [] }).to_string();
                graph_repo(name, 1, Some(&manifest))
            })
            .collect();
        setup.fake.on(GRAPHQL, 200, &graph_answer(&curated));

        let first = setup.discover.curated(Fetch::WhenStale).unwrap();
        let again = setup.discover.curated(Fetch::WhenStale).unwrap();

        assert_eq!(first.results.len(), CURATED.len());
        assert!(first.results.iter().all(|found| found.curated));
        assert!(again.from_cache);
        assert_eq!(setup.fake.sent().len(), 1);
        setup.later(CURATED_TTL);
        setup.discover.curated(Fetch::WhenStale).unwrap();
        assert_eq!(setup.fake.sent().len(), 2);
    }
}
