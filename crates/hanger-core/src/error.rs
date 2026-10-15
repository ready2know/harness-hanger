#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "cannot locate the Claude Code config directory: CLAUDE_CONFIG_DIR is unset and no home directory is known"
    )]
    ConfigDirNotFound,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
