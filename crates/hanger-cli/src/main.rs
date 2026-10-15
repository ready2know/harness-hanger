use anyhow::{Result, anyhow, bail};
use clap::{Parser, Subcommand};
use hanger_core::harness::claude_code::UserPlugins;
use hanger_core::harness::{Discovery, ImportItem, PluginStatus};
use hanger_core::marketplace::{
    Catalog, CatalogPlugins, Component, MarketplaceDetails, MarketplaceSource, MarketplaceSummary,
    PluginSource,
};
use hanger_core::{AppInfo, paths};
use serde::Serialize;

/// Manage AI harness components: skills, agents, plugins, marketplaces, hooks and MCP servers.
#[derive(Debug, Parser)]
#[command(name = "harness-hanger", version, about)]
struct Cli {
    /// Print machine-readable JSON instead of human-readable text.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Show the version and where harness configuration is read from.
    Info,
    /// Browse plugin marketplaces in harness-hanger's own catalog. Claude Code does not see them.
    #[command(subcommand)]
    Marketplace(MarketplaceCommand),
    /// Import marketplaces, with their plugins, and standalone skills from a harness, such as
    /// Claude Code. With nothing named to import, lists what can be imported.
    Import {
        /// The harness, such as `claude-code`. Every harness is listed when it is left out.
        harness: Option<String>,
        /// Marketplaces to import, by the name the harness knows them by.
        marketplaces: Vec<String>,
        /// Import the skills, agents, and commands that the harness loads outside plugins.
        #[arg(long)]
        standalone: bool,
        /// Import everything that can be imported and is not in the catalog yet.
        #[arg(long, conflicts_with_all = ["marketplaces", "standalone"])]
        all: bool,
    },
}

#[derive(Debug, Subcommand)]
enum MarketplaceCommand {
    /// Add a marketplace from GitHub (a URL or owner/repo) or from a local directory.
    Add {
        /// For example `https://github.com/owner/repo`, `owner/repo`, or `~/src/my-marketplace`.
        source: String,
    },
    /// List the marketplaces in the catalog.
    Ls,
    /// Show the plugins, skills, agents, and commands that a marketplace contains.
    Show {
        /// The ID that `marketplace ls` shows.
        id: i64,
    },
    /// List the plugins of every marketplace in the catalog, and whether Claude Code has each one
    /// enabled for the user.
    Plugins,
    /// Remove a marketplace from the catalog. A local directory is not deleted.
    Rm {
        /// The ID that `marketplace ls` shows.
        id: i64,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Info => info(cli.json),
        Command::Marketplace(command) => marketplace(command, cli.json),
        Command::Import {
            harness,
            marketplaces,
            standalone,
            all,
        } => import(harness, marketplaces, standalone, all, cli.json),
    }
}

fn import(
    harness: Option<String>,
    marketplaces: Vec<String>,
    standalone: bool,
    all: bool,
    json: bool,
) -> Result<()> {
    let catalog = Catalog::open_default()?;
    let discoveries = catalog.import_candidates()?;
    let known = || {
        discoveries
            .iter()
            .map(|discovery| discovery.harness)
            .collect::<Vec<_>>()
            .join(", ")
    };
    let find = |id: &str| {
        discoveries
            .iter()
            .find(|discovery| discovery.harness == id)
            .ok_or_else(|| anyhow!("unknown harness {id}; known harnesses: {}", known()))
    };

    if !all && !standalone && marketplaces.is_empty() {
        let shown = match &harness {
            Some(id) => vec![find(id)?],
            None => discoveries.iter().collect(),
        };
        if json {
            return print_json(&shown);
        }
        shown.into_iter().for_each(print_discovery);
        return Ok(());
    }

    let Some(harness) = harness else {
        bail!("name the harness to import from: {}", known());
    };
    let discovery = find(&harness)?;
    let mut items: Vec<ImportItem> = if all {
        discovery
            .marketplaces
            .iter()
            .filter(|found| found.source.is_some() && !found.in_catalog)
            .map(|found| ImportItem::Marketplace {
                name: found.name.clone(),
            })
            .collect()
    } else {
        marketplaces
            .into_iter()
            .map(|name| ImportItem::Marketplace { name })
            .collect()
    };
    let new_standalone = discovery
        .standalone
        .as_ref()
        .is_some_and(|found| !found.in_catalog);
    if standalone || (all && new_standalone) {
        items.push(ImportItem::Standalone);
    }
    if items.is_empty() && !json {
        println!("Nothing new to import from {}.", discovery.name);
    }

    let mut results = Vec::new();
    let mut failed = 0;
    for item in &items {
        let label = match item {
            ImportItem::Marketplace { name } => name.clone(),
            ImportItem::Standalone => "standalone skills, agents, and commands".to_owned(),
        };
        if !json {
            eprintln!("Importing {label}…");
        }
        match catalog.import(&harness, item) {
            Ok(summary) => {
                if !json {
                    println!("Imported {label} (ID {})", summary.id);
                }
                results.push(serde_json::json!({ "item": label, "imported": summary }));
            }
            Err(error) => {
                failed += 1;
                if !json {
                    println!("Not imported: {label}: {error}");
                }
                results.push(serde_json::json!({ "item": label, "error": error.to_string() }));
            }
        }
    }
    if json {
        print_json(&results)?;
    }
    if failed > 0 {
        bail!("{failed} of {} items were not imported", items.len());
    }
    Ok(())
}

