//! The GitHub API, which Discover uses to find marketplaces and to read their manifests without
//! cloning them.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::Account;
use crate::VERSION;
use crate::marketplace::{encode_segment, parse_github_repo};
use crate::time::{now, rfc3339};

/// Where a marketplace keeps its manifest.
const MANIFEST_PATH: &str = ".claude-plugin/marketplace.json";
/// The topic that marketplaces are searched by without a token.
pub const TOPIC: &str = "claude-code-marketplace";
/// Repositories asked for in one search.
const PAGE: usize = 50;
/// Repositories asked for in one GraphQL request. GitHub answers one with many repositories
/// slowly, so several small requests go out at once.
const GRAPHQL_CHUNK: usize = 10;
const API: &str = "https://api.github.com";
const RAW: &str = "https://raw.githubusercontent.com";
const WEB: &str = "https://github.com";
/// The most of an API answer that is read. A GraphQL answer holds up to [`GRAPHQL_CHUNK`]
/// manifests.
const MAX_API_BYTES: u64 = 64 << 20;
/// Larger manifests are not read. The official marketplace's is about 200 KB.
const MAX_MANIFEST_BYTES: u64 = 4 << 20;
/// Requests sent at once when each repository needs its own.
const WORKERS: usize = 8;

/// A repository on GitHub, with GitHub's capitalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepoRef {
    pub owner: String,
    pub repo: String,
}

impl RepoRef {
    /// Parses `owner/repo`, or `None` when it is not a name that the catalog accepts.
    pub fn parse(full_name: &str) -> Option<Self> {
        parse_github_repo(full_name)?;
        let (owner, repo) = full_name.split_once('/')?;
        Some(Self {
            owner: owner.to_owned(),
            repo: repo.to_owned(),
        })
    }

    /// Identifies the repository: GitHub ignores case in names.
    pub fn key(&self) -> String {
        format!("{}/{}", self.owner, self.repo).to_ascii_lowercase()
    }
}

/// What GitHub says about a repository.
#[derive(Debug, Clone)]
pub(crate) struct RepoInfo {
    pub repo: RepoRef,
    pub description: Option<String>,
    pub stars: u64,
    pub pushed_at: Option<String>,
    pub license: Option<String>,
    pub archived: bool,
    pub fork: bool,
}

/// Why GitHub could not answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Problem {
    /// GitHub cannot be reached, for example without a network.
    Offline { message: String },
    /// A GitHub rate limit is reached. `reset_at` is when it resets, as an RFC 3339 timestamp.
    RateLimited { reset_at: Option<String> },
    /// GitHub refused the token.
    Unauthorized { message: String },
    /// Any other error that GitHub reported.
    #[serde(rename = "github")]
    GitHub { status: u16, message: String },
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Offline { message } => write!(f, "cannot reach GitHub: {message}"),
            Self::RateLimited {
                reset_at: Some(reset_at),
            } => write!(f, "GitHub's rate limit is reached; it resets at {reset_at}"),
            Self::RateLimited { reset_at: None } => {
                write!(f, "GitHub's rate limit is reached; try again later")
            }
            Self::Unauthorized { message } => write!(
                f,
                "GitHub refused the token ({message}); sign in again in Settings or with `harness-hanger github login`, or check GH_TOKEN and GITHUB_TOKEN"
            ),
            Self::GitHub { status, message } => write!(f, "GitHub answered {status}: {message}"),
        }
    }
}

/// A rate limit as GitHub last reported it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimit {
    /// Which limit: `code_search`, `search`, `graphql`, or `core`.
    pub resource: String,
    pub limit: u64,
    pub remaining: u64,
    /// When it resets, as an RFC 3339 timestamp.
    pub reset_at: String,
}

pub(crate) struct Request {
    pub url: String,
    pub headers: Vec<(&'static str, String)>,
    /// Sent as JSON with a POST when set.
    pub body: Option<String>,
    /// The most bytes of the answer to read.
    pub limit: u64,
}

pub(crate) struct Response {
    pub status: u16,
    /// Names in lowercase.
    pub headers: HashMap<String, String>,
    pub body: String,
}

/// Why a request got no answer to read.
pub(crate) enum Failure {
    /// The server cannot be reached, for example without a network.
    Unreachable(String),
    /// The server answered, but the answer cannot be read, for example because it is too large.
    Unreadable { status: u16, message: String },
}

/// Sends requests. Tests replace it with canned answers.
pub(crate) trait Transport: Send + Sync {
    fn send(&self, request: &Request) -> Result<Response, Failure>;
}

/// A code that the person enters on github.com to let harness-hanger sign in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCode {
    /// What harness-hanger asks about the sign-in with. It is not shown.
    #[serde(alias = "device_code")]
    pub device_code: String,
    /// What the person enters, such as `ABCD-1234`.
    #[serde(alias = "user_code")]
    pub user_code: String,
    /// Where the person enters it.
    #[serde(alias = "verification_uri")]
    pub verification_uri: String,
    /// Seconds until the code expires.
    #[serde(alias = "expires_in")]
    pub expires_in: u64,
    /// Seconds to wait between two questions about the sign-in.
    pub interval: u64,
}

