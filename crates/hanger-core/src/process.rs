//! Commands that harness-hanger runs, such as `git`, `claude`, and `gh`.

use std::env;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::process::Command;

/// A command that runs in the background: on Windows, no console window flashes up when the
/// desktop app runs it.
pub(crate) fn background(program: impl AsRef<OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// Finds a command named one of `names` in `path` (the value of `PATH`), then in `extra`, the
/// usual install locations, which a desktop app on macOS needs because it does not get the shell's
/// `PATH`.
///
/// Only absolute directories count: a relative one would resolve against the working directory,
/// such as a project folder, which can come from anyone.
pub(crate) fn find(names: &[&str], path: Option<OsString>, extra: Vec<PathBuf>) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = path
        .map(|path| env::split_paths(&path).collect())
        .unwrap_or_default();
    dirs.extend(extra);
    dirs.extend(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from));
    dirs.iter()
        .filter(|dir| dir.is_absolute())
        .flat_map(|dir| names.iter().map(move |name| dir.join(name)))
        .find(|candidate| candidate.is_file())
}
