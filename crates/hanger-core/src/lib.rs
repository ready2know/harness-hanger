//! Core library of harness-hanger.
//!
//! Every capability lives here. The CLI (`hanger-cli`) and the desktop app
//! (`apps/desktop/src-tauri`) are thin adapters over this crate.

pub mod claude_code;
mod error;
mod frontmatter;
mod git;
mod info;
pub mod marketplace;
pub mod paths;
mod store;

pub use error::{Error, Result};
pub use info::AppInfo;

/// Version shared by the core and every front-end.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