/// Where a sign-in with a [`DeviceCode`] is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DeviceAnswer {
    Token(String),
    /// The person has not entered the code yet.
    Pending,
    /// Asked too often: wait `interval` seconds from now on.
    SlowDown {
        interval: u64,
    },
    Expired,
    /// The person refused.
    Denied,
}

/// What a request for many things gives: those that came, and why some did not.
pub(crate) type Partial<T> = (Vec<T>, Option<Problem>);

struct Ureq(ureq::Agent);

impl Transport for Ureq {
    fn send(&self, request: &Request) -> Result<Response, Failure> {
        let sent = if let Some(body) = &request.body {
            let mut builder = self.0.post(&request.url);
            for (name, value) in &request.headers {
                builder = builder.header(*name, value);
            }
            builder
                .header("Content-Type", "application/json")
                .send(body.as_str())
        } else {
            let mut builder = self.0.get(&request.url);
            for (name, value) in &request.headers {
                builder = builder.header(*name, value);
            }
            builder.call()
        };
        let mut response = sent.map_err(|error| Failure::Unreachable(error.to_string()))?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                Some((name.as_str().to_owned(), value.to_str().ok()?.to_owned()))
            })
            .collect();
        let body = response
            .body_mut()
            .with_config()
            .limit(request.limit)
            .read_to_string()
            .map_err(|error| Failure::Unreadable {
                status,
                message: error.to_string(),
            })?;
        Ok(Response {
            status,
            headers,
            body,
        })
    }
}

/// The GitHub API, as the person who runs harness-hanger, or anonymously.
pub(crate) struct GitHub {
    transport: Box<dyn Transport>,
    token: Option<String>,
    /// The account that the token belongs to, when the [account](Account) knows it.
    login: Option<String>,
    /// The rate limits that API answers reported, by resource.
    rate_limits: Mutex<HashMap<String, RateLimit>>,
}

impl GitHub {
    /// The GitHub API over the network, with the token of the person's [account](Account), if
    /// any.
    pub fn connect() -> Self {
        Account::system().github()
    }

