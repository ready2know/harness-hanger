//! The GitHub account that harness-hanger asks GitHub as: a token from the environment, one saved
//! in the system keychain, or the login of the `gh` command.
//!
//! Nothing here reaches GitHub on its own. Only signing in, saving a token, and checking one do.

use std::env;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::client::{DeviceAnswer, DeviceCode, GitHub, Problem};
use crate::{Error, Result, process};

/// The keychain entry of a saved token.
const KEYCHAIN_SERVICE: &str = "harness-hanger";
const KEYCHAIN_USER: &str = "github.com";
/// The variables that hold a token, in the order that `gh` reads them.
const TOKEN_VARIABLES: [&str; 2] = ["GH_TOKEN", "GITHUB_TOKEN"];
/// The client ID of harness-hanger's OAuth App, which signing in in the browser uses. The device
/// flow needs no client secret, so the ID can be public. `HARNESS_HANGER_GITHUB_CLIENT_ID`
/// replaces it when building, such as for a fork with an app of its own; set empty, it leaves
/// a pasted token and the `gh` login only.
const CLIENT_ID: &str = match option_env!("HARNESS_HANGER_GITHUB_CLIENT_ID") {
    Some(id) => id,
    None => "Ov23liZogXf3QIOB6Mk6",
};
/// How long `gh` may take to print its one-time code.
const GH_CODE_TIMEOUT: Duration = Duration::from_secs(30);

/// Where the token that harness-hanger asks GitHub with comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TokenSource {
    /// `GH_TOKEN` or `GITHUB_TOKEN`. It wins over the others, as it does for `gh`.
    Env { variable: String },
    /// A token saved in the system keychain, by signing in or by pasting one.
    Saved,
    /// The login of the `gh` command.
    Gh,
}

/// The `gh` command on this machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhStatus {
    /// Whether it is signed in to github.com.
    pub signed_in: bool,
    /// The account it is signed in as, as its configuration says.
    pub login: Option<String>,
}

/// Which account harness-hanger asks GitHub as. Reading it sends nothing to GitHub.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountStatus {
    /// Where the token in use comes from, or `None` when there is none.
    pub source: Option<TokenSource>,
    /// The account of the token in use, when harness-hanger knows it without asking GitHub.
    pub login: Option<String>,
    /// Whether a token is saved in the keychain, also when another one is in use.
    pub saved: bool,
    /// The account of the saved token.
    pub saved_login: Option<String>,
    /// `None` when the `gh` command is not installed.
    pub gh: Option<GhStatus>,
    /// Whether this build can sign in in the browser.
    pub browser_sign_in: bool,
    /// Why the keychain cannot be read, if it cannot.
    pub keychain_error: Option<String>,
}

/// Where a sign-in in the browser is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum DevicePoll {
    /// The person has not entered the code yet. Ask again after `interval` seconds.
    Pending { interval: u64 },
    /// The token is saved.
    Done { status: AccountStatus },
}

/// What the person needs to finish signing in with `gh`: a code, and where to enter it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignInPrompt {
    pub user_code: String,
    pub verification_uri: String,
}

/// A token saved in the keychain, with the account that GitHub said it belongs to.
#[derive(Debug, Serialize, Deserialize)]
struct Saved {
    token: String,
    login: Option<String>,
}

impl Saved {
    /// Reads an entry. One that is not JSON is taken as a bare token.
    fn parse(secret: String) -> Self {
        serde_json::from_str(&secret).unwrap_or(Self {
            token: secret,
            login: None,
        })
    }
}

/// Where a saved token is kept. Tests use one in memory.
pub(crate) trait Keychain: Send + Sync {
    fn get(&self) -> Result<Option<String>, String>;
    fn set(&self, secret: &str) -> Result<(), String>;
    fn delete(&self) -> Result<(), String>;
}

/// The system keychain: Keychain on macOS, Credential Manager on Windows, and the Secret Service
/// on Linux.
struct SystemKeychain;

impl SystemKeychain {
    fn entry() -> Result<keyring::Entry, String> {
        keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_USER).map_err(|error| error.to_string())
    }
}

