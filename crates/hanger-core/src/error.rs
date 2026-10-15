use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "cannot locate the Claude Code config directory: CLAUDE_CONFIG_DIR is unset and no home directory is known"
    )]
    ConfigDirNotFound,

    #[error(
        "cannot locate the harness-hanger data directory: HARNESS_HANGER_DATA_DIR is unset and the platform has no data directory"
    )]
    DataDirNotFound,

    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error(
        "{0}: expected a GitHub repository (https://github.com/owner/repo or owner/repo) or a local directory path"
    )]
    InvalidSource(String),

    #[error("{} is not a marketplace: {reason}", path.display())]
    NotAMarketplace { path: PathBuf, reason: String },

    #[error("git failed: {0}")]
    Git(String),

    #[error("{0} is already in the catalog")]
    AlreadyAdded(String),

    #[error("no marketplace with ID {0} in the catalog")]
    MarketplaceNotFound(i64),

    #[error("harness-hanger cannot import from {0}")]
    UnknownHarness(String),

    #[error("{0} is not on this machine")]
    HarnessMissing(String),

    #[error("{item} cannot be imported: {reason}")]
    NotImportable { item: String, reason: String },

    #[error("{0}: expected the path of a folder")]
    InvalidProjectPath(String),

    #[error("no project with ID {0}")]
    ProjectNotFound(i64),

    #[error(
        "cannot find the claude command; install Claude Code, or add the folder that holds it to PATH"
    )]
    ClaudeMissing,

    #[error("Claude Code: {0}")]
    Harness(String),
}

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