    /// The GitHub API over the network, with `token`.
    pub fn over_network(token: Option<String>) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .http_status_as_error(false)
            .user_agent(format!("harness-hanger/{VERSION}"))
            .build()
            .into();
        Self::new(Box::new(Ureq(agent)), token)
    }

    pub fn new(transport: Box<dyn Transport>, token: Option<String>) -> Self {
        Self {
            transport,
            token,
            login: None,
            rate_limits: Mutex::new(HashMap::new()),
        }
    }

    /// Whether requests carry a token. GitHub's code search needs one.
    pub fn signed_in(&self) -> bool {
        self.token.is_some()
    }

    /// The account that the token belongs to, when it is known without asking GitHub.
    pub fn login(&self) -> Option<&str> {
        self.login.as_deref()
    }

    #[must_use]
    pub fn with_login(mut self, login: Option<String>) -> Self {
        self.login = login;
        self
    }

    /// The rate limit of `resource`, such as `code_search`, as the last answer that counted
    /// against it reported.
    pub fn rate_limit(&self, resource: &str) -> Option<RateLimit> {
        self.rate_limits
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(resource)
            .cloned()
    }

    /// The repositories whose marketplace manifest matches `query`, in GitHub's order, and how
    /// many matched in all. GitHub's code search needs a token.
    pub fn search_code(&self, query: &str) -> Result<(Vec<RepoRef>, u64), Problem> {
        #[derive(Deserialize)]
        struct Page {
            total_count: u64,
            items: Vec<Item>,
        }
        #[derive(Deserialize)]
        struct Item {
            path: String,
            repository: ItemRepo,
        }
        #[derive(Deserialize)]
        struct ItemRepo {
            full_name: String,
            #[serde(default)]
            fork: bool,
        }

        let q = format!("{query} filename:marketplace.json path:.claude-plugin");
        let url = format!("{API}/search/code?q={}&per_page={PAGE}", encode_segment(&q));
        let page: Page = parse(&self.api(url, None)?)?;
        let mut seen = HashSet::new();
        let repos = page
            .items
            .into_iter()
            // `path:` also matches a manifest in a subdirectory, which is not a marketplace's.
            .filter(|item| item.path == MANIFEST_PATH && !item.repository.fork)
            .filter_map(|item| RepoRef::parse(&item.repository.full_name))
            .filter(|repo| seen.insert(repo.key()))
            .collect();
        Ok((repos, page.total_count))
    }

    /// The repositories with the topic [`TOPIC`] that match `query`, and how many matched in all.
    /// Works without a token.
    pub fn search_topic(&self, query: &str) -> Result<(Vec<RepoInfo>, u64), Problem> {
        #[derive(Deserialize)]
        struct Page {
            total_count: u64,
            items: Vec<RestRepo>,
        }

        let q = format!("{query} topic:{TOPIC}");
        let url = format!(
            "{API}/search/repositories?q={}&per_page={PAGE}",
            encode_segment(&q)
        );
        let page: Page = parse(&self.api(url, None)?)?;
        let repos = page
            .items
            .into_iter()
            .filter_map(RestRepo::into_info)
            .collect();
        Ok((repos, page.total_count))
    }

    /// Each repository's details and manifest, through GraphQL, which needs a token. A
    /// repository that does not exist is left out; one without a manifest has `None`.
    pub fn repos(&self, repos: &[RepoRef]) -> Result<Partial<(RepoInfo, Option<String>)>, Problem> {
        let chunks: Vec<_> = repos.chunks(GRAPHQL_CHUNK).collect();
        let (answers, mut problem) =
            successes(in_parallel(&chunks, |chunk| self.graphql_repos(chunk)))?;
        let mut found = Vec::new();
        for (repos, chunk_problem) in answers {
            found.extend(repos);
            problem = problem.or(chunk_problem);
        }
        Ok((found, problem))
    }

    fn graphql_repos(
        &self,
        repos: &[RepoRef],
    ) -> Result<Partial<(RepoInfo, Option<String>)>, Problem> {
        #[derive(Deserialize)]
        struct Answer {
            data: Option<Map<String, Value>>,
            #[serde(default)]
            errors: Vec<GraphError>,
        }
        #[derive(Deserialize)]
        struct GraphError {
            #[serde(rename = "type")]
            kind: Option<String>,
            message: String,
        }

        let mut variables = Map::new();
        for (index, repo) in repos.iter().enumerate() {
            variables.insert(format!("o{index}"), repo.owner.clone().into());
            variables.insert(format!("n{index}"), repo.repo.clone().into());
        }
        let body =
            serde_json::json!({ "query": graphql_query(repos.len()), "variables": variables });
        let answer: Answer = parse(&self.api(format!("{API}/graphql"), Some(body.to_string()))?)?;
        if answer
            .errors
            .iter()
            .any(|error| error.kind.as_deref() == Some("RATE_LIMITED"))
        {
            return Err(Problem::RateLimited {
                reset_at: self.rate_limit("graphql").map(|limit| limit.reset_at),
            });
        }
        // A repository that does not exist is an error next to the others' data.
        let Some(data) = answer.data else {
            let messages: Vec<_> = answer.errors.into_iter().map(|e| e.message).collect();
            return Err(Problem::GitHub {
                status: 200,
                message: messages.join("; "),
            });
        };

        let mut found = Vec::new();
        let mut unread = Vec::new();
        for index in 0..repos.len() {
            let Some(repo) = data
                .get(&format!("r{index}"))
                .and_then(|value| GraphRepo::deserialize(value).ok())
            else {
                continue;
            };
            let (info, manifest) = repo.into_parts();
            let Some(info) = info else { continue };
            match manifest {
                Blob::Missing => found.push((info, None)),
                Blob::Text(text) => found.push((info, Some(text))),
                Blob::TooLarge => {}
                Blob::Unread => unread.push(info),
            }
        }
        // GraphQL leaves out the text of a large file; the raw file has it.
        let manifests = in_parallel(&unread, |info| self.manifest(&info.repo));
        let mut problem = None;
        for (info, manifest) in unread.into_iter().zip(manifests) {
            match manifest {
                Ok(manifest) => found.push((info, manifest)),
                Err(error) => problem = problem.or(Some(error)),
            }
        }
        Ok((found, problem))
    }

    /// The details of one repository, or `None` when it does not exist. Works without a token.
    pub fn repo(&self, repo: &RepoRef) -> Result<Option<RepoInfo>, Problem> {
        let response = self.api(format!("{API}/repos/{}/{}", repo.owner, repo.repo), None)?;
        if response.status == 404 {
            return Ok(None);
        }
        Ok(parse::<RestRepo>(&response)?.into_info())
    }

    /// The marketplace manifest on the default branch, or `None` when there is none.
    ///
    /// It comes from raw.githubusercontent.com, which does not count against the API's rate
    /// limits, so it is sent without the token. A manifest that cannot be read, such as one too
    /// large, counts as none.
    pub fn manifest(&self, repo: &RepoRef) -> Result<Option<String>, Problem> {
        let request = Request {
            url: format!("{RAW}/{}/{}/HEAD/{MANIFEST_PATH}", repo.owner, repo.repo),
            headers: Vec::new(),
            body: None,
            limit: MAX_MANIFEST_BYTES,
        };
        let response = match self.transport.send(&request) {
            Ok(response) => response,
            Err(Failure::Unreachable(message)) => return Err(Problem::Offline { message }),
            Err(Failure::Unreadable { .. }) => return Ok(None),
        };
        match response.status {
            200 => Ok(Some(response.body)),
            404 => Ok(None),
            _ => Err(problem(&response)),
        }
    }

    /// The login of the account that the token belongs to.
    pub fn user(&self) -> Result<String, Problem> {
        #[derive(Deserialize)]
        struct User {
            login: String,
        }
        let response = self.api(format!("{API}/user"), None)?;
        Ok(parse::<User>(&response)?.login)
    }

    /// Starts GitHub's device flow for the OAuth App `client_id`: the person enters the code
    /// that this returns on github.com. No scope is asked for, because Discover reads only public
    /// repositories.
    pub fn device_code(&self, client_id: &str) -> Result<DeviceCode, Problem> {
        let body = serde_json::json!({ "client_id": client_id });
        let response = self.oauth(format!("{WEB}/login/device/code"), &body)?;
        if let Some(error) = oauth_error(&response) {
            return Err(error);
        }
        parse(&response)
    }

    /// Asks whether the person entered the code of `device_code` yet.
    pub fn device_token(
        &self,
        client_id: &str,
        device_code: &str,
    ) -> Result<DeviceAnswer, Problem> {
        #[derive(Deserialize)]
        struct Answer {
            access_token: Option<String>,
            error: Option<String>,
            interval: Option<u64>,
        }
        let body = serde_json::json!({
            "client_id": client_id,
            "device_code": device_code,
            "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
        });
        let response = self.oauth(format!("{WEB}/login/oauth/access_token"), &body)?;
        let answer: Answer = parse(&response)?;
        match (answer.access_token, answer.error.as_deref()) {
            (Some(token), _) => Ok(DeviceAnswer::Token(token)),
            (None, Some("authorization_pending")) => Ok(DeviceAnswer::Pending),
            (None, Some("slow_down")) => Ok(DeviceAnswer::SlowDown {
                interval: answer.interval.unwrap_or(10),
            }),
            (None, Some("expired_token")) => Ok(DeviceAnswer::Expired),
            (None, Some("access_denied")) => Ok(DeviceAnswer::Denied),
            _ => Err(oauth_error(&response).unwrap_or_else(|| Problem::GitHub {
                status: response.status,
                message: "unexpected answer".to_owned(),
            })),
        }
    }

    /// Sends a request to GitHub's OAuth endpoints, which take no token.
    fn oauth(&self, url: String, body: &Value) -> Result<Response, Problem> {
        let request = Request {
            url,
            headers: vec![("Accept", "application/json".to_owned())],
            body: Some(body.to_string()),
            limit: MAX_API_BYTES,
        };
        let response = self
            .transport
            .send(&request)
            .map_err(|failure| match failure {
                Failure::Unreachable(message) => Problem::Offline { message },
                Failure::Unreadable { status, message } => Problem::GitHub {
                    status,
                    message: format!("cannot read the answer: {message}"),
                },
            })?;
        if (200..300).contains(&response.status) {
            Ok(response)
        } else {
            Err(oauth_error(&response).unwrap_or_else(|| problem(&response)))
        }
    }

    /// Sends an API request, and returns the answer when it succeeded or found nothing.
    fn api(&self, url: String, body: Option<String>) -> Result<Response, Problem> {
        let mut headers = vec![
            ("Accept", "application/vnd.github+json".to_owned()),
            ("X-GitHub-Api-Version", "2022-11-28".to_owned()),
        ];
        if let Some(token) = &self.token {
            headers.push(("Authorization", format!("Bearer {token}")));
        }
        let request = Request {
            url,
            headers,
            body,
            limit: MAX_API_BYTES,
        };
        let response = self
            .transport
            .send(&request)
            .map_err(|failure| match failure {
                Failure::Unreachable(message) => Problem::Offline { message },
                Failure::Unreadable { status, message } => Problem::GitHub {
                    status,
                    message: format!("cannot read the answer: {message}"),
                },
            })?;
        if let Some(limit) = rate_limit(&response.headers) {
            self.rate_limits
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(limit.resource.clone(), limit);
        }
        if (200..300).contains(&response.status) || response.status == 404 {
            Ok(response)
        } else {
            Err(problem(&response))
        }
    }
}

