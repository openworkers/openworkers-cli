use crate::backend::{Backend, BackendError, CreateD1Input};
use clap::Subcommand;
use colored::Colorize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Subcommand)]
pub enum D1Command {
    /// List all D1 databases
    #[command(alias = "ls")]
    List,

    /// Show D1 database details
    #[command(alias = "info", after_help = "Example:\n  ow d1 get guestbook")]
    Get {
        /// D1 database name
        name: String,
    },

    /// Create a D1 database (SQLite in the runner)
    #[command(after_help = "Examples:\n  \
        ow d1 create guestbook\n  \
        ow d1 create app -d \"App data\"")]
    Create {
        /// D1 database name
        name: String,

        /// Description of what this database stores
        #[arg(short, long)]
        description: Option<String>,
    },

    /// Delete a D1 database and all its data
    #[command(alias = "rm", after_help = "Example:\n  ow d1 delete old-db")]
    Delete {
        /// D1 database name to delete
        name: String,
    },

    /// Run SQL on a D1 database
    #[command(after_help = "Examples:\n  \
        ow d1 execute guestbook --command \"SELECT * FROM entries\"\n  \
        ow d1 execute guestbook --file seed.sql")]
    Execute {
        /// D1 database name
        name: String,

        /// One statement; its rows are printed
        #[arg(
            long,
            short = 'c',
            conflicts_with = "file",
            required_unless_present = "file"
        )]
        command: Option<String>,

        /// A script of statements, each committed on its own
        #[arg(long, short = 'f')]
        file: Option<PathBuf>,

        /// Print the raw result as JSON
        #[arg(long)]
        json: bool,
    },

    /// Apply or list the SQL migrations of a D1 database
    Migrations {
        #[command(subcommand)]
        command: MigrationsCommand,
    },
}

#[derive(Subcommand)]
pub enum MigrationsCommand {
    /// Apply the pending migrations, in file name order
    #[command(
        after_help = "Each file runs in one transaction with its ledger row.\n\n\
        Example:\n  ow d1 migrations apply guestbook --dir migrations"
    )]
    Apply {
        /// D1 database name
        name: String,

        /// Directory of the .sql migration files
        #[arg(long, default_value = "migrations")]
        dir: PathBuf,
    },

    /// Show which migrations are applied, pending or changed
    #[command(after_help = "Example:\n  ow d1 migrations status guestbook")]
    Status {
        /// D1 database name
        name: String,

        /// Directory of the .sql migration files
        #[arg(long, default_value = "migrations")]
        dir: PathBuf,
    },
}

impl D1Command {
    pub async fn run<B: Backend>(self, backend: &B) -> Result<(), BackendError> {
        match self {
            Self::List => cmd_list(backend).await,
            Self::Get { name } => cmd_get(backend, &name).await,
            Self::Create { name, description } => cmd_create(backend, name, description).await,
            Self::Delete { name } => cmd_delete(backend, &name).await,
            Self::Execute {
                name,
                command,
                file,
                json,
            } => cmd_execute(backend, &name, command, file, json).await,
            Self::Migrations { command } => match command {
                MigrationsCommand::Apply { name, dir } => cmd_apply(backend, &name, &dir).await,
                MigrationsCommand::Status { name, dir } => cmd_status(backend, &name, &dir).await,
            },
        }
    }
}

async fn cmd_list<B: Backend>(backend: &B) -> Result<(), BackendError> {
    let databases = backend.list_d1().await?;

    if databases.is_empty() {
        println!("No D1 databases found.");
        return Ok(());
    }

    println!("{}", "D1 Databases".bold());
    println!("{}", "─".repeat(60));

    for database in databases {
        let desc = database
            .description
            .as_deref()
            .map(|d| format!(" - {}", d).dimmed().to_string())
            .unwrap_or_default();

        println!("  {}{}", database.name.bold(), desc);
    }

    Ok(())
}

async fn cmd_get<B: Backend>(backend: &B, name: &str) -> Result<(), BackendError> {
    let database = backend.get_d1(name).await?;

    println!("{:12} {}", "Name:".dimmed(), database.name.bold());
    println!("{:12} {}", "ID:".dimmed(), database.id);

    if let Some(desc) = &database.description {
        println!("{:12} {}", "Description:".dimmed(), desc);
    }

    println!(
        "{:12} {}",
        "Created:".dimmed(),
        database.created_at.format("%Y-%m-%d %H:%M:%S")
    );

    Ok(())
}

async fn cmd_create<B: Backend>(
    backend: &B,
    name: String,
    description: Option<String>,
) -> Result<(), BackendError> {
    let input = CreateD1Input {
        name,
        desc: description,
    };

    let database = backend.create_d1(input).await?;

    println!(
        "{} D1 database '{}' created ({}).",
        "Created".green(),
        database.name.bold(),
        database.id.dimmed()
    );

    Ok(())
}

