mod backend;
mod commands;
mod config;
mod s3;

use clap::{Parser, Subcommand};
use colored::Colorize;
use sqlx::postgres::PgPoolOptions;

use backend::BackendError;
use backend::api::ApiBackend;
use backend::db::DbBackend;
use commands::alias::AliasCommand;
use commands::databases::DatabasesCommand;
use commands::env::EnvCommand;
use commands::kv::KvCommand;
use commands::migrate::MigrateCommand;
use commands::projects::ProjectsCommand;
use commands::storage::StorageCommand;
use commands::users::UsersCommand;
use commands::workers::WorkersCommand;
use config::{AliasConfig, Config, PlatformStorageConfig};

const EXAMPLES: &str = color_print::cstr!(
    r#"<bold><underline>Examples:</underline></bold>
  <dim># Quick start</dim>
  ow login                              <dim>Authenticate with the API</dim>
  ow workers create my-api              <dim>Create a new worker</dim>
  ow workers deploy my-api worker.ts    <dim>Deploy code to worker</dim>

  <dim># Self-hosting (database setup)</dim>
  ow alias set local --db postgres://... <dim>Configure DB connection</dim>
  ow local migrate run                  <dim>Run migrations</dim>
  ow local users create admin           <dim>Create first user (bootstrap)</dim>
  ow alias set local --db postgres://... --user admin  <dim>Set user context</dim>

  <dim># Environment and bindings</dim>
  ow env create prod                    <dim>Create an environment</dim>
  ow kv create cache                    <dim>Create a KV namespace</dim>
  ow env bind prod CACHE cache -t kv    <dim>Bind KV to environment</dim>
  ow workers link my-api my-env     <dim>Link environment to worker</dim>

  <dim># Using aliases (for multiple backends)</dim>
  ow alias list                         <dim>Show configured aliases</dim>
  ow local workers list                 <dim>List workers using 'local' alias</dim>
  ow prod workers list                  <dim>List workers using 'prod' alias</dim>

  <dim># Upload with assets (SvelteKit, etc.)</dim>
  ow workers upload my-app ./dist       <dim>Upload worker + assets folder</dim>
"#
);

#[derive(Parser)]
#[command(name = "ow")]
#[command(author, version, disable_version_flag = true)]
#[command(about = "OpenWorkers CLI - Deploy and manage serverless workers")]
#[command(
    long_about = "OpenWorkers CLI - Deploy and manage serverless workers.\n\n\
                  Commands can be prefixed with an alias name to target a specific backend:\n  \
                  ow <alias> <command>       e.g., ow local workers list\n  \
                  ow <command>               uses the default alias"
)]
#[command(after_help = EXAMPLES)]
struct Cli {
    /// Print version
    #[arg(short = 'v', long = "version", action = clap::ArgAction::Version)]
    version: (),

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage connection aliases (API or database backends)
    #[command(after_help = "Examples:\n  \
        ow alias list                                  List all aliases\n  \
        ow alias set prod --api https://api.example.com   Add API alias\n  \
        ow alias set local --db postgres://... --user max Add DB alias\n  \
        ow alias set-default prod                      Set default alias")]
    Alias {
        #[command(subcommand)]
        command: AliasCommand,
    },

    /// Authenticate and store API token for the current alias
    #[command(after_help = "Examples:\n  \
        ow login           Login to default alias\n  \
        ow prod login      Login to 'prod' alias")]
    Login,

    /// Run database migrations (requires db alias)
    #[command(after_help = "Examples:\n  \
        ow local migrate status    Show migration status\n  \
        ow local migrate run       Run pending migrations")]
    Migrate {
        #[command(subcommand)]
        command: MigrateCommand,
    },

