//! The system `git`, used to fetch marketplaces.

use std::path::Path;
use std::process::Command;

use crate::{Error, Result, process};

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

/// The commit that `repo` has checked out.
pub(crate) fn head(repo: &Path) -> Result<String> {
    let output = git()
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|source| Error::Git(format!("cannot run git: {source}")))?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned());
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

/// The variables that tie git to one repository, as `git rev-parse --local-env-vars` lists them.
///
/// Git sets some of them for the hooks it runs. A git run from a hook, for instance by
/// `cargo test` in `pre-push`, would otherwise work on that repository instead of the one in its
/// working directory.
const LOCAL_ENV_VARS: &[&str] = &[
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_CONFIG",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_OBJECT_DIRECTORY",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_GRAFT_FILE",
    "GIT_INDEX_FILE",
    "GIT_NO_REPLACE_OBJECTS",
    "GIT_REPLACE_REF_BASE",
    "GIT_PREFIX",
    "GIT_SHALLOW_FILE",
    "GIT_COMMON_DIR",
];

/// Makes `command` find its repository from its working directory or arguments only.
pub(crate) fn detach(command: &mut Command) -> &mut Command {
    for name in LOCAL_ENV_VARS {
        command.env_remove(name);
    }
    command
}

fn git() -> Command {
    let mut command = process::background("git");
    detach(&mut command);
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
    command
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    /// Runs git in `dir` for a test, and returns what it printed.
    fn run_git(dir: &Path, args: &[&str]) -> String {
        let output = detach(&mut Command::new("git"))
            .args(["-c", "user.name=test", "-c", "user.email=test@example.com"])
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?} failed");
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    #[test]
    fn ignores_the_repository_that_the_environment_names() {
        let tmp = tempfile::tempdir().unwrap();
        let named = tmp.path().join("named");
        let own = tmp.path().join("own");
        fs::create_dir(&named).unwrap();
        fs::create_dir(&own).unwrap();
        run_git(&named, &["init", "--quiet"]);

        // As in a git hook, which names the repository that runs it.
        let status = detach(Command::new("git").env("GIT_DIR", named.join(".git")))
            .args(["init", "--quiet"])
            .current_dir(&own)
            .status()
            .unwrap();

        assert!(status.success());
        assert!(own.join(".git").is_dir());
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
        assert_eq!(
            run_git(&dest, &["rev-parse", "--is-shallow-repository"]),
            "true"
        );

        set_origin(&dest, "https://github.com/acme/tools.git").unwrap();
        assert_eq!(
            run_git(&dest, &["remote", "get-url", "origin"]),
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
