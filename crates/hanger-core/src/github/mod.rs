//! GitHub: the API that harness-hanger asks, and the account that it asks as.

mod account;
mod client;

pub use account::{
    Account, AccountStatus, DevicePoll, GhSignIn, GhStatus, SignInPrompt, TokenSource,
};
#[cfg(test)]
pub(crate) use client::fake;
pub use client::{DeviceCode, Problem, RateLimit, TOPIC};
pub(crate) use client::{GitHub, Partial, RepoInfo, RepoRef, in_parallel, successes};
