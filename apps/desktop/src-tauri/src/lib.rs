use std::sync::{Mutex, PoisonError};

use hanger_core::github::{Account, AccountStatus, DeviceCode, DevicePoll, GhSignIn, SignInPrompt};
use hanger_core::harness::claude_code::{ClaudeCode, ScopeState, UserPlugins};
use hanger_core::harness::{Discovery, ImportItem};
use hanger_core::marketplace::discover::{Discover, DiscoverResults, Fetch};
use hanger_core::marketplace::{
    Catalog, CatalogPlugins, ComponentDetails, ComponentKind, MarketplaceDetails,
    MarketplaceSource, MarketplaceSummary, PluginDetails,
};
use hanger_core::projects::{Project, Projects};
use hanger_core::{AppInfo, paths};

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo::collect()
}

#[tauri::command]
async fn list_marketplaces() -> Result<Vec<MarketplaceSummary>, String> {
    with_catalog(Catalog::list).await
}

/// `source` is what the person typed: a GitHub URL, `owner/repo`, or a local directory path.
#[tauri::command]
async fn add_marketplace(source: String) -> Result<MarketplaceSummary, String> {
    with_catalog(move |catalog| catalog.add(&MarketplaceSource::parse(&source)?)).await
}

#[tauri::command]
async fn marketplace_details(id: i64) -> Result<MarketplaceDetails, String> {
    with_catalog(move |catalog| catalog.details(id)).await
}

/// Every plugin in the catalog, with its status in Claude Code for the user.
#[tauri::command]
async fn list_catalog_plugins() -> Result<CatalogPlugins, String> {
    with_catalog(|catalog| catalog.plugins(&UserPlugins::read(&paths::claude_config_dir()?)?)).await
}

/// A plugin of the marketplace `marketplace`, with its status in Claude Code and its README.
#[tauri::command]
async fn plugin_details(marketplace: i64, plugin: String) -> Result<PluginDetails, String> {
    with_catalog(move |catalog| {
        catalog.plugin_details(
            marketplace,
            &plugin,
            &UserPlugins::read(&paths::claude_config_dir()?)?,
        )
    })
    .await
}

/// A skill, an agent, or a command of a plugin, with the file that defines it.
#[tauri::command]
async fn component_details(
    marketplace: i64,
    plugin: String,
    kind: ComponentKind,
    name: String,
) -> Result<ComponentDetails, String> {
    with_catalog(move |catalog| {
        catalog.component_details(
            marketplace,
            &plugin,
            kind,
            &name,
            &UserPlugins::read(&paths::claude_config_dir()?)?,
        )
    })
    .await
}

/// What the harnesses have that the catalog can import.
#[tauri::command]
async fn import_candidates() -> Result<Vec<Discovery>, String> {
    with_catalog(Catalog::import_candidates).await
}

/// Imports one thing from the harness `harness`. A clone can take a while.
#[tauri::command]
async fn import_from_harness(
    harness: String,
    item: ImportItem,
) -> Result<MarketplaceSummary, String> {
    with_catalog(move |catalog| catalog.import(&harness, &item)).await
}

#[tauri::command]
async fn list_projects() -> Result<Vec<Project>, String> {
    blocking(|| Projects::open_default()?.list()).await
}

/// `path` is a folder path, which can start with `~`.
#[tauri::command]
async fn add_project(path: String) -> Result<Project, String> {
    blocking(move || Projects::open_default()?.add(&path)).await
}

#[tauri::command]
async fn remove_project(id: i64) -> Result<(), String> {
    blocking(move || Projects::open_default()?.remove(id)).await
}

/// What applies in a project, or globally when `project` is `None`.
#[tauri::command]
async fn scope_state(project: Option<i64>) -> Result<ScopeState, String> {
    blocking(move || {
        let path = project_path(project)?;
        ClaudeCode::from_env().scope_state(path.as_deref())
    })
    .await
}

/// Switches `plugin` (`plugin@marketplace`) on or off in a project, or for the user when
/// `project` is `None`.
#[tauri::command]
async fn set_plugin_enabled(
    project: Option<i64>,
    plugin: String,
    enabled: bool,
) -> Result<(), String> {
    blocking(move || {
        let claude = ClaudeCode::from_env();
        match project_path(project)? {
            Some(path) => claude.set_enabled(&path, &plugin, enabled),
            None => claude.set_enabled_for_user(&plugin, enabled),
        }
    })
    .await
}

/// Installs `plugin` from the catalog marketplace `marketplace` into a project.
#[tauri::command]
async fn install_plugin(project: i64, marketplace: i64, plugin: String) -> Result<(), String> {
    blocking(move || {
        let path = Projects::open_default()?.get(project)?.path;
        let summary = Catalog::open_default()?.get(marketplace)?;
        let Some(name) = summary.name else {
            return Err(hanger_core::Error::Harness(format!(
                "cannot read the marketplace: {}",
                summary.error.unwrap_or_default()
            )));
        };
        ClaudeCode::from_env()
            .install_in_project(&path, &plugin, &name, &summary.source)
            .map(drop)
    })
    .await
}

