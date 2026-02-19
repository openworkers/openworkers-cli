use crate::backend::{Backend, BackendError, CreateDatabaseInput, DatabaseProvider};
use clap::Subcommand;
use colored::Colorize;

#[derive(Subcommand)]
pub enum DatabasesCommand {
    /// List all database configurations
    #[command(alias = "ls")]
    List,

    /// Show database configuration details
    #[command(after_help = "Example:\n  ow databases get my-db")]
    Get {
        /// Database name
        name: String,
    },

    /// Create a database configuration for SQL access from workers
    #[command(after_help = "Examples:\n  \
        ow databases create my-db\n  \
        ow databases create my-db --provider postgres \\\n    \
          --connection-string postgres://user:pass@host/db\n  \
        ow databases create analytics --max-rows 5000 --timeout 60")]
    Create {
        /// Database configuration name
        name: String,

        /// Database provider: platform (managed) or postgres (bring your own)
        #[arg(long, value_enum, default_value = "platform")]
        provider: DatabaseProvider,

        /// PostgreSQL connection string (required for postgres provider)
        #[arg(long)]
        connection_string: Option<String>,

        /// Description of this database
        #[arg(short, long)]
        description: Option<String>,

        /// Maximum rows returned per query (default: 1000)
        #[arg(long)]
        max_rows: Option<i32>,

        /// Query timeout in seconds (default: 30)
        #[arg(long)]
        timeout: Option<i32>,
    },

    /// Delete a database configuration
    #[command(alias = "rm", after_help = "Example:\n  ow databases delete old-db")]
    Delete {
        /// Database name to delete
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
        let provider_badge = match db.provider {
            DatabaseProvider::Platform => "[platform]".cyan(),
            DatabaseProvider::Postgres => "[postgres]".yellow(),
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
    provider: DatabaseProvider,
    connection_string: Option<String>,
    description: Option<String>,
    max_rows: Option<i32>,
    timeout: Option<i32>,
) -> Result<(), BackendError> {
    if provider == DatabaseProvider::Postgres && connection_string.is_none() {
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
