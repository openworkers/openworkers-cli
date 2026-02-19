use crate::config::{AliasConfig, Config, ConfigError};
use clap::Subcommand;
use colored::Colorize;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};

#[derive(Debug, thiserror::Error)]
pub enum UsersError {
    #[error("Config error: {0}")]
    Config(#[from] ConfigError),

    #[error("Database error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("Alias '{0}' is not a database alias. Use --db when creating the alias.")]
    NotDbAlias(String),

    #[error("No alias specified and no default alias configured")]
    NoAlias,

    #[error("User '{0}' not found")]
    UserNotFound(String),

    #[error("User '{0}' already exists")]
    UserExists(String),
}

#[derive(Subcommand)]
pub enum UsersCommand {
    /// List all users
    #[command(alias = "ls", after_help = "Example:\n  ow local users list")]
    List,

    /// Show user details
    #[command(after_help = "Example:\n  ow local users get admin")]
    Get {
        /// Username
        username: String,
    },

    /// Create a new user (bootstrap mode - no user required)
    #[command(after_help = "Examples:\n  \
        ow local users create max\n  \
        ow local users create max --system")]
    Create {
        /// Username for the new user
        username: String,

        /// Claim the system user (rename __system__ to this username)
        #[arg(long)]
        system: bool,
    },

    /// Delete a user
    #[command(
        alias = "rm",
        after_help = "Example:\n  ow local users delete old-user"
    )]
    Delete {
        /// Username to delete
        username: String,
    },
}

impl UsersCommand {
    pub async fn run(self, alias: Option<String>) -> Result<(), UsersError> {
        let database_url = resolve_database_url(alias)?;
        let pool = connect(&database_url).await?;

        match self {
            Self::List => cmd_list(&pool).await,
            Self::Get { username } => cmd_get(&pool, &username).await,
            Self::Create { username, system } => cmd_create(&pool, username, system).await,
            Self::Delete { username } => cmd_delete(&pool, &username).await,
        }
    }
}

fn resolve_database_url(alias: Option<String>) -> Result<String, UsersError> {
    let config = Config::load()?;

    let alias_name = alias
        .or(config.default.clone())
        .ok_or(UsersError::NoAlias)?;

    let alias_config = config
        .get_alias(&alias_name)
        .ok_or_else(|| ConfigError::AliasNotFound(alias_name.clone()))?;

    match alias_config {
        AliasConfig::Db { database_url, .. } => Ok(database_url.clone()),
        AliasConfig::Api { .. } => Err(UsersError::NotDbAlias(alias_name)),
    }
}

async fn connect(database_url: &str) -> Result<PgPool, UsersError> {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await?;

    Ok(pool)
}

async fn cmd_list(pool: &PgPool) -> Result<(), UsersError> {
    let rows = sqlx::query(
        r#"
        SELECT id, username, created_at
        FROM users
        ORDER BY created_at
        "#,
    )
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        println!("No users found.");
        return Ok(());
    }

    println!("{}", "Users".bold());
    println!("{}", "─".repeat(60));

    for row in rows {
        let username: String = row.get("username");
        let id: uuid::Uuid = row.get("id");
        let created_at: chrono::NaiveDateTime = row.get("created_at");

        println!(
            "  {} {} {}",
            username.bold(),
            format!("({})", id).dimmed(),
            format!("created {}", created_at.format("%Y-%m-%d")).dimmed()
        );
    }

    Ok(())
}

async fn cmd_get(pool: &PgPool, username: &str) -> Result<(), UsersError> {
    let row = sqlx::query(
        r#"
        SELECT id, username, created_at, updated_at
        FROM users
        WHERE username = $1
        "#,
    )
    .bind(username)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| UsersError::UserNotFound(username.to_string()))?;

    let id: uuid::Uuid = row.get("id");
    let username: String = row.get("username");
    let created_at: chrono::NaiveDateTime = row.get("created_at");
    let updated_at: chrono::NaiveDateTime = row.get("updated_at");

    println!("{:12} {}", "Username:".dimmed(), username.bold());
    println!("{:12} {}", "ID:".dimmed(), id);
    println!(
        "{:12} {}",
        "Created:".dimmed(),
        created_at.format("%Y-%m-%d %H:%M:%S")
    );
    println!(
        "{:12} {}",
        "Updated:".dimmed(),
        updated_at.format("%Y-%m-%d %H:%M:%S")
    );

    Ok(())
}

async fn cmd_create(pool: &PgPool, username: String, system: bool) -> Result<(), UsersError> {
    if system {
        // Rename the system user to claim it
        let result = sqlx::query(
            "UPDATE users SET username = $1 WHERE id = '00000000-0000-0000-0000-000000000000'",
        )
        .bind(&username)
        .execute(pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(UsersError::UserNotFound("__system__".to_string()));
        }

        println!(
            "{} System user renamed to '{}'.",
            "Updated".green().bold(),
            username.bold(),
        );
    } else {
        // Check if user already exists
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE username = $1)")
                .bind(&username)
                .fetch_one(pool)
                .await?;

        if exists {
            return Err(UsersError::UserExists(username));
        }

        // Insert new user
        let row = sqlx::query(
            r#"
            INSERT INTO users (username)
            VALUES ($1)
            RETURNING id, username, created_at
            "#,
        )
        .bind(&username)
        .fetch_one(pool)
        .await?;

        let id: uuid::Uuid = row.get("id");

        println!(
            "{} User '{}' created (ID: {}).",
            "Created".green().bold(),
            username.bold(),
            id.to_string().dimmed()
        );
    }

    println!("\n{} Set this user as default with:", "Next:".cyan().bold());
    println!(
        "  {}",
        format!("ow alias set <alias> --db <url> --user {}", username).cyan()
    );

    Ok(())
}

async fn cmd_delete(pool: &PgPool, username: &str) -> Result<(), UsersError> {
    let result = sqlx::query("DELETE FROM users WHERE username = $1")
        .bind(username)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(UsersError::UserNotFound(username.to_string()));
    }

    println!(
        "{} User '{}' deleted.",
        "Deleted".red().bold(),
        username.bold()
    );

    Ok(())
}
