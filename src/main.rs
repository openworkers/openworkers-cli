mod backend;
mod config;

use anyhow::Result;
use backend::{Backend, DeployOptions, api::ApiBackend, db::DbBackend};
use clap::{Parser, Subcommand};
use config::Config;
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "ow")]
#[command(about = "OpenWorkers CLI - Manage your JavaScript workers", long_about = None)]
struct Cli {
    /// Use direct database connection instead of API
    #[arg(long, global = true)]
    db: bool,

    /// Database URL (overrides config)
    #[arg(long, global = true)]
    database_url: Option<String>,

    /// API URL (overrides config)
    #[arg(long, global = true)]
    api_url: Option<String>,

    /// API key (overrides config)
    #[arg(long, global = true)]
    api_key: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List all workers
    List,

    /// Get worker details
    Get {
        /// Worker ID or name
        id_or_name: String,
    },

    /// Deploy a new worker
    Deploy {
        /// Worker name
        #[arg(short, long)]
        name: String,

        /// Path to JavaScript file
        file: PathBuf,

        /// Environment variables (KEY=VALUE)
        #[arg(short, long)]
        env: Vec<String>,
    },

    /// Update worker code
    Update {
        /// Worker ID
        id: Uuid,

        /// Path to JavaScript file
        file: PathBuf,
    },

    /// Set environment variables
    Env {
        /// Worker ID
        id: Uuid,

        /// Environment variables (KEY=VALUE)
        #[arg(required = true)]
        vars: Vec<String>,
    },

    /// Delete a worker
    Delete {
        /// Worker ID
        id: Uuid,

        /// Skip confirmation
        #[arg(short, long)]
        force: bool,
    },

    /// View worker logs
    Logs {
        /// Worker ID
        id: Uuid,

        /// Number of log entries
        #[arg(short, long, default_value = "50")]
        limit: i64,
    },

    /// Initialize configuration
    Init,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = Config::load()?;

    let backend: Box<dyn Backend> = if cli.db {
        let db_url = cli.database_url.or(config.database_url).ok_or_else(|| {
            anyhow::anyhow!("Database URL required. Use --database-url or set in config.")
        })?;
        Box::new(DbBackend::new(&db_url).await?)
    } else {
        let api_url = cli
            .api_url
            .or(config.api_url)
            .unwrap_or_else(|| "https://api.openworkers.dev".to_string());
        let api_key = cli.api_key.or(config.api_key);
        Box::new(ApiBackend::new(&api_url, api_key))
    };

    match cli.command {
        Commands::List => cmd_list(&*backend).await,
        Commands::Get { id_or_name } => cmd_get(&*backend, &id_or_name).await,
        Commands::Deploy { name, file, env } => cmd_deploy(&*backend, &name, &file, &env).await,
        Commands::Update { id, file } => cmd_update(&*backend, id, &file).await,
        Commands::Env { id, vars } => cmd_env(&*backend, id, &vars).await,
        Commands::Delete { id, force } => cmd_delete(&*backend, id, force).await,
        Commands::Logs { id, limit } => cmd_logs(&*backend, id, limit).await,
        Commands::Init => cmd_init().await,
    }
}

async fn cmd_list(backend: &dyn Backend) -> Result<()> {
    let workers = backend.list_workers().await?;

    if workers.is_empty() {
        println!("No workers found.");
        return Ok(());
    }

    println!("{:<36}  {:<20}  {:<20}", "ID", "NAME", "UPDATED");
    println!("{}", "-".repeat(80));

    for worker in workers {
        println!(
            "{:<36}  {:<20}  {:<20}",
            worker.id,
            truncate(&worker.name, 20),
            worker.updated_at.format("%Y-%m-%d %H:%M:%S")
        );
    }

    Ok(())
}