/// Uninstalls `plugin` (`plugin@marketplace`) from a project.
#[tauri::command]
async fn uninstall_plugin(project: i64, plugin: String) -> Result<(), String> {
    blocking(move || {
        let path = Projects::open_default()?.get(project)?.path;
        ClaudeCode::from_env().uninstall_from_project(&path, &plugin)
    })
    .await
}

fn project_path(project: Option<i64>) -> hanger_core::Result<Option<std::path::PathBuf>> {
    project
        .map(|id| Ok(Projects::open_default()?.get(id)?.path))
        .transpose()
}

/// Public marketplaces on GitHub that match `query`, or the curated ones when it is empty.
/// GitHub is asked unless the same search ran within the hour, or always with `refresh`.
#[tauri::command]
async fn discover_marketplaces(query: String, refresh: bool) -> Result<DiscoverResults, String> {
    let fetch = if refresh {
        Fetch::Refresh
    } else {
        Fetch::WhenStale
    };
    blocking(move || Discover::open_default()?.search(&query, fetch)).await
}

/// The `gh auth login` that Settings started, while it runs.
#[derive(Default)]
struct GhSignInState(Mutex<Option<GhSignIn>>);

/// Which GitHub account harness-hanger uses. Asks GitHub nothing.
#[tauri::command]
async fn github_status() -> Result<AccountStatus, String> {
    blocking(|| Ok(Account::system().status())).await
}

/// Checks `token` with GitHub and saves it in the system keychain.
#[tauri::command]
async fn github_save_token(token: String) -> Result<AccountStatus, String> {
    blocking(move || Account::system().save_token(&token)).await
}

/// Forgets the token saved in the system keychain.
#[tauri::command]
async fn github_sign_out() -> Result<AccountStatus, String> {
    blocking(|| Account::system().sign_out()).await
}

/// Asks GitHub which account the token in use belongs to.
#[tauri::command]
async fn github_check() -> Result<String, String> {
    blocking(|| Account::system().check()).await
}

#[tauri::command]
async fn github_start_browser_sign_in() -> Result<DeviceCode, String> {
    blocking(|| Account::system().start_browser_sign_in()).await
}

/// Asks whether the person entered the code yet; the token is saved once they did.
#[tauri::command]
async fn github_poll_browser_sign_in(
    device_code: String,
    interval: u64,
) -> Result<DevicePoll, String> {
    blocking(move || Account::system().poll_browser_sign_in(&device_code, interval)).await
}

/// Starts `gh auth login`, and returns the code to enter. A new sign-in stops the one before.
#[tauri::command]
async fn github_start_gh_sign_in(
    state: tauri::State<'_, GhSignInState>,
) -> Result<SignInPrompt, String> {
    let sign_in = blocking(|| Account::system().start_gh_sign_in()).await?;
    let prompt = sign_in.prompt();
    *state.0.lock().unwrap_or_else(PoisonError::into_inner) = Some(sign_in);
    Ok(prompt)
}

/// The account once `gh` finished signing in, or `None` while it waits.
#[tauri::command]
async fn github_poll_gh_sign_in(
    state: tauri::State<'_, GhSignInState>,
) -> Result<Option<AccountStatus>, String> {
    {
        let mut running = state.0.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(sign_in) = running.as_mut() else {
            return Err("no sign-in with gh is running".to_owned());
        };
        match sign_in.poll() {
            Ok(false) => return Ok(None),
            Ok(true) => *running = None,
            Err(error) => {
                *running = None;
                return Err(error.to_string());
            }
        }
    }
    blocking(|| Ok(Account::system().status())).await.map(Some)
}

/// Stops the `gh auth login` that Settings started, if it still runs.
#[tauri::command]
async fn github_cancel_gh_sign_in(state: tauri::State<'_, GhSignInState>) -> Result<(), String> {
    let running = state
        .0
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take();
    // Stopping `gh` waits for it to exit, so it happens off the main thread.
    blocking(move || {
        drop(running);
        Ok(())
    })
    .await
}

#[tauri::command]
async fn remove_marketplace(id: i64) -> Result<(), String> {
    with_catalog(move |catalog| catalog.remove(id)).await
}

/// Runs catalog work, which reads files and may run git, on a blocking thread.
async fn with_catalog<T: Send + 'static>(
    work: impl FnOnce(&Catalog) -> hanger_core::Result<T> + Send + 'static,
) -> Result<T, String> {
    blocking(move || work(&Catalog::open_default()?)).await
}

/// Runs work that reads files or runs `git` or `claude` on a blocking thread.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> hanger_core::Result<T> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
}

/// Starts the desktop app.
///
/// # Panics
///
/// Panics if the Tauri runtime fails to start.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(GhSignInState::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            list_marketplaces,
            add_marketplace,
            marketplace_details,
            list_catalog_plugins,
            plugin_details,
            component_details,
            import_candidates,
            import_from_harness,
            list_projects,
            add_project,
            remove_project,
            scope_state,
            set_plugin_enabled,
            install_plugin,
            uninstall_plugin,
            discover_marketplaces,
            github_status,
            github_save_token,
            github_sign_out,
            github_check,
            github_start_browser_sign_in,
            github_poll_browser_sign_in,
            github_start_gh_sign_in,
            github_poll_gh_sign_in,
            github_cancel_gh_sign_in,
            remove_marketplace,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run the harness-hanger desktop app");
}