impl Keychain for SystemKeychain {
    fn get(&self) -> Result<Option<String>, String> {
        match Self::entry()?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }

    fn set(&self, secret: &str) -> Result<(), String> {
        Self::entry()?
            .set_password(secret)
            .map_err(|error| error.to_string())
    }

    fn delete(&self) -> Result<(), String> {
        match Self::entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
}

type Variables = Box<dyn Fn(&str) -> Option<String> + Send + Sync>;
type Connect = Box<dyn Fn(Option<String>) -> GitHub + Send + Sync>;

/// The person's GitHub account, as harness-hanger uses it.
pub struct Account {
    keychain: Box<dyn Keychain>,
    variables: Variables,
    /// The `gh` command, when it is installed.
    gh: Option<PathBuf>,
    connect: Connect,
    client_id: Option<String>,
}

impl Account {
    /// The account on this machine: the environment, the system keychain, `gh`, and the network.
    pub fn system() -> Self {
        Self {
            keychain: Box::new(SystemKeychain),
            variables: Box::new(|name| env::var(name).ok()),
            gh: find_gh(),
            connect: Box::new(GitHub::over_network),
            client_id: Some(CLIENT_ID)
                .filter(|id| !id.is_empty())
                .map(str::to_owned),
        }
    }

    /// Which account harness-hanger asks GitHub as, without asking GitHub.
    pub fn status(&self) -> AccountStatus {
        let (saved, keychain_error) = match self.keychain.get() {
            Ok(saved) => (saved.map(Saved::parse), None),
            Err(error) => (None, Some(error)),
        };
        let gh = self.gh.as_deref().map(|gh| GhStatus {
            signed_in: gh_token(gh).is_some(),
            login: gh_login(gh),
        });
        let (source, login) = if let Some((variable, _)) = self.env_token() {
            (Some(TokenSource::Env { variable }), None)
        } else if let Some(saved) = &saved {
            (Some(TokenSource::Saved), saved.login.clone())
        } else if let Some(gh) = gh.as_ref().filter(|gh| gh.signed_in) {
            (Some(TokenSource::Gh), gh.login.clone())
        } else {
            (None, None)
        };
        AccountStatus {
            source,
            login,
            saved: saved.is_some(),
            saved_login: saved.and_then(|saved| saved.login),
            gh,
            browser_sign_in: self.client_id.is_some(),
            keychain_error,
        }
    }

    /// The GitHub API, asked with the token in use.
    pub(crate) fn github(&self) -> GitHub {
        match self.credentials() {
            Some((token, login)) => (self.connect)(Some(token)).with_login(login),
            None => (self.connect)(None),
        }
    }

    /// Checks `token` with GitHub, and saves it in the keychain with the account it belongs to.
    pub fn save_token(&self, token: &str) -> Result<AccountStatus> {
        let token = token.trim();
        if token.is_empty() {
            return Err(Error::GitHub("the token is empty".to_owned()));
        }
        let login =
            (self.connect)(Some(token.to_owned()))
                .user()
                .map_err(|problem| match problem {
                    Problem::Unauthorized { message } => {
                        Error::GitHub(format!("GitHub refused the token: {message}"))
                    }
                    problem => Error::GitHub(problem.to_string()),
                })?;
        self.save(token, &login)?;
        Ok(self.status())
    }

    /// Forgets the saved token. A token from the environment or from `gh` stays in use.
    pub fn sign_out(&self) -> Result<AccountStatus> {
        self.keychain.delete().map_err(Error::Keychain)?;
        Ok(self.status())
    }

    /// Asks GitHub which account the token in use belongs to.
    pub fn check(&self) -> Result<String> {
        let token = self
            .token()
            .ok_or_else(|| Error::GitHub("harness-hanger is not signed in to GitHub".to_owned()))?;
        (self.connect)(Some(token))
            .user()
            .map_err(|problem| Error::GitHub(problem.to_string()))
    }