async fn cmd_get(backend: &dyn Backend, id_or_name: &str) -> Result<()> {
    let worker = backend
        .get_worker(id_or_name)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Worker not found: {}", id_or_name))?;

    println!("ID:         {}", worker.id);
    println!("Name:       {}", worker.name);
    println!(
        "Created:    {}",
        worker.created_at.format("%Y-%m-%d %H:%M:%S UTC")
    );
    println!(
        "Updated:    {}",
        worker.updated_at.format("%Y-%m-%d %H:%M:%S UTC")
    );

    if let Some(env) = &worker.env {
        if !env.is_empty() {
            println!("\nEnvironment:");
            for (key, value) in env {
                println!("  {}={}", key, mask_value(value));
            }
        }
    }

    println!("\nCode ({} bytes):", worker.code.len());
    println!("{}", "-".repeat(40));
    println!("{}", worker.code);

    Ok(())
}

async fn cmd_deploy(
    backend: &dyn Backend,
    name: &str,
    file: &PathBuf,
    env_args: &[String],
) -> Result<()> {
    let code = std::fs::read_to_string(file)?;
    let env = parse_env_vars(env_args)?;

    let options = DeployOptions {
        name: name.to_string(),
        code,
        env: if env.is_empty() { None } else { Some(env) },
    };

    let worker = backend.deploy(options).await?;

    println!("Worker deployed successfully!");
    println!("ID:   {}", worker.id);
    println!("Name: {}", worker.name);

    Ok(())
}

async fn cmd_update(backend: &dyn Backend, id: Uuid, file: &PathBuf) -> Result<()> {
    let code = std::fs::read_to_string(file)?;
    let worker = backend.update_code(id, &code).await?;

    println!("Worker updated successfully!");
    println!("ID:      {}", worker.id);
    println!(
        "Updated: {}",
        worker.updated_at.format("%Y-%m-%d %H:%M:%S UTC")
    );

    Ok(())
}

async fn cmd_env(backend: &dyn Backend, id: Uuid, vars: &[String]) -> Result<()> {
    let env = parse_env_vars(vars)?;
    let worker = backend.update_env(id, env).await?;

    println!("Environment updated for worker: {}", worker.name);

    Ok(())
}

async fn cmd_delete(backend: &dyn Backend, id: Uuid, force: bool) -> Result<()> {
    if !force {
        print!("Are you sure you want to delete worker {}? [y/N] ", id);
        use std::io::{self, Write};
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    backend.delete(id).await?;
    println!("Worker {} deleted.", id);

    Ok(())
}

async fn cmd_logs(backend: &dyn Backend, id: Uuid, limit: i64) -> Result<()> {
    let logs = backend.logs(id, limit).await?;

    if logs.is_empty() {
        println!("No logs found.");
        return Ok(());
    }

    for log in logs {
        let level_colored = match log.level.to_uppercase().as_str() {
            "ERROR" => "\x1b[31mERROR\x1b[0m",
            "WARN" => "\x1b[33mWARN\x1b[0m",
            "INFO" => "\x1b[34mINFO\x1b[0m",
            "DEBUG" => "\x1b[90mDEBUG\x1b[0m",
            _ => &log.level,
        };

        println!(
            "{} [{}] {}",
            log.date.format("%Y-%m-%d %H:%M:%S"),
            level_colored,
            log.message
        );
    }

    Ok(())
}

async fn cmd_init() -> Result<()> {
    let config_path = Config::path()?;

    if config_path.exists() {
        println!("Config already exists at: {}", config_path.display());
        return Ok(());
    }

    let default_config = r#"# OpenWorkers CLI Configuration

# API mode (default)
api_url = "https://api.openworkers.dev"
# api_key = "your-api-key"

# Direct database mode (use with --db flag)
# database_url = "postgres://user:pass@localhost/openworkers"
"#;

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(&config_path, default_config)?;
    println!("Config created at: {}", config_path.display());

    Ok(())
}

// Helper functions
fn parse_env_vars(vars: &[String]) -> Result<HashMap<String, String>> {
    let mut env = HashMap::new();
    for var in vars {
        let parts: Vec<&str> = var.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!(
                "Invalid environment variable format: {}. Use KEY=VALUE",
                var
            ));
        }
        env.insert(parts[0].to_string(), parts[1].to_string());
    }
    Ok(env)
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

fn mask_value(value: &str) -> String {
    if value.len() <= 4 {
        "*".repeat(value.len())
    } else {
        format!("{}***", &value[..2])
    }
}
