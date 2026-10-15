//! Where Claude Code keeps its configuration on disk.

use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

use crate::{Error, Result};

/// Environment variable Claude Code reads to relocate its user config directory.
pub const CLAUDE_CONFIG_DIR_ENV: &str = "CLAUDE_CONFIG_DIR";

/// Returns the Claude Code user config directory: `$CLAUDE_CONFIG_DIR`, else `~/.claude`.
pub fn claude_config_dir() -> Result<PathBuf> {
    resolve_claude_config_dir(env::var_os(CLAUDE_CONFIG_DIR_ENV), env::home_dir())
}

fn resolve_claude_config_dir(
    override_dir: Option<OsString>,
    home: Option<PathBuf>,
) -> Result<PathBuf> {
    if let Some(dir) = override_dir.filter(|dir| !dir.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    home.map(|home| home.join(".claude"))
        .ok_or(Error::ConfigDirNotFound)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_override_wins_over_home() {
        let dir =
            resolve_claude_config_dir(Some("/custom".into()), Some("/home/u".into())).unwrap();
        assert_eq!(dir, PathBuf::from("/custom"));
    }

    #[test]
    fn empty_override_falls_back_to_home() {
        let dir = resolve_claude_config_dir(Some(OsString::new()), Some("/home/u".into())).unwrap();
        assert_eq!(dir, PathBuf::from("/home/u/.claude"));
    }

    #[test]
    fn no_override_and_no_home_is_an_error() {
        assert!(matches!(
            resolve_claude_config_dir(None, None),
            Err(Error::ConfigDirNotFound)
        ));
    }
}