    /// Starts signing in in the browser. The person enters the code it returns at its
    /// `verification_uri`, and [`poll_browser_sign_in`](Self::poll_browser_sign_in) asks
    /// whether they did.
    pub fn start_browser_sign_in(&self) -> Result<DeviceCode> {
        let client_id = self.client_id()?;
        (self.connect)(None)
            .device_code(client_id)
            .map_err(|problem| match problem {
                Problem::GitHub { status: 404, .. } => Error::GitHub(format!(
                    "GitHub does not know the OAuth App that this build signs in with (client ID {client_id})"
                )),
                problem => Error::GitHub(problem.to_string()),
            })
    }

    /// Asks whether the person entered the code of `device_code`, and saves the token once they
    /// did.
    pub fn poll_browser_sign_in(&self, device_code: &str, interval: u64) -> Result<DevicePoll> {
        let client_id = self.client_id()?;
        let github = (self.connect)(None);
        let answer = github
            .device_token(client_id, device_code)
            .map_err(|problem| Error::GitHub(problem.to_string()))?;
        let token = match answer {
            DeviceAnswer::Token(token) => token,
            DeviceAnswer::Pending => return Ok(DevicePoll::Pending { interval }),
            DeviceAnswer::SlowDown { interval } => return Ok(DevicePoll::Pending { interval }),
            DeviceAnswer::Expired => {
                return Err(Error::GitHub(
                    "the code expired before it was entered; sign in again".to_owned(),
                ));
            }
            DeviceAnswer::Denied => {
                return Err(Error::GitHub(
                    "the sign-in was refused on GitHub".to_owned(),
                ));
            }
        };
        let login = (self.connect)(Some(token.clone()))
            .user()
            .map_err(|problem| Error::GitHub(problem.to_string()))?;
        self.save(&token, &login)?;
        Ok(DevicePoll::Done {
            status: self.status(),
        })
    }

    /// Starts `gh auth login` in the browser, and returns once `gh` prints its one-time code.
    ///
    /// This signs `gh` itself in, which harness-hanger then uses. It does not change how git
    /// talks to GitHub.
    pub fn start_gh_sign_in(&self) -> Result<GhSignIn> {
        let gh = self
            .gh
            .as_deref()
            .ok_or_else(|| Error::GitHub("the gh command is not installed".to_owned()))?;
        let mut child = process::background(gh)
            .args([
                "auth",
                "login",
                "--hostname",
                "github.com",
                "--web",
                "--skip-ssh-key",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| Error::GitHub(format!("cannot run gh: {error}")))?;
        let output = Arc::new(Mutex::new(String::new()));
        let (code_sender, code) = mpsc::channel();
        if let Some(stderr) = child.stderr.take() {
            let output = Arc::clone(&output);
            // Reads to the end, so that `gh` never blocks on a full pipe.
            thread::spawn(move || {
                for line in BufReader::new(stderr)
                    .lines()
                    .map_while(std::io::Result::ok)
                {
                    if let Some(found) = one_time_code(&line) {
                        let _ = code_sender.send(found.to_owned());
                    }
                    let mut output = output.lock().unwrap_or_else(PoisonError::into_inner);
                    output.push_str(&line);
                    output.push('\n');
                }
            });
        }
        let mut sign_in = GhSignIn {
            child,
            code: String::new(),
            output,
        };
        match code.recv_timeout(GH_CODE_TIMEOUT) {
            Ok(code) => {
                sign_in.code = code;
                Ok(sign_in)
            }
            // Dropping the sign-in stops `gh`.
            Err(_) => Err(Error::GitHub(format!(
                "gh did not start signing in: {}",
                sign_in.output()
            ))),
        }
    }

    fn client_id(&self) -> Result<&str> {
        self.client_id.as_deref().ok_or_else(|| {
            Error::GitHub(
                "this build cannot sign in in the browser; paste a token, or sign in with gh"
                    .to_owned(),
            )
        })
    }

    /// The token in use: from the environment, else the saved one, else that of `gh`.
    pub(crate) fn token(&self) -> Option<String> {
        self.credentials().map(|(token, _)| token)
    }

    /// The token in use, and the account it belongs to when that is known without asking
    /// GitHub.
    fn credentials(&self) -> Option<(String, Option<String>)> {
        if let Some((_, token)) = self.env_token() {
            return Some((token, None));
        }
        if let Ok(Some(secret)) = self.keychain.get() {
            let saved = Saved::parse(secret);
            return Some((saved.token, saved.login));
        }
        let gh = self.gh.as_deref()?;
        Some((gh_token(gh)?, gh_login(gh)))
    }

    /// The first token variable that is set, and its value.
    fn env_token(&self) -> Option<(String, String)> {
        TOKEN_VARIABLES.into_iter().find_map(|name| {
            let value = (self.variables)(name)?.trim().to_owned();
            (!value.is_empty()).then(|| (name.to_owned(), value))
        })
    }

    fn save(&self, token: &str, login: &str) -> Result<()> {
        let saved = Saved {
            token: token.to_owned(),
            login: Some(login.to_owned()),
        };
        let secret =
            serde_json::to_string(&saved).map_err(|error| Error::Keychain(error.to_string()))?;
        self.keychain.set(&secret).map_err(Error::Keychain)
    }
}

/// A sign-in that `gh auth login` runs. Dropping it stops `gh`.
pub struct GhSignIn {
    child: Child,
    code: String,
    /// What `gh` printed, to explain a failure.
    output: Arc<Mutex<String>>,
}

impl GhSignIn {
    /// The one-time code that `gh` printed, and where to enter it.
    #[must_use]
    pub fn prompt(&self) -> SignInPrompt {
        SignInPrompt {
            user_code: self.code.clone(),
            verification_uri: "https://github.com/login/device".to_owned(),
        }
    }

