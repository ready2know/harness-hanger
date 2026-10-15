use hanger_core::harness::claude_code::{ClaudeCode, ScopeState, UserPlugins};
use hanger_core::harness::{Discovery, ImportItem};
use hanger_core::marketplace::{
    Catalog, CatalogPlugins, MarketplaceDetails, MarketplaceSource, MarketplaceSummary,
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

/// Switches `plugin` (`plugin@marketplace`) on or off in a project.
#[tauri::command]
async fn set_plugin_enabled(project: i64, plugin: String, enabled: bool) -> Result<(), String> {
    blocking(move || {
        let path = Projects::open_default()?.get(project)?.path;
        ClaudeCode::from_env().set_enabled(&path, &plugin, enabled)
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
        .plugin(tauri_plugin_dialog::init())
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
            import_candidates,
            import_from_harness,
            list_projects,
            add_project,
            remove_project,
            scope_state,
            set_plugin_enabled,
            install_plugin,
            uninstall_plugin,
            remove_marketplace,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run the harness-hanger desktop app");
}
