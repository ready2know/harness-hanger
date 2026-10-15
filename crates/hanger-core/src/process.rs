//! Commands that harness-hanger runs, such as `git` and `claude`.

use std::ffi::OsStr;
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
