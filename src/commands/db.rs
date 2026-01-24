use crate::config::{AliasConfig, Config, ConfigError};
use clap::Subcommand;
use colored::Colorize;
use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};

static MIGRATOR: Migrator = sqlx::migrate!();

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("Config error: {0}")]
    Config(#[from] ConfigError),

    #[error("Database error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("Migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),

    #[error("Alias '{0}' is not a database alias. Use --db or configure a db alias.")]
    NotDbAlias(String),

    #[error("No alias specified and no default alias configured")]
    NoAlias,
}

#[derive(Subcommand)]
pub enum DbCommand {
    /// Run pending migrations
    Migrate,

    /// Show migration status
    Status,

    /// Mark all migrations as applied without running them (for existing databases)
    Baseline,
}

impl DbCommand {
    pub async fn run(self, alias: Option<String>) -> Result<(), DbError> {
        let database_url = resolve_database_url(alias)?;
        let pool = connect(&database_url).await?;

        match self {
            Self::Migrate => cmd_migrate(&pool).await,
            Self::Status => cmd_status(&pool).await,
            Self::Baseline => cmd_baseline(&pool).await,
        }
    }
}

fn resolve_database_url(alias: Option<String>) -> Result<String, DbError> {
    let config = Config::load()?;

    let alias_name = alias.or(config.default.clone()).ok_or(DbError::NoAlias)?;

    let alias_config = config
        .get_alias(&alias_name)
        .ok_or_else(|| ConfigError::AliasNotFound(alias_name.clone()))?;

    match alias_config {
        AliasConfig::Db { database_url, .. } => Ok(database_url.clone()),
        AliasConfig::Api { .. } => Err(DbError::NotDbAlias(alias_name)),
    }
}

async fn connect(database_url: &str) -> Result<PgPool, DbError> {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await?;

    Ok(pool)
}

async fn cmd_migrate(pool: &PgPool) -> Result<(), DbError> {
    println!("Running migrations...\n");

    MIGRATOR.run(pool).await?;

    println!("\n{}", "Migrations complete.".green().bold());

    Ok(())
}

async fn cmd_status(pool: &PgPool) -> Result<(), DbError> {
    // Get applied migrations from DB
    let applied: Vec<(i64, Vec<u8>)> =
        sqlx::query("SELECT version, checksum FROM _sqlx_migrations ORDER BY version")
            .fetch_all(pool)
            .await
            .unwrap_or_default()
            .iter()
            .map(|row| (row.get("version"), row.get("checksum")))
            .collect();

    println!("{}", "Migration Status".bold());
    println!("{}", "─".repeat(70));

    let mut pending_count = 0;

    for migration in MIGRATOR.iter() {
        let applied_entry = applied.iter().find(|(v, _)| *v == migration.version);

        let (status, checksum_warn) = match applied_entry {
            Some((_, db_checksum)) => {
                let matches = db_checksum == &migration.checksum.to_vec();

                if matches {
                    ("applied".green(), "")
                } else {
                    ("modified".red(), " (checksum mismatch!)")
                }
            }
            None => {
                pending_count += 1;
                ("pending".yellow(), "")
            }
        };

        println!(
            "  {:50} {}{}",
            migration.description.dimmed(),
            status,
            checksum_warn.red()
        );
    }

    println!("{}", "─".repeat(70));

    if pending_count == 0 {
        println!("{}", "All migrations applied.".green());
    } else {
        println!(
            "{} pending migration(s). Run '{}' to apply.",
            pending_count.to_string().yellow(),
            "ow db migrate".cyan()
        );
    }

    Ok(())
}

async fn cmd_baseline(pool: &PgPool) -> Result<(), DbError> {
    // Create _sqlx_migrations table if it doesn't exist
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS _sqlx_migrations (
            version BIGINT PRIMARY KEY,
            description TEXT NOT NULL,
            installed_on TIMESTAMPTZ NOT NULL DEFAULT now(),
            success BOOLEAN NOT NULL,
            checksum BYTEA NOT NULL,
            execution_time BIGINT NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Check if already baselined
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(pool)
        .await?;

    if count > 0 {
        println!(
            "{} Database already has {} migration(s) recorded.",
            "Warning:".yellow().bold(),
            count
        );
        println!("Use '{}' to check status.", "ow db status".cyan());
        return Ok(());
    }

    println!("Marking all migrations as applied...\n");

    for migration in MIGRATOR.iter() {
        sqlx::query(
            r#"
            INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time)
            VALUES ($1, $2, true, $3, 0)
            "#,
        )
        .bind(migration.version)
        .bind(&*migration.description)
        .bind(&migration.checksum[..])
        .execute(pool)
        .await?;

        println!("  {} {}", "Baseline".blue(), migration.description);
    }

    // Drop old _migrations table if it exists
    sqlx::query("DROP TABLE IF EXISTS _migrations")
        .execute(pool)
        .await?;

    println!("\n{}", "Baseline complete.".green().bold());

    Ok(())
}