/// A repository in a REST answer.
#[derive(Deserialize)]
struct RestRepo {
    full_name: String,
    description: Option<String>,
    #[serde(default)]
    stargazers_count: u64,
    pushed_at: Option<String>,
    license: Option<License>,
    #[serde(default)]
    archived: bool,
    #[serde(default)]
    fork: bool,
}

impl RestRepo {
    fn into_info(self) -> Option<RepoInfo> {
        Some(RepoInfo {
            repo: RepoRef::parse(&self.full_name)?,
            description: self.description,
            stars: self.stargazers_count,
            pushed_at: self.pushed_at,
            license: self.license.and_then(License::label),
            archived: self.archived,
            fork: self.fork,
        })
    }
}

/// A repository in a GraphQL answer.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphRepo {
    name_with_owner: String,
    description: Option<String>,
    #[serde(default)]
    stargazer_count: u64,
    pushed_at: Option<String>,
    license_info: Option<License>,
    #[serde(default)]
    is_archived: bool,
    #[serde(default)]
    is_fork: bool,
    manifest: Option<GraphBlob>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphBlob {
    text: Option<String>,
    #[serde(default)]
    is_truncated: bool,
    #[serde(default)]
    byte_size: u64,
}

/// What a GraphQL answer holds of a manifest.
enum Blob {
    /// There is no such file.
    Missing,
    Text(String),
    /// The text is left out, and the raw file has it.
    Unread,
    /// Larger than harness-hanger reads.
    TooLarge,
}

