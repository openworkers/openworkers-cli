use crate::backend::{Backend, BackendError, CreateDatabaseInput};
use clap::Subcommand;
use colored::Colorize;

#[derive(Subcommand)]
pub enum DatabasesCommand {
    /// List all databases
    #[command(alias = "ls")]
    List,

    /// Get database details
    Get {
        /// Database name
        name: String,
    },

    /// Create a new database
    Create {
        /// Database name
        name: String,

        /// Provider: platform (default) or postgres
        #[arg(long, default_value = "platform")]
        provider: String,

        /// Postgres connection string (required for postgres provider)
        #[arg(long)]
        connection_string: Option<String>,

        /// Description
        #[arg(short, long)]
        description: Option<String>,

        /// Maximum rows per query (default: 1000)
        #[arg(long)]
        max_rows: Option<i32>,

        /// Query timeout in seconds (default: 30)
        #[arg(long)]
        timeout: Option<i32>,
    },

    /// Delete a database
    #[command(alias = "rm")]
    Delete {
        /// Database name
        name: String,
    },
}

impl DatabasesCommand {
    pub async fn run<B: Backend>(self, backend: &B) -> Result<(), BackendError> {
        match self {
            Self::List => cmd_list(backend).await,
            Self::Get { name } => cmd_get(backend, &name).await,
            Self::Create {
                name,
                provider,
                connection_string,
                description,
                max_rows,
                timeout,
            } => {
                cmd_create(
                    backend,
                    name,
                    provider,
                    connection_string,
                    description,
                    max_rows,
                    timeout,
                )
                .await
            }
            Self::Delete { name } => cmd_delete(backend, &name).await,
        }
    }
}

async fn cmd_list<B: Backend>(backend: &B) -> Result<(), BackendError> {
    let databases = backend.list_databases().await?;

    if databases.is_empty() {
        println!("No databases found.");
        return Ok(());
    }

    println!("{}", "Databases".bold());
    println!("{}", "─".repeat(60));

    for db in databases {
        let provider_badge = match db.provider.as_str() {
            "platform" => "[platform]".cyan(),
            "postgres" => "[postgres]".yellow(),
            _ => format!("[{}]", db.provider).dimmed(),
        };

        println!("  {} {:30}", provider_badge, db.name.bold());
    }

    Ok(())
}

async fn cmd_get<B: Backend>(backend: &B, name: &str) -> Result<(), BackendError> {
    let db = backend.get_database(name).await?;

    println!("{:12} {}", "Name:".dimmed(), db.name.bold());
    println!("{:12} {}", "ID:".dimmed(), db.id);
    println!("{:12} {}", "Provider:".dimmed(), db.provider);

    if let Some(desc) = &db.description {
        println!("{:12} {}", "Description:".dimmed(), desc);
    }

    println!("{:12} {}", "Max Rows:".dimmed(), db.max_rows);
    println!("{:12} {}s", "Timeout:".dimmed(), db.timeout_seconds);

    println!(
        "{:12} {}",
        "Created:".dimmed(),
        db.created_at.format("%Y-%m-%d %H:%M:%S")
    );

    Ok(())
}

async fn cmd_create<B: Backend>(
    backend: &B,
    name: String,
    provider: String,
    connection_string: Option<String>,
    description: Option<String>,
    max_rows: Option<i32>,
    timeout: Option<i32>,
) -> Result<(), BackendError> {
    if provider == "postgres" && connection_string.is_none() {
        return Err(BackendError::Api(
            "--connection-string is required for postgres provider".to_string(),
        ));
    }

    let input = CreateDatabaseInput {
        name,
        desc: description,
        provider: provider.clone(),
        connection_string,
        max_rows,
        timeout_seconds: timeout,
    };

    let db = backend.create_database(input).await?;

    println!(
        "{} Database '{}' created ({} provider).",
        "Created".green(),
        db.name.bold(),
        provider
    );

    Ok(())
}

async fn cmd_delete<B: Backend>(backend: &B, name: &str) -> Result<(), BackendError> {
    backend.delete_database(name).await?;

    println!("{} Database '{}' deleted.", "Deleted".red(), name.bold());

    Ok(())
}
