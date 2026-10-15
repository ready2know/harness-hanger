use anyhow::Result;
use clap::{Parser, Subcommand};
use hanger_core::AppInfo;

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
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Info => info(cli.json),
    }
}

fn info(json: bool) -> Result<()> {
    let info = AppInfo::collect();
    if json {
        println!("{}", serde_json::to_string_pretty(&info)?);
        return Ok(());
    }
    println!("harness-hanger {}", info.version);
    match &info.claude_config_dir {
        Some(dir) => println!("Claude Code config: {}", dir.display()),
        None => println!("Claude Code config: not found"),
    }
    Ok(())
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
