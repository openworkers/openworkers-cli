mod commands;
mod config;

use clap::{Parser, Subcommand};
use colored::Colorize;

use commands::alias::AliasCommand;
use commands::db::DbCommand;
use config::Config;

#[derive(Parser)]
#[command(name = "ow")]
#[command(author, version, about = "OpenWorkers CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage connection aliases
    Alias {
        #[command(subcommand)]
        command: AliasCommand,
    },

    /// Database operations (requires db alias)
    Db {
        #[command(subcommand)]
        command: DbCommand,
    },
}

/// Extract alias from args if first arg matches a known alias.
/// Returns (alias, filtered_args) where filtered_args has the alias removed.
fn extract_alias_from_args() -> (Option<String>, Vec<String>) {
    let args: Vec<String> = std::env::args().collect();

    // Need at least: program name + potential alias + command
    if args.len() < 2 {
        return (None, args);
    }

    let potential_alias = &args[1];

    // Skip if it looks like a flag or is a known command
    if potential_alias.starts_with('-') {
        return (None, args);
    }

    let known_commands = ["alias", "db", "help", "--help", "-h", "--version", "-V"];

    if known_commands.contains(&potential_alias.as_str()) {
        return (None, args);
    }

    // Check if it's a known alias
    if let Ok(config) = Config::load() {
        if config.get_alias(potential_alias).is_some() {
            // Remove the alias from args
            let mut filtered: Vec<String> = Vec::with_capacity(args.len() - 1);
            filtered.push(args[0].clone());
            filtered.extend(args[2..].iter().cloned());
            return (Some(potential_alias.clone()), filtered);
        }
    }

    (None, args)
}

#[tokio::main]
async fn main() {
    let (alias, args) = extract_alias_from_args();

    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(e) => {
            e.exit();
        }
    };

    let result = match cli.command {
        Commands::Alias { command } => command.run().map_err(|e| e.to_string()),
        Commands::Db { command } => command.run(alias).await.map_err(|e| e.to_string()),
    };

    if let Err(e) = result {
        eprintln!("{} {}", "error:".red().bold(), e);
        std::process::exit(1);
    }
}