    /// Whether `gh` finished signing in. An error says why it failed.
    pub fn poll(&mut self) -> Result<bool> {
        match self.child.try_wait() {
            Ok(None) => Ok(false),
            Ok(Some(status)) if status.success() => Ok(true),
            Ok(Some(_)) => Err(Error::GitHub(format!(
                "gh could not sign in: {}",
                self.output()
            ))),
            Err(error) => Err(Error::GitHub(format!("cannot wait for gh: {error}"))),
        }
    }

    fn output(&self) -> String {
        self.output
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .trim()
            .to_owned()
    }
}

impl Drop for GhSignIn {
    fn drop(&mut self) {
        // Fails, as intended, once `gh` has exited.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The one-time code in a line that `gh` prints, such as `ABCD-1234`.
fn one_time_code(line: &str) -> Option<&str> {
    line.split(|c: char| !c.is_ascii_alphanumeric() && c != '-')
        .find(|word| {
            let bytes = word.as_bytes();
            bytes.len() == 9
                && bytes[4] == b'-'
                && bytes
                    .iter()
                    .enumerate()
                    .all(|(i, b)| i == 4 || b.is_ascii_uppercase() || b.is_ascii_digit())
        })
}

/// The `gh` command, also where it is installed outside `PATH`.
fn find_gh() -> Option<PathBuf> {
    let names: &[&str] = if cfg!(windows) { &["gh.exe"] } else { &["gh"] };
    let extra = env::var_os("ProgramFiles")
        .filter(|_| cfg!(windows))
        .map(|dir| vec![PathBuf::from(dir).join("GitHub CLI")])
        .unwrap_or_default();
    process::find(names, env::var_os("PATH"), extra)
}

/// The token of the login of `gh` on github.com, read without network access.
fn gh_token(gh: &std::path::Path) -> Option<String> {
    gh_output(gh, &["auth", "token", "--hostname", "github.com"])
}

/// The account that `gh` is signed in as on github.com, from its configuration.
fn gh_login(gh: &std::path::Path) -> Option<String> {
    gh_output(gh, &["config", "get", "user", "--host", "github.com"])
}

fn gh_output(gh: &std::path::Path, args: &[&str]) -> Option<String> {
    let output = process::background(gh)
        .args(args)
        .env("GH_PROMPT_DISABLED", "1")
        .stdin(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::Value;

    use super::super::fake::Fake;
    use super::*;

    const USER: &str = "https://api.github.com/user";
    const DEVICE_CODE: &str = "https://github.com/login/device/code";
    const DEVICE_TOKEN: &str = "https://github.com/login/oauth/access_token";

    /// A keychain in memory, which can also fail as a locked or missing one does.
    #[derive(Clone, Default)]
    struct MemoryKeychain {
        secret: Arc<Mutex<Option<String>>>,
        broken: bool,
    }

    impl MemoryKeychain {
        fn secret(&self) -> Option<String> {
            self.secret.lock().unwrap().clone()
        }
    }

    impl Keychain for MemoryKeychain {
        fn get(&self) -> Result<Option<String>, String> {
            if self.broken {
                return Err("no keychain".to_owned());
            }
            Ok(self.secret())
        }

        fn set(&self, secret: &str) -> Result<(), String> {
            *self.secret.lock().unwrap() = Some(secret.to_owned());
            Ok(())
        }

        fn delete(&self) -> Result<(), String> {
            *self.secret.lock().unwrap() = None;
            Ok(())
        }
    }

    struct Setup {
        fake: Fake,
        keychain: MemoryKeychain,
    }

    impl Setup {
        fn new() -> Self {
            let fake = Fake::default();
            fake.on(USER, 200, r#"{ "login": "octocat" }"#);
            Self {
                fake,
                keychain: MemoryKeychain::default(),
            }
        }

        fn account(&self, variables: &'static [(&'static str, &'static str)]) -> Account {
            let fake = self.fake.clone();
            Account {
                keychain: Box::new(self.keychain.clone()),
                variables: Box::new(move |name| {
                    variables
                        .iter()
                        .find(|(key, _)| *key == name)
                        .map(|(_, value)| (*value).to_owned())
                }),
                gh: None,
                connect: Box::new(move |token| fake.github(token.as_deref())),
                client_id: Some("client-id".to_owned()),
            }
        }
    }

    #[test]
    fn saves_a_token_that_github_accepts() {
        let setup = Setup::new();
        let account = setup.account(&[]);

        let status = account.save_token("  ghp_secret \n").unwrap();

        assert_eq!(status.source, Some(TokenSource::Saved));
        assert_eq!(status.login.as_deref(), Some("octocat"));
        assert!(status.saved);
        let saved: Value = serde_json::from_str(&setup.keychain.secret().unwrap()).unwrap();
        assert_eq!(saved["token"], "ghp_secret");
        assert_eq!(saved["login"], "octocat");
        assert!(setup.fake.sent()[0].authorized);
        assert_eq!(account.token().as_deref(), Some("ghp_secret"));
        assert_eq!(account.github().login(), Some("octocat"));
    }

    #[test]
    fn refuses_a_token_that_github_refuses() {
        let setup = Setup::new();
        setup
            .fake
            .on(USER, 401, r#"{ "message": "Bad credentials" }"#);

        let error = setup.account(&[]).save_token("wrong").unwrap_err();

        assert_eq!(
            error.to_string(),
            "GitHub: GitHub refused the token: Bad credentials"
        );
        assert_eq!(setup.keychain.secret(), None);
        assert!(setup.account(&[]).save_token("   ").is_err());
    }

    #[test]
    fn prefers_the_environment_to_the_saved_token() {
        let setup = Setup::new();
        setup.account(&[]).save_token("saved").unwrap();

        let account = setup.account(&[("GH_TOKEN", " "), ("GITHUB_TOKEN", "from-env")]);

        assert_eq!(account.token().as_deref(), Some("from-env"));
        let status = account.status();
        assert_eq!(
            status.source,
            Some(TokenSource::Env {
                variable: "GITHUB_TOKEN".to_owned()
            })
        );
        assert_eq!(status.login, None);
        // The saved token is still there, under the one in use.
        assert_eq!(status.saved_login.as_deref(), Some("octocat"));
    }

    #[test]
    fn signs_out_by_forgetting_the_saved_token() {
        let setup = Setup::new();
        let account = setup.account(&[]);
        account.save_token("saved").unwrap();

        let status = account.sign_out().unwrap();

        assert_eq!(status.source, None);
        assert!(!status.saved);
        assert_eq!(account.token(), None);
        assert_eq!(setup.keychain.secret(), None);
    }

    #[test]
    fn reads_a_bare_token_from_the_keychain() {
        let setup = Setup::new();
        setup.keychain.set("bare-token").unwrap();

        let account = setup.account(&[]);

        assert_eq!(account.token().as_deref(), Some("bare-token"));
        assert_eq!(account.status().login, None);
    }

    #[test]
    fn says_why_the_keychain_cannot_be_read() {
        let mut setup = Setup::new();
        setup.keychain.broken = true;

        let status = setup.account(&[]).status();

        assert_eq!(status.keychain_error.as_deref(), Some("no keychain"));
        assert_eq!(status.source, None);
    }

    #[test]
    fn checks_the_token_in_use_with_github() {
        let setup = Setup::new();

        assert_eq!(
            setup.account(&[("GH_TOKEN", "t")]).check().unwrap(),
            "octocat"
        );
        assert!(setup.account(&[]).check().is_err());
    }

    #[test]
    fn signs_in_in_the_browser() {
        let setup = Setup::new();
        setup.fake.on(
            DEVICE_CODE,
            200,
            r#"{ "device_code": "dc", "user_code": "ABCD-1234", "verification_uri": "https://github.com/login/device", "expires_in": 899, "interval": 5 }"#,
        );
        setup
            .fake
            .on(DEVICE_TOKEN, 200, r#"{ "error": "authorization_pending" }"#);
        let account = setup.account(&[]);

        let code = account.start_browser_sign_in().unwrap();
        assert_eq!(code.user_code, "ABCD-1234");
        assert_eq!(code.interval, 5);
        assert_eq!(
            account.poll_browser_sign_in(&code.device_code, 5).unwrap(),
            DevicePoll::Pending { interval: 5 }
        );

        setup.fake.on(
            DEVICE_TOKEN,
            200,
            r#"{ "error": "slow_down", "interval": 10 }"#,
        );
        assert_eq!(
            account.poll_browser_sign_in("dc", 5).unwrap(),
            DevicePoll::Pending { interval: 10 }
        );

        setup.fake.on(
            DEVICE_TOKEN,
            200,
            r#"{ "access_token": "gho_new", "token_type": "bearer", "scope": "" }"#,
        );
        let DevicePoll::Done { status } = account.poll_browser_sign_in("dc", 10).unwrap() else {
            panic!("expected the sign-in to be done");
        };
        assert_eq!(status.login.as_deref(), Some("octocat"));
        assert_eq!(account.token().as_deref(), Some("gho_new"));

        let sent = setup.fake.sent();
        let first: Value = serde_json::from_str(sent[0].body.as_deref().unwrap()).unwrap();
        assert_eq!(first, serde_json::json!({ "client_id": "client-id" }));
        // The OAuth endpoints get no token.
        assert!(
            sent.iter()
                .filter(|s| !s.url.starts_with(USER))
                .all(|s| !s.authorized)
        );
    }

    #[test]
    fn says_why_a_browser_sign_in_stopped() {
        let setup = Setup::new();
        let account = setup.account(&[]);

        setup
            .fake
            .on(DEVICE_TOKEN, 200, r#"{ "error": "expired_token" }"#);
        assert!(
            account
                .poll_browser_sign_in("dc", 5)
                .unwrap_err()
                .to_string()
                .contains("expired")
        );

        setup
            .fake
            .on(DEVICE_TOKEN, 200, r#"{ "error": "access_denied" }"#);
        assert!(
            account
                .poll_browser_sign_in("dc", 5)
                .unwrap_err()
                .to_string()
                .contains("refused")
        );

        setup.fake.on(
            DEVICE_CODE,
            200,
            r#"{ "error": "device_flow_disabled", "error_description": "Device Flow must be explicitly enabled for this App" }"#,
        );
        assert!(
            account
                .start_browser_sign_in()
                .unwrap_err()
                .to_string()
                .contains("Device Flow must be")
        );

        setup
            .fake
            .on(DEVICE_CODE, 404, r#"{ "error": "Not Found" }"#);
        assert!(
            account
                .start_browser_sign_in()
                .unwrap_err()
                .to_string()
                .contains("does not know the OAuth App")
        );

        let mut without_app = setup.account(&[]);
        without_app.client_id = None;
        assert!(!without_app.status().browser_sign_in);
        assert!(
            without_app
                .start_browser_sign_in()
                .unwrap_err()
                .to_string()
                .contains("paste a token")
        );
    }

    #[test]
    fn finds_the_one_time_code_that_gh_prints() {
        assert_eq!(
            one_time_code("! One-time code (3D9A-0A02) copied to clipboard"),
            Some("3D9A-0A02")
        );
        assert_eq!(
            one_time_code("! First copy your one-time code: ABCD-1234"),
            Some("ABCD-1234")
        );
        assert_eq!(
            one_time_code("Open this URL to continue: https://github.com/login/device"),
            None
        );
        assert_eq!(one_time_code("Logged in as some-user-name"), None);
    }

    #[cfg(unix)]
    mod gh {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        use super::*;

        /// A fake `gh` that runs `script` for `auth login`, and answers `auth token` and
        /// `config get user` as one signed in as octocat.
        fn fake_gh(dir: &std::path::Path, login_script: &str) -> PathBuf {
            let gh = dir.join("gh");
            fs::write(
                &gh,
                format!(
                    "#!/bin/sh\ncase \"$1 $2\" in\n  'auth token') echo gho_from_gh ;;\n  'config get') echo octocat ;;\n  'auth login') {login_script} ;;\nesac\n"
                ),
            )
            .unwrap();
            fs::set_permissions(&gh, fs::Permissions::from_mode(0o755)).unwrap();
            gh
        }

        #[test]
        fn uses_the_login_of_gh() {
            let tmp = tempfile::tempdir().unwrap();
            let setup = Setup::new();
            let mut account = setup.account(&[]);
            account.gh = Some(fake_gh(tmp.path(), "exit 1"));

            let status = account.status();

            assert_eq!(status.source, Some(TokenSource::Gh));
            assert_eq!(status.login.as_deref(), Some("octocat"));
            assert_eq!(
                status.gh,
                Some(GhStatus {
                    signed_in: true,
                    login: Some("octocat".to_owned())
                })
            );
            assert_eq!(account.token().as_deref(), Some("gho_from_gh"));
        }

        #[test]
        fn signs_in_through_gh() {
            let tmp = tempfile::tempdir().unwrap();
            let setup = Setup::new();
            let mut account = setup.account(&[]);
            account.gh = Some(fake_gh(
                tmp.path(),
                "echo '! One-time code (ABCD-1234) copied to clipboard' >&2; sleep 0.3; echo '✓ Logged in' >&2",
            ));

            let mut sign_in = account.start_gh_sign_in().unwrap();

            assert_eq!(sign_in.prompt().user_code, "ABCD-1234");
            let mut done = false;
            for _ in 0..50 {
                done = sign_in.poll().unwrap();
                if done {
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }
            assert!(done);
        }

        #[test]
        fn says_why_gh_could_not_sign_in() {
            let tmp = tempfile::tempdir().unwrap();
            let setup = Setup::new();
            let mut account = setup.account(&[]);
            account.gh = Some(fake_gh(
                tmp.path(),
                "echo '! One-time code (ABCD-1234) copied to clipboard' >&2; echo 'error: code expired' >&2; exit 1",
            ));

            let mut sign_in = account.start_gh_sign_in().unwrap();
            let mut result = sign_in.poll();
            for _ in 0..50 {
                if !matches!(result, Ok(false)) {
                    break;
                }
                thread::sleep(Duration::from_millis(50));
                result = sign_in.poll();
            }

            assert!(result.unwrap_err().to_string().contains("code expired"));
        }

        #[test]
        fn explains_a_gh_that_prints_no_code() {
            let tmp = tempfile::tempdir().unwrap();
            let setup = Setup::new();
            let mut account = setup.account(&[]);
            account.gh = Some(fake_gh(
                tmp.path(),
                "echo 'unknown flag: --skip-ssh-key' >&2; exit 1",
            ));

            let error = account.start_gh_sign_in().err().unwrap();

            assert!(error.to_string().contains("unknown flag"));
        }
    }
}
