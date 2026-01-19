mod commands;
mod config;

use clap::{Parser, Subcommand};
use colored::Colorize;

use commands::alias::AliasCommand;
use commands::db::DbCommand;

#[derive(Parser)]
#[command(name = "ow")]
#[command(author, version, about = "OpenWorkers CLI", long_about = None)]
struct Cli {
    /// Use a specific alias
    #[arg(long, global = true)]
    alias: Option<String>,

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

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Alias { command } => command.run().map_err(|e| e.to_string()),
        Commands::Db { command } => command.run(cli.alias).await.map_err(|e| e.to_string()),
    };

    if let Err(e) = result {
        eprintln!("{} {}", "error:".red().bold(), e);
        std::process::exit(1);
    }
}