impl GraphRepo {
    fn into_parts(self) -> (Option<RepoInfo>, Blob) {
        let blob = match self.manifest {
            None => Blob::Missing,
            Some(blob) if blob.byte_size > MAX_MANIFEST_BYTES => Blob::TooLarge,
            Some(GraphBlob {
                text: Some(text),
                is_truncated: false,
                ..
            }) => Blob::Text(text),
            Some(_) => Blob::Unread,
        };
        let info = RepoRef::parse(&self.name_with_owner).map(|repo| RepoInfo {
            repo,
            description: self.description,
            stars: self.stargazer_count,
            pushed_at: self.pushed_at,
            license: self.license_info.and_then(License::label),
            archived: self.is_archived,
            fork: self.is_fork,
        });
        (info, blob)
    }
}

/// A license in either API, which have the same field names apart from case.
#[derive(Deserialize)]
struct License {
    #[serde(alias = "spdxId")]
    spdx_id: Option<String>,
    name: Option<String>,
}

impl License {
    /// The SPDX ID, else the name, such as `Other` for a license GitHub does not recognize.
    fn label(self) -> Option<String> {
        self.spdx_id
            .filter(|id| !id.is_empty() && id != "NOASSERTION")
            .or(self.name)
    }
}

/// A GraphQL query for `count` repositories, with their owners and names in the variables `o0`,
/// `n0`, `o1`, `n1`, and so on.
fn graphql_query(count: usize) -> String {
    let params: Vec<_> = (0..count)
        .map(|i| format!("$o{i}: String!, $n{i}: String!"))
        .collect();
    let fields = (0..count)
        .map(|i| format!("r{i}: repository(owner: $o{i}, name: $n{i}) {{ ...repo }} "))
        .collect::<Vec<_>>()
        .concat();
    format!(
        "query({}) {{ {fields}}} fragment repo on Repository {{ nameWithOwner description \
         stargazerCount pushedAt isArchived isFork licenseInfo {{ spdxId name }} \
         manifest: object(expression: \"HEAD:{MANIFEST_PATH}\") {{ ... on Blob {{ text \
         isTruncated byteSize }} }} }}",
        params.join(", ")
    )
}

fn parse<T: DeserializeOwned>(response: &Response) -> Result<T, Problem> {
    serde_json::from_str(&response.body).map_err(|error| Problem::GitHub {
        status: response.status,
        message: format!("unexpected answer: {error}"),
    })
}

fn rate_limit(headers: &HashMap<String, String>) -> Option<RateLimit> {
    let number = |name: &str| headers.get(name)?.parse::<u64>().ok();
    Some(RateLimit {
        resource: headers.get("x-ratelimit-resource")?.clone(),
        limit: number("x-ratelimit-limit")?,
        remaining: number("x-ratelimit-remaining")?,
        reset_at: rfc3339(headers.get("x-ratelimit-reset")?.parse().ok()?),
    })
}

/// What went wrong, from an answer that is not a success.
fn problem(response: &Response) -> Problem {
    let headers = &response.headers;
    let message = error_message(&response.body);
    let exhausted = headers
        .get("x-ratelimit-remaining")
        .is_some_and(|remaining| remaining == "0");
    let retry_after = headers
        .get("retry-after")
        .and_then(|seconds| seconds.parse::<i64>().ok());
    let limited = response.status == 429
        || (response.status == 403
            && (exhausted
                || retry_after.is_some()
                || message.to_lowercase().contains("rate limit")));
    if limited {
        let reset = headers
            .get("x-ratelimit-reset")
            .and_then(|reset| reset.parse::<i64>().ok())
            .filter(|_| exhausted);
        let reset_at = reset
            .or_else(|| retry_after.map(|seconds| now() + seconds))
            .map(rfc3339);
        return Problem::RateLimited { reset_at };
    }
    if response.status == 401 {
        return Problem::Unauthorized { message };
    }
    Problem::GitHub {
        status: response.status,
        message,
    }
}

/// The error that an OAuth endpoint reported in a successful answer, such as a client ID that
/// GitHub does not know.
fn oauth_error(response: &Response) -> Option<Problem> {
    let value: Value = serde_json::from_str(&response.body).ok()?;
    let error = value.get("error")?.as_str()?;
    let description = value
        .get("error_description")
        .and_then(Value::as_str)
        .unwrap_or(error);
    Some(Problem::GitHub {
        status: response.status,
        message: description.to_owned(),
    })
}