fn print_discovery(discovery: &Discovery) {
    let Some(config_dir) = &discovery.config_dir else {
        println!(
            "{} ({}): not found on this machine",
            discovery.name, discovery.harness
        );
        return;
    };
    println!(
        "{} ({}), {}",
        discovery.name,
        discovery.harness,
        config_dir.display()
    );
    for found in &discovery.marketplaces {
        let state = match (&found.source, found.in_catalog) {
            (Some(_), true) => "in catalog",
            (Some(_), false) => "new",
            (None, _) => "cannot",
        };
        let detail = match (&found.source, &found.problem) {
            (Some(source), _) => source.to_string(),
            (None, Some(problem)) => problem.clone(),
            (None, None) => String::new(),
        };
        let installed = if found.installed_plugins.is_empty() {
            String::new()
        } else {
            format!(", installed: {}", found.installed_plugins.join(", "))
        };
        println!("  {state:<10}  {}  {detail}{installed}", found.name);
    }
    if let Some(found) = &discovery.standalone {
        let state = if found.in_catalog {
            "in catalog"
        } else {
            "new"
        };
        println!(
            "  {state:<10}  standalone: {} skills, {} agents, {} commands",
            found.skills, found.agents, found.commands
        );
    }
    if !discovery.unmatched_plugins.is_empty() {
        println!(
            "  cannot      plugins whose marketplace {} does not describe: {}",
            discovery.name,
            discovery.unmatched_plugins.join(", ")
        );
    }
}

fn info(json: bool) -> Result<()> {
    let info = AppInfo::collect();
    if json {
        return print_json(&info);
    }
    println!("harness-hanger {}", info.version);
    match &info.claude_config_dir {
        Some(dir) => println!("Claude Code config: {}", dir.display()),
        None => println!("Claude Code config: not found"),
    }
    Ok(())
}

fn marketplace(command: MarketplaceCommand, json: bool) -> Result<()> {
    let catalog = Catalog::open_default()?;
    match command {
        MarketplaceCommand::Add { source } => {
            let source = MarketplaceSource::parse(&source)?;
            if !json && matches!(source, MarketplaceSource::GitHub { .. }) {
                eprintln!("Cloning {source}…");
            }
            let added = catalog.add(&source)?;
            if json {
                return print_json(&added);
            }
            println!(
                "Added {} (ID {}): {} plugins",
                added.name.as_deref().unwrap_or("?"),
                added.id,
                added.plugin_count
            );
        }
        MarketplaceCommand::Ls => {
            let marketplaces = catalog.list()?;
            if json {
                return print_json(&marketplaces);
            }
            print_list(&marketplaces);
        }
        MarketplaceCommand::Show { id } => {
            let details = catalog.details(id)?;
            if json {
                return print_json(&details);
            }
            print_details(&details);
        }
        MarketplaceCommand::Plugins => {
            let claude = UserPlugins::read(&paths::claude_config_dir()?)?;
            let plugins = catalog.plugins(&claude)?;
            if json {
                return print_json(&plugins);
            }
            print_plugins(&plugins);
        }
        MarketplaceCommand::Rm { id } => {
            catalog.remove(id)?;
            if json {
                return print_json(&serde_json::json!({ "removed": id }));
            }
            println!("Removed marketplace {id}");
        }
    }
    Ok(())
}