async fn cmd_delete<B: Backend>(backend: &B, name: &str) -> Result<(), BackendError> {
    backend.delete_d1(name).await?;

    println!("{} D1 database '{}' deleted.", "Deleted".red(), name.bold());

    Ok(())
}

async fn cmd_execute<B: Backend>(
    backend: &B,
    name: &str,
    command: Option<String>,
    file: Option<PathBuf>,
    as_json: bool,
) -> Result<(), BackendError> {
    let op = match (command, file) {
        (Some(sql), _) => json!({
            "kind": "query",
            "statement": { "sql": sql, "params": [] },
            "rows": "all",
        }),
        (None, Some(path)) => json!({ "kind": "exec", "sql": read_file(&path)? }),
        (None, None) => unreachable!("clap requires --command or --file"),
    };

    let result = backend
        .d1_request(name, json!({ "type": "op", "op": op }))
        .await?;

    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&result).unwrap_or_default()
        );
        return Ok(());
    }

    match result["type"].as_str() {
        Some("query") => print_rows(&result),
        Some("exec") => println!(
            "{} {} statements in {:.1} ms.",
            "Executed".green(),
            result["count"],
            result["duration"].as_f64().unwrap_or_default()
        ),
        _ => return Err(d1_error(&result)),
    }

    Ok(())
}

/// A local migration file and its checksum
struct Migration {
    name: String,
    sql: String,
    checksum: String,
}

/// An applied migration, from the `d1_migrations` table of the database
struct Applied {
    name: String,
    checksum: String,
}

fn read_migrations(dir: &Path) -> Result<Vec<Migration>, BackendError> {
    let entries = std::fs::read_dir(dir).map_err(|e| {
        BackendError::Api(format!(
            "cannot read migrations in {}: {}",
            dir.display(),
            e
        ))
    })?;

    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "sql"))
        .collect();

    paths.sort();

    paths
        .into_iter()
        .map(|path| {
            let sql = read_file(&path)?;
            let checksum = hex::encode(Sha256::digest(sql.as_bytes()));
            let name = path.file_name().unwrap().to_string_lossy().into_owned();

            Ok(Migration {
                name,
                sql,
                checksum,
            })
        })
        .collect()
}

async fn applied_migrations<B: Backend>(
    backend: &B,
    name: &str,
) -> Result<Vec<Applied>, BackendError> {
    let op = json!({
        "kind": "query",
        "statement": {
            "sql": "SELECT name, checksum FROM d1_migrations ORDER BY id",
            "params": [],
        },
        "rows": "all",
    });

    let result = backend
        .d1_request(name, json!({ "type": "op", "op": op }))
        .await?;

    if result["type"] == "error" {
        let missing_table = result["message"]
            .as_str()
            .is_some_and(|message| message.starts_with("no such table: d1_migrations"));

        if missing_table {
            return Ok(Vec::new());
        }

        return Err(d1_error(&result));
    }

    let rows = result["rows"].as_array().cloned().unwrap_or_default();

    Ok(rows
        .iter()
        .map(|row| Applied {
            name: row[0].as_str().unwrap_or_default().to_string(),
            checksum: row[1].as_str().unwrap_or_default().to_string(),
        })
        .collect())
}

/// Local migrations not applied yet, after checking that no applied one changed
fn pending<'a>(
    migrations: &'a [Migration],
    applied: &[Applied],
) -> Result<Vec<&'a Migration>, BackendError> {
    let mut pending = Vec::new();

    for migration in migrations {
        match applied.iter().find(|a| a.name == migration.name) {
            Some(a) if a.checksum != migration.checksum => {
                return Err(BackendError::Api(format!(
                    "migration {} changed after it was applied; add a new migration instead",
                    migration.name
                )));
            }
            Some(_) => {}
            None => pending.push(migration),
        }
    }

    Ok(pending)
}

async fn cmd_status<B: Backend>(backend: &B, name: &str, dir: &Path) -> Result<(), BackendError> {
    let migrations = read_migrations(dir)?;
    let applied = applied_migrations(backend, name).await?;

    println!("{}", format!("Migrations of '{}'", name).bold());
    println!("{}", "─".repeat(60));

    for migration in &migrations {
        let state = match applied.iter().find(|a| a.name == migration.name) {
            Some(a) if a.checksum != migration.checksum => "changed".red(),
            Some(_) => "applied".green(),
            None => "pending".yellow(),
        };

        println!("  {:8} {}", state, migration.name);
    }

    for a in applied
        .iter()
        .filter(|a| !migrations.iter().any(|m| m.name == a.name))
    {
        println!("  {:8} {}", "missing".red(), a.name);
    }

    Ok(())
}