    /// Manage users (requires db alias, no user context needed for create)
    #[command(
        visible_alias = "u",
        alias = "user",
        after_help = "Examples:\n  \
        ow local users list                    List all users\n  \
        ow local users create admin            Create user (bootstrap mode)\n  \
        ow local users get admin               Show user details"
    )]
    Users {
        #[command(subcommand)]
        command: UsersCommand,
    },

    /// Create, deploy, and manage workers
    #[command(
        visible_alias = "w",
        alias = "worker",
        after_help = "Examples:\n  \
        ow workers list                        List all workers\n  \
        ow workers create my-api               Create worker 'my-api'\n  \
        ow workers deploy my-api worker.ts     Deploy TypeScript code\n  \
        ow workers upload my-app ./dist        Upload folder with assets\n  \
        ow workers link my-api my-env      Link to environment"
    )]
    Workers {
        #[command(subcommand)]
        command: WorkersCommand,
    },

    /// Manage projects (multi-worker deployments)
    #[command(
        visible_alias = "p",
        alias = "project",
        after_help = "Examples:\n  \
        ow projects list                       List all projects\n  \
        ow projects delete my-app              Delete project and all its workers"
    )]
    Projects {
        #[command(subcommand)]
        command: ProjectsCommand,
    },

    /// Manage environments with variables, secrets, and bindings
    #[command(
        visible_alias = "e",
        alias = "envs",
        alias = "environment",
        alias = "environments",
        after_help = "Examples:\n  \
        ow env list                            List environments\n  \
        ow env create prod                     Create 'prod' environment\n  \
        ow env set prod API_KEY sk-xxx -s      Set secret\n  \
        ow env bind prod DB my-db -t database  Bind database\n  \
        ow env bind prod KV cache -t kv        Bind KV namespace\n  \
        ow env bind prod ASSETS storage -t assets  Bind storage for assets"
    )]
    Env {
        #[command(subcommand)]
        command: EnvCommand,
    },

    /// Manage S3/R2 storage configurations for file storage
    #[command(
        visible_alias = "s",
        alias = "storages",
        after_help = "Examples:\n  \
        ow storage list                        List storage configs\n  \
        ow storage create my-bucket --bucket name --endpoint https://..."
    )]
    Storage {
        #[command(subcommand)]
        command: StorageCommand,
    },

    /// Manage KV namespaces for key-value storage
    #[command(
        visible_alias = "k",
        alias = "kvs",
        after_help = "Examples:\n  \
        ow kv list                             List KV namespaces\n  \
        ow kv create cache                     Create 'cache' namespace"
    )]
    Kv {
        #[command(subcommand)]
        command: KvCommand,
    },

    /// Manage SQL databases
    #[command(
        visible_alias = "d",
        alias = "db",
        alias = "database",
        after_help = "Examples:\n  \
        ow databases list                      List databases\n  \
        ow databases create my-db              Create database"
    )]
    Databases {
        #[command(subcommand)]
        command: DatabasesCommand,
    },

    /// Configure platform storage for asset uploads (one-time setup for DB aliases)
    #[command(after_help = "Example:\n  \
        ow local setup-storage \\\n    \
          --endpoint https://xxx.r2.cloudflarestorage.com \\\n    \
          --bucket my-assets \\\n    \
          --access-key-id AKIA... \\\n    \
          --secret-access-key ...")]
    SetupStorage {
        /// S3-compatible endpoint URL (e.g., https://xxx.r2.cloudflarestorage.com)
        #[arg(long)]
        endpoint: String,

        /// Bucket name
        #[arg(long)]
        bucket: String,

        /// Access key ID
        #[arg(long)]
        access_key_id: String,

        /// Secret access key
        #[arg(long)]
        secret_access_key: String,

        /// Region (default: auto)
        #[arg(long, default_value = "auto")]
        region: String,

        /// Optional key prefix for all uploads
        #[arg(long)]
        prefix: Option<String>,
    },

    /// Test latency to the configured backend
    #[command(after_help = "Examples:\n  \
        ow test-latency              Test request latency (reuses connection)\n  \
        ow test-latency --connect    Test connection latency (new connection each time)\n  \
        ow local test-latency -n 20  Test with 20 iterations\n  \
        ow test-latency -p 5         Test with 5 parallel requests")]
    TestLatency {
        /// Test connection latency instead of request latency (new connection each time)
        #[arg(short, long)]
        connect: bool,

        /// Number of iterations (default: 10)
        #[arg(short = 'n', long, default_value = "10")]
        count: usize,

        /// Number of parallel requests (default: 1)
        #[arg(short, long, default_value = "1")]
        parallel: usize,

        /// Timeout in seconds (default: 5)
        #[arg(short, long, default_value = "5")]
        timeout: u64,
    },

    #[cfg(feature = "mcp")]
    /// Start MCP server (Model Context Protocol) on stdio
    #[command(after_help = "Examples:\n  \
        ow mcp                Start MCP server with default alias\n  \
        ow local mcp          Start MCP server with 'local' alias\n  \
        ow prod mcp           Start MCP server with 'prod' alias\n\n\
        The MCP server exposes CLI commands as tools for AI assistants.\n\
        It communicates via stdio using the Model Context Protocol.")]
    Mcp,
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

    #[cfg(feature = "mcp")]
    let known_commands = [
        // Main commands
        "alias",
        "login",
        "migrate",
        "users",
        "workers",
        "env",
        "storage",
        "kv",
        "databases",
        "setup-storage",
        "test-latency",
        "mcp",
        // Short aliases
        "u",
        "w",
        "e",
        "s",
        "k",
        "d",
        // Singular/plural variants (for flexibility)
        "user",
        "worker",
        "envs",
        "environment",
        "environments",
        "storages",
        "kvs",
        "db",
        "database",
        // Help flags
        "help",
        "--help",
        "-h",
        "--version",
        "-v",
    ];

    #[cfg(not(feature = "mcp"))]
    let known_commands = [
        // Main commands
        "alias",
        "login",
        "migrate",
        "users",
        "workers",
        "env",
        "storage",
        "kv",
        "databases",
        "setup-storage",
        "test-latency",
        // Short aliases
        "u",
        "w",
        "e",
        "s",
        "k",
        "d",
        // Singular/plural variants (for flexibility)
        "user",
        "worker",
        "envs",
        "environment",
        "environments",
        "storages",
        "kvs",
        "db",
        "database",
        // Help flags
        "help",
        "--help",
        "-h",
        "--version",
        "-v",
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
        AliasConfig::Db {
            database_url,
            user,
            storage,
        } => {
            let pool = PgPoolOptions::new()
                .max_connections(1)
                .connect(&database_url)
                .await
                .map_err(|e| e.to_string())?;

            let backend = DbBackend::new(pool, user, storage)
                .await
                .map_err(format_backend_error)?;
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

async fn run_projects_command(
    alias: Option<String>,
    command: ProjectsCommand,
) -> Result<(), String> {
    let alias_config = resolve_alias(alias)?;

    match alias_config {
        AliasConfig::Db {
            database_url, user, ..
        } => {
            let pool = PgPoolOptions::new()
                .max_connections(1)
                .connect(&database_url)
                .await
                .map_err(|e| e.to_string())?;

            let backend = DbBackend::new(pool, user, None)
                .await
                .map_err(format_backend_error)?;
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
        AliasConfig::Db {
            database_url, user, ..
        } => {
            let pool = PgPoolOptions::new()
                .max_connections(1)
                .connect(&database_url)
                .await
                .map_err(|e| e.to_string())?;

            let backend = DbBackend::new(pool, user, None)
                .await
                .map_err(format_backend_error)?;
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
        AliasConfig::Db {
            database_url,
            user,
            storage,
        } => {
            let pool = PgPoolOptions::new()
                .max_connections(1)
                .connect(&database_url)
                .await
                .map_err(|e| e.to_string())?;

            let backend = DbBackend::new(pool, user, storage)
                .await
                .map_err(format_backend_error)?;
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
        AliasConfig::Db {
            database_url, user, ..
        } => {
            let pool = PgPoolOptions::new()
                .max_connections(1)
                .connect(&database_url)
                .await
                .map_err(|e| e.to_string())?;

            let backend = DbBackend::new(pool, user, None)
                .await
                .map_err(format_backend_error)?;
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
        AliasConfig::Db {
            database_url, user, ..
        } => {
            let pool = PgPoolOptions::new()
                .max_connections(1)
                .connect(&database_url)
                .await
                .map_err(|e| e.to_string())?;

            let backend = DbBackend::new(pool, user, None)
                .await
                .map_err(format_backend_error)?;
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

fn cmd_setup_storage(
    alias: Option<String>,
    endpoint: String,
    bucket: String,
    access_key_id: String,
    secret_access_key: String,
    region: String,
    prefix: Option<String>,
) -> Result<(), String> {
    let mut config = Config::load().map_err(|e| e.to_string())?;

    let alias_name = alias
        .or(config.default.clone())
        .ok_or("No alias specified and no default configured")?;

    let alias_config = config
        .get_alias(&alias_name)
        .ok_or_else(|| format!("Alias '{}' not found", alias_name))?;

    // Ensure alias is a DB alias and extract existing fields
    let (database_url, user) = match alias_config {
        AliasConfig::Db {
            database_url, user, ..
        } => (database_url.clone(), user.clone()),
        AliasConfig::Api { .. } => {
            return Err("Storage can only be configured for DB aliases".to_string());
        }
    };

    let storage = PlatformStorageConfig {
        endpoint: endpoint.clone(),
        bucket: bucket.clone(),
        access_key_id,
        secret_access_key,
        region,
        prefix,
    };

    config.aliases.insert(
        alias_name.clone(),
        AliasConfig::db(database_url, user, Some(storage)),
    );
    config.save().map_err(|e| e.to_string())?;

    println!(
        "Configured storage for alias '{}' ({}/{})",
        alias_name.green().bold(),
        endpoint.cyan(),
        bucket.cyan()
    );

    Ok(())
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
        Commands::Migrate { command } => command.run(alias).await.map_err(|e| e.to_string()),
        Commands::Users { command } => command.run(alias).await.map_err(|e| e.to_string()),
        Commands::Workers { command } => run_workers_command(alias, command).await,
        Commands::Projects { command } => run_projects_command(alias, command).await,
        Commands::Env { command } => run_env_command(alias, command).await,
        Commands::Storage { command } => run_storage_command(alias, command).await,
        Commands::Kv { command } => run_kv_command(alias, command).await,
        Commands::Databases { command } => run_databases_command(alias, command).await,
        Commands::TestLatency {
            connect,
            count,
            parallel,
            timeout,
        } => commands::latency::run(alias, connect, count, parallel, timeout)
            .await
            .map_err(|e| e.to_string()),
        Commands::SetupStorage {
            endpoint,
            bucket,
            access_key_id,
            secret_access_key,
            region,
            prefix,
        } => cmd_setup_storage(
            alias,
            endpoint,
            bucket,
            access_key_id,
            secret_access_key,
            region,
            prefix,
        ),

        #[cfg(feature = "mcp")]
        Commands::Mcp => commands::mcp::run(alias).await.map_err(|e| e.to_string()),
    };

    if let Err(e) = result {
        eprintln!("{} {}", "error:".red().bold(), e);
        std::process::exit(1);
    }
}