fn print_json(value: &impl Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn print_list(marketplaces: &[MarketplaceSummary]) {
    if marketplaces.is_empty() {
        println!("No marketplaces. Add one with `harness-hanger marketplace add <source>`.");
        return;
    }
    for marketplace in marketplaces {
        let name = marketplace.name.as_deref().unwrap_or("(unreadable)");
        let contents = match marketplace.source {
            MarketplaceSource::Harness { .. } => "standalone".to_owned(),
            _ => format!("{} plugins", marketplace.plugin_count),
        };
        println!(
            "{:>4}  {name}  {contents}  {}",
            marketplace.id, marketplace.source
        );
        if let Some(error) = &marketplace.error {
            println!("      error: {error}");
        }
    }
}

fn print_details(details: &MarketplaceDetails) {
    let Some(preview) = &details.preview else {
        let error = details.summary.error.as_deref().unwrap_or("unknown error");
        println!(
            "Source: {}\nCannot read it: {error}",
            details.summary.source
        );
        return;
    };
    println!("{}", preview.name);
    if let Some(description) = &preview.description {
        println!("{description}");
    }
    println!("Source: {}", details.summary.source);
    if let Some(owner) = &preview.owner {
        println!("Owner: {owner}");
    }
    for plugin in &preview.plugins {
        println!();
        let version = plugin
            .version
            .as_deref()
            .map(|version| format!(" {version}"))
            .unwrap_or_default();
        match plugin.source {
            PluginSource::Harness { .. } => println!("standalone"),
            _ => println!("{}{version}", plugin.name),
        }
        if let Some(description) = &plugin.description {
            println!("  {description}");
        }
        if let PluginSource::External { source, location } = &plugin.source {
            let location = location.as_deref().unwrap_or("");
            println!(
                "  external ({source} {location}): installed from elsewhere, contents unknown"
            );
        }
        print_components("skills", &plugin.skills);
        print_components("agents", &plugin.agents);
        print_components("commands", &plugin.commands);
    }
    if !preview.warnings.is_empty() {
        println!("\nWarnings:");
        for warning in &preview.warnings {
            println!("  {}: {}", warning.path.display(), warning.message);
        }
    }
}

fn print_plugins(catalog: &CatalogPlugins) {
    for marketplace in &catalog.unreadable {
        let error = marketplace.error.as_deref().unwrap_or("unknown error");
        eprintln!("Skipped marketplace {}: {error}", marketplace.id);
    }
    if catalog.plugins.is_empty() {
        println!("No plugins. Add a marketplace with `harness-hanger marketplace add <source>`.");
        return;
    }
    for entry in &catalog.plugins {
        let status = match entry.status {
            PluginStatus::Enabled => "enabled",
            PluginStatus::Disabled => "disabled",
            PluginStatus::NotInstalled => "not installed",
        };
        let plugin = &entry.plugin;
        let contents = if let PluginSource::External { .. } = plugin.source {
            "external".to_owned()
        } else {
            format!(
                "{} skills, {} agents, {} commands",
                plugin.skills.len(),
                plugin.agents.len(),
                plugin.commands.len()
            )
        };
        let id = match plugin.source {
            PluginSource::Harness { .. } => format!("standalone in {}", entry.marketplace),
            _ => format!("{}@{}", plugin.name, entry.marketplace),
        };
        println!("{status:<13}  {id}  {contents}");
    }
}

fn print_components(kind: &str, components: &[Component]) {
    if components.is_empty() {
        return;
    }
    let names: Vec<_> = components.iter().map(|c| c.name.as_str()).collect();
    println!("  {kind}: {}", names.join(", "));
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
