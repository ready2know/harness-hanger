//! Where Claude Code keeps its configuration, and where harness-hanger keeps its own data.

use std::env;
use std::ffi::OsString;
use std::path::{self, PathBuf};

use crate::{Error, Result};

/// Environment variable Claude Code reads to relocate its user config directory.
pub const CLAUDE_CONFIG_DIR_ENV: &str = "CLAUDE_CONFIG_DIR";

/// Environment variable that relocates the harness-hanger data directory.
pub const DATA_DIR_ENV: &str = "HARNESS_HANGER_DATA_DIR";

/// Returns the Claude Code user config directory: `$CLAUDE_CONFIG_DIR`, else `~/.claude`.
pub fn claude_config_dir() -> Result<PathBuf> {
    resolve_claude_config_dir(env::var_os(CLAUDE_CONFIG_DIR_ENV), env::home_dir())
}

/// Returns the harness-hanger data directory: `$HARNESS_HANGER_DATA_DIR`, else `harness-hanger`
/// in the platform's local data directory.
pub fn data_dir() -> Result<PathBuf> {
    resolve_data_dir(env::var_os(DATA_DIR_ENV), dirs::data_local_dir())
}

/// Expands a leading `~` and makes `input` absolute against the current directory. `None` when
/// `input` is empty, or starts with `~` and there is no home directory.
pub(crate) fn expand(input: &str) -> Option<PathBuf> {
    let expanded = match input.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with(['/', '\\']) => {
            env::home_dir()?.join(rest.trim_start_matches(['/', '\\']))
        }
        _ => PathBuf::from(input),
    };
    path::absolute(expanded).ok()
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

fn resolve_data_dir(override_dir: Option<OsString>, platform: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(dir) = override_dir.filter(|dir| !dir.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    platform
        .map(|dir| dir.join("harness-hanger"))
        .ok_or(Error::DataDirNotFound)
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

    #[test]
    fn data_dir_env_override_wins_over_platform() {
        let dir = resolve_data_dir(Some("/custom".into()), Some("/data".into())).unwrap();
        assert_eq!(dir, PathBuf::from("/custom"));
    }

    #[test]
    fn data_dir_defaults_to_a_subdirectory_of_the_platform_dir() {
        let dir = resolve_data_dir(Some(OsString::new()), Some("/data".into())).unwrap();
        assert_eq!(dir, PathBuf::from("/data/harness-hanger"));
    }

    #[test]
    fn no_override_and_no_platform_dir_is_an_error() {
        assert!(matches!(
            resolve_data_dir(None, None),
            Err(Error::DataDirNotFound)
        ));
    }
}
