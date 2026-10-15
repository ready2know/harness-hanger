use std::fmt;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{Error, Result, paths};

/// Where a marketplace in the catalog comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum MarketplaceSource {
    /// A GitHub repository. harness-hanger keeps a clone of it in its data directory.
    GitHub { owner: String, repo: String },
    /// A directory on this machine, such as a local clone. It is read in place.
    Local { path: PathBuf },
    /// The skills, agents, and commands that a harness loads for the user outside plugins, such as
    /// those in `~/.claude`. They are read in place. `harness` is the adapter's ID.
    Harness { harness: String },
}

impl MarketplaceSource {
    /// Parses what a person typed: a GitHub URL, `owner/repo`, or a local directory path.
    ///
    /// GitHub names are lowercased, because GitHub treats them case-insensitively. A relative path
    /// must start with `.` and resolves against the current directory.
    pub fn parse(input: &str) -> Result<Self> {
        let input = input.trim();
        let invalid = || Error::InvalidSource(input.to_owned());

        let lower = input.to_ascii_lowercase();
        if let Some(rest) = GITHUB_PREFIXES
            .iter()
            .find_map(|prefix| lower.strip_prefix(prefix))
        {
            return github(rest).ok_or_else(invalid);
        }
        if looks_like_path(input) {
            return paths::expand(input)
                .map(|path| Self::Local { path })
                .ok_or_else(invalid);
        }
        github(&lower).ok_or_else(invalid)
    }

    /// `kind` and `location` together identify a catalog entry.
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::GitHub { .. } => "github",
            Self::Local { .. } => "local",
            Self::Harness { .. } => "harness",
        }
    }

    pub(crate) fn location(&self) -> Result<String> {
        match self {
            Self::GitHub { owner, repo } => Ok(format!("{owner}/{repo}")),
            Self::Local { path } => path
                .to_str()
                .map(str::to_owned)
                .ok_or_else(|| Error::InvalidSource(path.display().to_string())),
            Self::Harness { harness } => Ok(harness.clone()),
        }
    }

    pub(crate) fn from_row(kind: &str, location: String) -> Option<Self> {
        match kind {
            // Checked again: the location decides which directory a removal deletes.
            "github" => github(&location),
            "local" => Some(Self::Local {
                path: location.into(),
            }),
            "harness" if is_harness_id(&location) => Some(Self::Harness { harness: location }),
            _ => None,
        }
    }
}

impl fmt::Display for MarketplaceSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GitHub { owner, repo } => write!(f, "github.com/{owner}/{repo}"),
            Self::Local { path } => write!(f, "{}", path.display()),
            Self::Harness { harness } => write!(f, "{harness} user directory"),
        }
    }
}

fn is_harness_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

const GITHUB_PREFIXES: &[&str] = &[
    "https://github.com/",
    "http://github.com/",
    "https://www.github.com/",
    "github.com/",
    "git@github.com:",
    "ssh://git@github.com/",
];

/// Parses `owner/repo`, allowing a trailing `/` or `.git`.
fn github(path: &str) -> Option<MarketplaceSource> {
    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let (owner, repo) = path.split_once('/')?;
    let valid_owner = !owner.is_empty()
        && owner.len() <= 39
        && owner.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    // Windows drops a trailing dot from a directory name, so `tools.` would share `tools`'s clone.
    let valid_repo = !repo.is_empty()
        && !repo.ends_with('.')
        && repo
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    (valid_owner && valid_repo).then(|| MarketplaceSource::GitHub {
        owner: owner.to_owned(),
        repo: repo.to_owned(),
    })
}

fn looks_like_path(input: &str) -> bool {
    let path = Path::new(input);
    path.is_absolute() || path.has_root() || input.starts_with('~') || input.starts_with('.')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn github_source(owner: &str, repo: &str) -> MarketplaceSource {
        MarketplaceSource::GitHub {
            owner: owner.into(),
            repo: repo.into(),
        }
    }

    #[test]
    fn parses_github_urls_and_shorthand() {
        for input in [
            "https://github.com/Acme/Tools",
            "https://github.com/acme/tools/",
            "https://github.com/acme/tools.git",
            "github.com/acme/tools",
            "git@github.com:acme/tools.git",
            "  acme/tools  ",
        ] {
            assert_eq!(
                MarketplaceSource::parse(input).unwrap(),
                github_source("acme", "tools"),
                "{input}"
            );
        }
    }

    #[test]
    fn keeps_dots_and_underscores_in_repository_names() {
        assert_eq!(
            MarketplaceSource::parse("acme/my_tools.v2").unwrap(),
            github_source("acme", "my_tools.v2")
        );
    }

    #[test]
    fn rejects_urls_that_are_not_a_repository() {
        for input in [
            "https://github.com/acme",
            "https://github.com/acme/tools/tree/main",
            "https://gitlab.com/acme/tools",
            "acme",
            "acme/tools.",
            "acme/..",
            "",
        ] {
            assert!(
                matches!(
                    MarketplaceSource::parse(input),
                    Err(Error::InvalidSource(_))
                ),
                "{input}"
            );
        }
    }

    #[test]
    fn parses_absolute_and_relative_paths() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().to_str().unwrap();
        assert_eq!(
            MarketplaceSource::parse(input).unwrap(),
            MarketplaceSource::Local {
                path: dir.path().into()
            }
        );

        let MarketplaceSource::Local { path } = MarketplaceSource::parse("./market").unwrap()
        else {
            panic!("expected a local source");
        };
        assert!(path.is_absolute());
        assert!(path.ends_with("market"));
    }

    #[test]
    fn expands_the_home_directory() {
        let home = std::env::home_dir().unwrap();
        assert_eq!(
            MarketplaceSource::parse("~/market").unwrap(),
            MarketplaceSource::Local {
                path: home.join("market")
            }
        );
    }

    #[test]
    fn round_trips_through_a_catalog_row() {
        for source in [
            github_source("acme", "tools"),
            MarketplaceSource::Local {
                path: "/srv/market".into(),
            },
            MarketplaceSource::Harness {
                harness: "claude-code".into(),
            },
        ] {
            let restored =
                MarketplaceSource::from_row(source.kind(), source.location().unwrap()).unwrap();
            assert_eq!(restored, source);
        }
    }

    #[test]
    fn skips_harness_rows_with_an_id_that_is_not_one() {
        assert_eq!(MarketplaceSource::from_row("harness", "../x".into()), None);
    }

    #[test]
    fn skips_catalog_rows_that_are_not_a_repository() {
        for location in ["x//Users/me", "x/../../..", "acme/tools/extra", "acme"] {
            assert_eq!(
                MarketplaceSource::from_row("github", location.into()),
                None,
                "{location}"
            );
        }
    }

    #[test]
    fn serializes_with_a_kind_tag() {
        assert_eq!(
            serde_json::to_value(github_source("acme", "tools")).unwrap(),
            serde_json::json!({ "kind": "github", "owner": "acme", "repo": "tools" })
        );
    }
}