async fn cmd_apply<B: Backend>(backend: &B, name: &str, dir: &Path) -> Result<(), BackendError> {
    let migrations = read_migrations(dir)?;
    let applied = applied_migrations(backend, name).await?;
    let pending = pending(&migrations, &applied)?;

    if pending.is_empty() {
        println!("No pending migrations.");
        return Ok(());
    }

    for migration in pending {
        let request = json!({
            "type": "migrate",
            "name": migration.name,
            "sql": migration.sql,
            "checksum": migration.checksum,
        });

        let result = backend.d1_request(name, request).await?;

        if result["type"] != "exec" {
            return Err(d1_error(&result));
        }

        println!(
            "{} {} ({} statements)",
            "Applied".green(),
            migration.name,
            result["count"]
        );
    }

    Ok(())
}

fn read_file(path: &Path) -> Result<String, BackendError> {
    std::fs::read_to_string(path)
        .map_err(|e| BackendError::Api(format!("cannot read {}: {}", path.display(), e)))
}

/// The error of a D1 result, as the guest would see it
fn d1_error(result: &Value) -> BackendError {
    let prefix = match result["kind"].as_str() {
        Some("type") => "D1_TYPE_ERROR",
        Some("range") => "D1_RANGE_ERROR",
        _ => "D1_ERROR",
    };

    let at = result["statement"]
        .as_u64()
        .map(|index| format!(" (statement {})", index + 1))
        .unwrap_or_default();

    BackendError::Api(format!(
        "{}: {}{}",
        prefix,
        result["message"].as_str().unwrap_or("unexpected result"),
        at
    ))
}

fn print_rows(result: &Value) {
    let columns: Vec<String> = result["columns"]
        .as_array()
        .map(|columns| {
            columns
                .iter()
                .map(|c| c.as_str().unwrap_or_default().to_string())
                .collect()
        })
        .unwrap_or_default();

    let rows: Vec<Vec<String>> = result["rows"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .map(|row| {
                    row.as_array()
                        .map(|values| values.iter().map(format_value).collect())
                        .unwrap_or_default()
                })
                .collect()
        })
        .unwrap_or_default();

    if !columns.is_empty() {
        let widths: Vec<usize> = columns
            .iter()
            .enumerate()
            .map(|(i, column)| {
                rows.iter()
                    .map(|row| row.get(i).map_or(0, |v| v.chars().count()))
                    .max()
                    .unwrap_or(0)
                    .max(column.chars().count())
            })
            .collect();

        let line = |values: &[String]| {
            values
                .iter()
                .zip(&widths)
                .map(|(value, width)| format!("{:width$}", value, width = width))
                .collect::<Vec<_>>()
                .join("  ")
        };

        println!("{}", line(&columns).bold());

        for row in &rows {
            println!("{}", line(row));
        }
    }

    let meta = &result["meta"];

    println!(
        "{}",
        format!(
            "{} rows, {} changes, {:.1} ms",
            rows.len(),
            meta["changes"],
            meta["duration"].as_f64().unwrap_or_default()
        )
        .dimmed()
    );
}

fn format_value(value: &Value) -> String {
    match value {
        Value::Null => "NULL".to_string(),
        Value::String(text) => text.clone(),
        Value::Object(object) => match object.get("$bytes").and_then(Value::as_str) {
            Some(encoded) => format!("<blob {} base64 chars>", encoded.len()),
            None => value.to_string(),
        },
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn migration(name: &str, checksum: &str) -> Migration {
        Migration {
            name: name.to_string(),
            sql: String::new(),
            checksum: checksum.to_string(),
        }
    }

    fn applied(name: &str, checksum: &str) -> Applied {
        Applied {
            name: name.to_string(),
            checksum: checksum.to_string(),
        }
    }

    #[test]
    fn pending_skips_applied_migrations() {
        let local = [migration("0001.sql", "a"), migration("0002.sql", "b")];
        let pending = pending(&local, &[applied("0001.sql", "a")]).unwrap();

        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].name, "0002.sql");
    }

    #[test]
    fn pending_refuses_a_changed_migration() {
        let local = [migration("0001.sql", "changed")];
        let err = pending(&local, &[applied("0001.sql", "a")]).err().unwrap();

        assert!(err.to_string().contains("0001.sql changed"), "{err}");
    }

    #[test]
    fn errors_carry_the_d1_prefix_and_statement() {
        let result = json!({ "type": "error", "kind": "sql", "message": "no such table: t: SQLITE_ERROR", "statement": 1 });

        assert_eq!(
            d1_error(&result).to_string(),
            "API error: D1_ERROR: no such table: t: SQLITE_ERROR (statement 2)"
        );
    }

    #[test]
    fn blobs_print_as_a_size() {
        assert_eq!(
            format_value(&json!({ "$bytes": "AQID" })),
            "<blob 4 base64 chars>"
        );
        assert_eq!(format_value(&Value::Null), "NULL");
        assert_eq!(format_value(&json!(1.5)), "1.5");
    }
}
