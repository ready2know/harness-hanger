//! The system `git`, used to fetch marketplaces.

use std::path::Path;
use std::process::Command;

use crate::{Error, Result};

/// Clones the default branch of `url` into `dest`, without history. `url` can also be a local
/// repository, such as a copy that a harness keeps.
pub(crate) fn shallow_clone(url: &str, dest: &Path) -> Result<()> {
    let output = git()
        // Give up on a transfer that stalls below 1 KB/s for a minute.
        .args([
            "-c",
            "http.lowSpeedLimit=1000",
            "-c",
            "http.lowSpeedTime=60",
        ])
        // `--no-local` makes `--depth` apply to a local repository too. `--origin` keeps the
        // remote's name whatever `clone.defaultRemoteName` says, so `set_origin` can find it.
        .args([
            "clone",
            "--depth",
            "1",
            "--no-local",
            "--origin",
            "origin",
            "--quiet",
            "--",
        ])
        .arg(url)
        .arg(dest)
        .output()
        .map_err(|source| Error::Git(format!("cannot run git: {source}")))?;
    if output.status.success() {
        return Ok(());
    }
    Err(Error::Git(describe_failure(&String::from_utf8_lossy(
        &output.stderr,
    ))))
}

/// Points the `origin` remote of `repo` at `url`.
pub(crate) fn set_origin(repo: &Path, url: &str) -> Result<()> {
    let output = git()
        .arg("-C")
        .arg(repo)
        .args(["remote", "set-url", "origin", url])
        .output()
        .map_err(|source| Error::Git(format!("cannot run git: {source}")))?;
    if output.status.success() {
        return Ok(());
    }
    Err(Error::Git(
        String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    ))
}

/// Explains the failure that git reports when a server asks for credentials that are not there.
///
/// GitHub asks for them both for a missing repository and for a private one.
fn describe_failure(stderr: &str) -> String {
    const NEEDS_CREDENTIALS: &[&str] = &[
        "could not read Username",
        "Cannot prompt",
        "terminal prompts disabled",
        "Authentication failed",
        "Repository not found",
    ];
    if NEEDS_CREDENTIALS.iter().any(|text| stderr.contains(text)) {
        return "the repository does not exist, or it is private and git has no credentials for it"
            .to_owned();
    }
    stderr.trim().to_owned()
}

fn git() -> Command {
    let mut command = Command::new("git");
    // Fail instead of asking for credentials, which the desktop app has nowhere to show.
    // Saved credentials still work. An empty GIT_ASKPASS also stops git from trying
    // `core.askPass` and SSH_ASKPASS.
    command
        .args(["-c", "credential.interactive=never"])
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "")
        .env("GCM_INTERACTIVE", "never");
    // English messages, which `describe_failure` recognizes.
    command.env("LC_ALL", "C");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Keeps a console window from flashing up when the desktop app runs git.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn run_git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(["-c", "user.name=test", "-c", "user.email=test@example.com"])
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap()
            .status;
        assert!(status.success(), "git {args:?} failed");
    }

    #[test]
    fn clones_a_repository() {
        let tmp = tempfile::tempdir().unwrap();
        let origin = tmp.path().join("origin");
        fs::create_dir(&origin).unwrap();
        run_git(&origin, &["init", "--quiet"]);
        for content in ["first", "hello"] {
            fs::write(origin.join("README.md"), content).unwrap();
            run_git(&origin, &["add", "."]);
            run_git(&origin, &["commit", "--quiet", "-m", content]);
        }

        // A local path, as when cloning from a copy that a harness keeps.
        let dest = tmp.path().join("clone");
        shallow_clone(origin.to_str().unwrap(), &dest).unwrap();

        assert_eq!(fs::read_to_string(dest.join("README.md")).unwrap(), "hello");
        let shallow = Command::new("git")
            .args(["rev-parse", "--is-shallow-repository"])
            .current_dir(&dest)
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&shallow.stdout).trim(), "true");

        set_origin(&dest, "https://github.com/acme/tools.git").unwrap();
        let origin = Command::new("git")
            .args(["remote", "get-url", "origin"])
            .current_dir(&dest)
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&origin.stdout).trim(),
            "https://github.com/acme/tools.git"
        );
    }

    #[test]
    fn explains_a_request_for_credentials() {
        let stderr = "fatal: Cannot prompt because user interactivity has been disabled.\nfatal: unable to get password from user\n";
        assert!(describe_failure(stderr).starts_with("the repository does not exist"));
    }

    #[test]
    fn reports_what_git_said_on_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("missing");

        let error =
            shallow_clone(missing.to_str().unwrap(), &tmp.path().join("clone")).unwrap_err();

        assert!(matches!(&error, Error::Git(message) if message.contains("does not exist")));
    }
}