/// GitHub's `message`, with the first detail from `errors`, else the start of the body.
fn error_message(body: &str) -> String {
    let parsed = serde_json::from_str::<Value>(body).ok().and_then(|value| {
        let message = value.get("message")?.as_str()?.to_owned();
        let detail = value
            .pointer("/errors/0/message")
            .and_then(Value::as_str)
            .map(|detail| format!("{message}: {detail}"));
        Some(detail.unwrap_or(message))
    });
    parsed.unwrap_or_else(|| body.trim().chars().take(200).collect())
}

/// The successes among `results`, with the first problem among the others. When every one
/// failed, that problem is the result, since the same one, such as no network, most likely
/// stopped them all.
pub(crate) fn successes<T>(results: Vec<Result<T, Problem>>) -> Result<Partial<T>, Problem> {
    let mut found = Vec::new();
    let mut first_problem = None;
    for result in results {
        match result {
            Ok(value) => found.push(value),
            Err(problem) => {
                first_problem.get_or_insert(problem);
            }
        }
    }
    match first_problem {
        Some(problem) if found.is_empty() => Err(problem),
        problem => Ok((found, problem)),
    }
}

/// Runs `work` on each item, a few at a time, and returns the results in the items' order.
pub(crate) fn in_parallel<T: Sync, R: Send>(items: &[T], work: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<R>>> = Mutex::new(items.iter().map(|_| None).collect());
    thread::scope(|scope| {
        for _ in 0..WORKERS.min(items.len()) {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(item) = items.get(index) else { break };
                    let result = work(item);
                    results.lock().unwrap_or_else(PoisonError::into_inner)[index] = Some(result);
                }
            });
        }
    });
    results
        .into_inner()
        .unwrap_or_else(PoisonError::into_inner)
        .into_iter()
        .map(|result| result.expect("a scoped thread that panicked stops the scope"))
        .collect()
}

#[cfg(test)]
pub(crate) mod fake {
    use std::sync::Arc;

    use super::*;

    /// A request that the fake received.
    #[derive(Debug, Clone)]
    pub struct Sent {
        pub url: String,
        pub authorized: bool,
        pub body: Option<String>,
    }

    enum Answer {
        Reply(u16, Vec<(String, String)>, String),
        Unreadable,
        Unreachable,
    }

    /// Canned answers by URL prefix, and a log of what was sent. Clones share both, so a test
    /// keeps one while a [`GitHub`] uses another. An unknown URL gets a 404.
    #[derive(Clone, Default)]
    pub struct Fake {
        answers: Arc<Mutex<Vec<(String, Answer)>>>,
        sent: Arc<Mutex<Vec<Sent>>>,
    }

    impl Fake {
        pub fn on(&self, prefix: &str, status: u16, body: &str) -> &Self {
            self.on_with(prefix, status, &[], body)
        }

        pub fn on_with(
            &self,
            prefix: &str,
            status: u16,
            headers: &[(&str, &str)],
            body: &str,
        ) -> &Self {
            let headers = headers
                .iter()
                .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
                .collect();
            self.answers.lock().unwrap().push((
                prefix.to_owned(),
                Answer::Reply(status, headers, body.to_owned()),
            ));
            self
        }

        /// Answers `prefix` with something that cannot be read, such as a file too large.
        pub fn unreadable(&self, prefix: &str) -> &Self {
            self.answers
                .lock()
                .unwrap()
                .push((prefix.to_owned(), Answer::Unreadable));
            self
        }

        /// Answers nothing more, as without a network.
        pub fn go_offline(&self) {
            let mut answers = self.answers.lock().unwrap();
            answers.clear();
            answers.push((String::new(), Answer::Unreachable));
        }

        pub fn sent(&self) -> Vec<Sent> {
            self.sent.lock().unwrap().clone()
        }

        pub fn github(&self, token: Option<&str>) -> GitHub {
            GitHub::new(Box::new(self.clone()), token.map(str::to_owned))
        }
    }

