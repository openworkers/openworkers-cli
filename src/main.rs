mod backend;
mod commands;
mod config;

use clap::{Parser, Subcommand};
use colored::Colorize;
use sqlx::postgres::PgPoolOptions;

use backend::BackendError;
use backend::api::ApiBackend;
use backend::db::DbBackend;
use commands::alias::AliasCommand;
use commands::databases::DatabasesCommand;
use commands::db::DbCommand;
use commands::env::EnvCommand;
use commands::kv::KvCommand;
use commands::storage::StorageCommand;
use commands::workers::WorkersCommand;
use config::{AliasConfig, Config};

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

    /// Set API token for an alias
    Login,

    /// Database operations (requires db alias)
    Db {
        #[command(subcommand)]
        command: DbCommand,
    },

    /// Manage workers
    Workers {
        #[command(subcommand)]
        command: WorkersCommand,
    },

    /// Manage environments (variables and secrets)
    Env {
        #[command(subcommand)]
        command: EnvCommand,
    },

    /// Manage storage configs (S3/R2 bindings)
    Storage {
        #[command(subcommand)]
        command: StorageCommand,
    },

    /// Manage KV namespaces
    Kv {
        #[command(subcommand)]
        command: KvCommand,
    },

    /// Manage databases
    Databases {
        #[command(subcommand)]
        command: DatabasesCommand,
    },
}

/// Extract alias from args if first arg matches a known alias.
fn extract_alias_from_args() -> (Option<String>, Vec<String>) {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        return (None, args);
    }

    let potential_alias = &args[1];

    if potential_alias.starts_with('-') {
        return (None, args);
    }

    let known_commands = [
        "alias",
        "login",
        "db",
        "workers",
        "env",
        "storage",
        "kv",
        "databases",
        "help",
        "--help",
        "-h",
        "--version",
        "-V",
    ];

    if known_commands.contains(&potential_alias.as_str()) {
        return (None, args);
    }

    if let Ok(config) = Config::load() {
        if config.get_alias(potential_alias).is_some() {
            let mut filtered: Vec<String> = Vec::with_capacity(args.len() - 1);
            filtered.push(args[0].clone());
            filtered.extend(args[2..].iter().cloned());
            return (Some(potential_alias.clone()), filtered);
        }
    }

    (None, args)
}

fn resolve_alias(alias: Option<String>) -> Result<AliasConfig, String> {
    let config = Config::load().map_err(|e| e.to_string())?;

    let alias_name = alias
        .or(config.default.clone())
        .ok_or("No alias specified and no default configured")?;

    config
        .get_alias(&alias_name)
        .cloned()
        .ok_or_else(|| format!("Alias '{}' not found", alias_name))
}

async fn run_workers_command(alias: Option<String>, command: WorkersCommand) -> Result<(), String> {
    let alias_config = resolve_alias(alias)?;

    match alias_config {
        AliasConfig::Db { database_url } => {
            let pool = PgPoolOptions::new()
                .max_connections(1)
                .connect(&database_url)
                .await
                .map_err(|e| e.to_string())?;

            let backend = DbBackend::new(pool);
            command.run(&backend).await.map_err(format_backend_error)
        }

        AliasConfig::Api {
            url,
            token,
            insecure,
        } => {
            let backend = ApiBackend::new(url, token, insecure);
            command.run(&backend).await.map_err(format_backend_error)
        }
    }
}

async fn run_env_command(alias: Option<String>, command: EnvCommand) -> Result<(), String> {
    let alias_config = resolve_alias(alias)?;

    match alias_config {
        AliasConfig::Db { database_url } => {
            let pool = PgPoolOptions::new()
                .max_connections(1)
                .connect(&database_url)
                .await
                .map_err(|e| e.to_string())?;

            let backend = DbBackend::new(pool);
            command.run(&backend).await.map_err(format_backend_error)
        }

        AliasConfig::Api {
            url,
            token,
            insecure,
        } => {
            let backend = ApiBackend::new(url, token, insecure);
            command.run(&backend).await.map_err(format_backend_error)
        }
    }
}

async fn run_storage_command(alias: Option<String>, command: StorageCommand) -> Result<(), String> {
    let alias_config = resolve_alias(alias)?;

    match alias_config {
        AliasConfig::Db { database_url } => {
            let pool = PgPoolOptions::new()
                .max_connections(1)
                .connect(&database_url)
                .await
                .map_err(|e| e.to_string())?;

            let backend = DbBackend::new(pool);
            command.run(&backend).await.map_err(format_backend_error)
        }

        AliasConfig::Api {
            url,
            token,
            insecure,
        } => {
            let backend = ApiBackend::new(url, token, insecure);
            command.run(&backend).await.map_err(format_backend_error)
        }
    }
}

async fn run_kv_command(alias: Option<String>, command: KvCommand) -> Result<(), String> {
    let alias_config = resolve_alias(alias)?;

    match alias_config {
        AliasConfig::Db { database_url } => {
            let pool = PgPoolOptions::new()
                .max_connections(1)
                .connect(&database_url)
                .await
                .map_err(|e| e.to_string())?;

            let backend = DbBackend::new(pool);
            command.run(&backend).await.map_err(format_backend_error)
        }

        AliasConfig::Api {
            url,
            token,
            insecure,
        } => {
            let backend = ApiBackend::new(url, token, insecure);
            command.run(&backend).await.map_err(format_backend_error)
        }
    }
}

async fn run_databases_command(
    alias: Option<String>,
    command: DatabasesCommand,
) -> Result<(), String> {
    let alias_config = resolve_alias(alias)?;

    match alias_config {
        AliasConfig::Db { database_url } => {
            let pool = PgPoolOptions::new()
                .max_connections(1)
                .connect(&database_url)
                .await
                .map_err(|e| e.to_string())?;

            let backend = DbBackend::new(pool);
            command.run(&backend).await.map_err(format_backend_error)
        }

        AliasConfig::Api {
            url,
            token,
            insecure,
        } => {
            let backend = ApiBackend::new(url, token, insecure);
            command.run(&backend).await.map_err(format_backend_error)
        }
    }
}

fn format_backend_error(e: BackendError) -> String {
    match e {
        BackendError::NotFound(msg) => msg,
        BackendError::Unauthorized => "Unauthorized. Check your token.".to_string(),
        _ => e.to_string(),
    }
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
        Commands::Login => (|| {
            let config = Config::load().map_err(|e| e.to_string())?;
            let alias_name = alias
                .or(config.default.clone())
                .ok_or("No alias specified and no default configured".to_string())?;
            commands::login::run(&alias_name).map_err(|e| e.to_string())
        })(),
        Commands::Db { command } => command.run(alias).await.map_err(|e| e.to_string()),
        Commands::Workers { command } => run_workers_command(alias, command).await,
        Commands::Env { command } => run_env_command(alias, command).await,
        Commands::Storage { command } => run_storage_command(alias, command).await,
        Commands::Kv { command } => run_kv_command(alias, command).await,
        Commands::Databases { command } => run_databases_command(alias, command).await,
    };

    if let Err(e) = result {
        eprintln!("{} {}", "error:".red().bold(), e);
        std::process::exit(1);
    }
}