    impl Transport for Fake {
        fn send(&self, request: &Request) -> Result<Response, Failure> {
            self.sent.lock().unwrap().push(Sent {
                url: request.url.clone(),
                authorized: request
                    .headers
                    .iter()
                    .any(|(name, _)| *name == "Authorization"),
                body: request.body.clone(),
            });
            let answers = self.answers.lock().unwrap();
            // The longest matching prefix, so that a test can answer one URL apart.
            let answer = answers
                .iter()
                .filter(|(prefix, _)| request.url.starts_with(prefix.as_str()))
                .max_by_key(|(prefix, _)| prefix.len());
            match answer {
                Some((_, Answer::Unreachable)) => {
                    Err(Failure::Unreachable("connection refused".to_owned()))
                }
                Some((_, Answer::Unreadable)) => Err(Failure::Unreadable {
                    status: 200,
                    message: "body too large".to_owned(),
                }),
                Some((_, Answer::Reply(status, headers, body))) => Ok(Response {
                    status: *status,
                    headers: headers.iter().cloned().collect(),
                    body: body.clone(),
                }),
                None => Ok(Response {
                    status: 404,
                    headers: HashMap::new(),
                    body: r#"{"message":"Not Found"}"#.to_owned(),
                }),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::Fake;
    use super::*;

    const MANIFEST: &str = r#"{ "name": "acme-tools", "plugins": [] }"#;

    fn repo(full_name: &str) -> RepoRef {
        RepoRef::parse(full_name).unwrap()
    }

    fn response(status: u16, headers: &[(&str, &str)], body: &str) -> Response {
        Response {
            status,
            headers: headers
                .iter()
                .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
                .collect(),
            body: body.to_owned(),
        }
    }

    #[test]
    fn reports_a_rate_limit_with_its_reset_time() {
        let exhausted = response(
            403,
            &[
                ("x-ratelimit-remaining", "0"),
                ("x-ratelimit-reset", "1792152351"),
            ],
            r#"{"message":"API rate limit exceeded"}"#,
        );
        assert_eq!(
            problem(&exhausted),
            Problem::RateLimited {
                reset_at: Some("2026-10-16T12:05:51Z".to_owned())
            }
        );

        let secondary = response(429, &[("retry-after", "60")], "");
        assert!(matches!(
            problem(&secondary),
            Problem::RateLimited { reset_at: Some(_) }
        ));
        // raw.githubusercontent.com limits without saying until when.
        assert_eq!(
            problem(&response(429, &[], "")),
            Problem::RateLimited { reset_at: None }
        );
    }

    #[test]
    fn tells_other_errors_apart() {
        assert_eq!(
            problem(&response(401, &[], r#"{"message":"Bad credentials"}"#)),
            Problem::Unauthorized {
                message: "Bad credentials".to_owned()
            }
        );
        assert_eq!(
            problem(&response(
                422,
                &[],
                r#"{"message":"Validation Failed","errors":[{"message":"The query is too long"}]}"#
            )),
            Problem::GitHub {
                status: 422,
                message: "Validation Failed: The query is too long".to_owned()
            }
        );
        // A 403 that is not about a rate limit, such as a blocked repository.
        assert!(matches!(
            problem(&response(
                403,
                &[],
                r#"{"message":"Repository access blocked"}"#
            )),
            Problem::GitHub { status: 403, .. }
        ));
    }

    #[test]
    fn says_why_github_cannot_be_reached() {
        let fake = Fake::default();
        fake.go_offline();

        let error = fake.github(None).search_topic("x").unwrap_err();

        assert_eq!(
            error,
            Problem::Offline {
                message: "connection refused".to_owned()
            }
        );
        assert_eq!(error.to_string(), "cannot reach GitHub: connection refused");
    }

    #[test]
    fn finds_repositories_with_a_manifest_at_their_root() {
        let fake = Fake::default();
        fake.on_with(
            "https://api.github.com/search/code",
            200,
            &[
                ("x-ratelimit-resource", "code_search"),
                ("x-ratelimit-limit", "10"),
                ("x-ratelimit-remaining", "9"),
                ("x-ratelimit-reset", "1792152351"),
            ],
            r#"{ "total_count": 120, "items": [
                { "path": ".claude-plugin/marketplace.json", "repository": { "full_name": "Acme/Tools" } },
                { "path": "sub/.claude-plugin/marketplace.json", "repository": { "full_name": "acme/nested" } },
                { "path": ".claude-plugin/marketplace.json", "repository": { "full_name": "acme/fork", "fork": true } },
                { "path": ".claude-plugin/marketplace.json", "repository": { "full_name": "acme/tools" } },
                { "path": ".claude-plugin/marketplace.json", "repository": { "full_name": "not a repo" } }
            ] }"#,
        );
        let github = fake.github(Some("secret"));

        let (repos, total) = github.search_code("code review").unwrap();

        assert_eq!(repos, vec![repo("Acme/Tools")]);
        assert_eq!(total, 120);
        let sent = fake.sent();
        assert_eq!(
            sent[0].url,
            "https://api.github.com/search/code?q=code%20review%20filename%3Amarketplace.json%20path%3A.claude-plugin&per_page=50"
        );
        assert!(sent[0].authorized);
        assert_eq!(github.rate_limit("code_search").unwrap().remaining, 9);
    }

    #[test]
    fn reads_details_and_manifests_through_graphql() {
        let fake = Fake::default();
        fake.on(
            "https://api.github.com/graphql",
            200,
            &serde_json::json!({ "data": {
                "r0": {
                    "nameWithOwner": "Acme/Tools", "description": "Acme's tools", "stargazerCount": 7,
                    "pushedAt": "2026-10-01T00:00:00Z", "isArchived": true, "isFork": false,
                    "licenseInfo": { "spdxId": "MIT", "name": "MIT License" },
                    "manifest": { "text": MANIFEST, "isTruncated": false, "byteSize": 40 }
                },
                "r1": null,
                "r2": {
                    "nameWithOwner": "beta/big", "stargazerCount": 1,
                    "licenseInfo": { "spdxId": "NOASSERTION", "name": "Other" },
                    "manifest": { "text": null, "isTruncated": true, "byteSize": 600_000 }
                },
                "r3": { "nameWithOwner": "beta/none", "manifest": null }
            }, "errors": [{ "type": "NOT_FOUND", "message": "Could not resolve to a Repository" }] })
            .to_string(),
        );
        fake.on(
            "https://raw.githubusercontent.com/beta/big/HEAD/.claude-plugin/marketplace.json",
            200,
            r#"{ "name": "big", "plugins": [] }"#,
        );
        let refs = [
            repo("Acme/Tools"),
            repo("gone/away"),
            repo("beta/big"),
            repo("beta/none"),
        ];

        let (found, problem) = fake.github(Some("secret")).repos(&refs).unwrap();

        assert_eq!(problem, None);

        let (acme, manifest) = &found[0];
        assert_eq!(acme.repo, repo("Acme/Tools"));
        assert_eq!((acme.stars, acme.archived), (7, true));
        assert_eq!(acme.license.as_deref(), Some("MIT"));
        assert_eq!(manifest.as_deref(), Some(MANIFEST));
        assert_eq!(found[1].0.repo, repo("beta/none"));
        assert_eq!(found[1].1, None);
        // The text that GraphQL left out comes from the raw file.
        assert_eq!(found[2].0.license.as_deref(), Some("Other"));
        assert_eq!(
            found[2].1.as_deref(),
            Some(r#"{ "name": "big", "plugins": [] }"#)
        );
        assert_eq!(found.len(), 3);

        let sent = fake.sent();
        let body: Value = serde_json::from_str(sent[0].body.as_deref().unwrap()).unwrap();
        assert_eq!(body["variables"]["o0"], "Acme");
        assert_eq!(body["variables"]["n3"], "none");
        // The raw file is read without the token.
        assert!(sent[0].authorized && !sent[1].authorized);
    }

    #[test]
    fn asks_graphql_in_small_requests_at_once() {
        let fake = Fake::default();
        fake.on("https://api.github.com/graphql", 200, r#"{ "data": {} }"#);
        let refs: Vec<_> = (0..25).map(|i| repo(&format!("acme/r{i}"))).collect();

        fake.github(Some("secret")).repos(&refs).unwrap();

        assert_eq!(fake.sent().len(), 3);
    }

    #[test]
    fn reports_a_graphql_rate_limit() {
        let fake = Fake::default();
        fake.on_with(
            "https://api.github.com/graphql",
            200,
            &[
                ("x-ratelimit-resource", "graphql"),
                ("x-ratelimit-limit", "5000"),
                ("x-ratelimit-remaining", "0"),
                ("x-ratelimit-reset", "1792152351"),
            ],
            r#"{ "errors": [{ "type": "RATE_LIMITED", "message": "API rate limit exceeded" }] }"#,
        );

        let error = fake
            .github(Some("secret"))
            .repos(&[repo("acme/tools")])
            .unwrap_err();

        assert_eq!(
            error,
            Problem::RateLimited {
                reset_at: Some("2026-10-16T12:05:51Z".to_owned())
            }
        );
    }

    #[test]
    fn reads_a_repository_without_a_token() {
        let fake = Fake::default();
        fake.on(
            "https://api.github.com/repos/acme/tools",
            200,
            r#"{ "full_name": "acme/tools", "stargazers_count": 3, "license": null }"#,
        );
        let github = fake.github(None);

        let info = github.repo(&repo("acme/tools")).unwrap().unwrap();

        assert_eq!(info.stars, 3);
        assert_eq!(
            github.repo(&repo("acme/gone")).unwrap().map(|i| i.stars),
            None
        );
        assert!(fake.sent().iter().all(|sent| !sent.authorized));
    }

    #[test]
    fn keeps_successes_unless_everything_failed() {
        let offline = || Problem::Offline {
            message: "down".to_owned(),
        };
        assert_eq!(
            successes(vec![Ok(1), Err(offline()), Ok(2)]),
            Ok((vec![1, 2], Some(offline())))
        );
        assert_eq!(successes(vec![Ok(1)]), Ok((vec![1], None)));
        assert_eq!(
            successes::<i32>(vec![Err(offline()), Err(offline())]),
            Err(offline())
        );
        assert_eq!(successes::<i32>(vec![]), Ok((vec![], None)));
    }

    #[test]
    fn counts_a_manifest_that_cannot_be_read_as_none() {
        let fake = Fake::default();
        fake.unreadable("https://raw.githubusercontent.com/");
        fake.unreadable("https://api.github.com/");
        let github = fake.github(None);

        assert_eq!(github.manifest(&repo("acme/huge")), Ok(None));
        // An API answer that cannot be read is not a network failure.
        assert_eq!(
            github.repo(&repo("acme/huge")).unwrap_err(),
            Problem::GitHub {
                status: 200,
                message: "cannot read the answer: body too large".to_owned()
            }
        );
    }

    #[test]
    fn runs_work_in_parallel_and_keeps_the_order() {
        let items: Vec<u32> = (0..40).collect();
        assert_eq!(
            in_parallel(&items, |n| n * 2),
            items.iter().map(|n| n * 2).collect::<Vec<_>>()
        );
    }
}
