use crate::config::{AliasConfig, Config, ConfigError};
use clap::Subcommand;
use colored::Colorize;

#[derive(Subcommand)]
pub enum AliasCommand {
    /// Configure a new alias for API or direct database access
    #[command(after_help = "Examples:\n  \
        ow alias set prod --api https://dash.openworkers.com\n  \
        ow alias set local --db postgres://localhost/ow --user admin@example.com\n  \
        ow alias set dev --api https://localhost:8080 --insecure")]
    Set {
        /// Alias name (used as prefix: ow <alias> workers list)
        name: String,

        /// API URL for HTTP backend (e.g., https://dash.openworkers.com)
        #[arg(long, conflicts_with = "db")]
        api: Option<String>,

        /// API token (obtained via ow login)
        #[arg(long, requires = "api")]
        token: Option<String>,

        /// Accept invalid TLS certificates (for local development)
        #[arg(long, requires = "api")]
        insecure: bool,

        /// PostgreSQL URL for direct database access
        #[arg(long, conflicts_with = "api")]
        db: Option<String>,

        /// User email to operate as (required for db backend)
        #[arg(long, requires = "db")]
        user: Option<String>,

        /// Overwrite existing alias without confirmation
        #[arg(short, long)]
        force: bool,
    },

    /// List all configured aliases (* = default)
    #[command(alias = "ls")]
    List,

    /// Remove an alias from configuration
    #[command(alias = "rm", after_help = "Example:\n  ow alias remove old-prod")]
    Remove {
        /// Alias name to remove
        name: String,
    },

    /// Set the default alias (used when no alias prefix is given)
    #[command(after_help = "Example:\n  ow alias set-default prod")]
    SetDefault {
        /// Alias name to set as default
        name: String,
    },
}

impl AliasCommand {
    pub fn run(self) -> Result<(), ConfigError> {
        match self {
            Self::Set {
                name,
                api,
                token,
                insecure,
                db,
                user,
                force,
            } => cmd_set(name, api, token, insecure, db, user, force),
            Self::List => cmd_list(),
            Self::Remove { name } => cmd_remove(name),
            Self::SetDefault { name } => cmd_set_default(name),
        }
    }
}

fn cmd_set(
    name: String,
    api: Option<String>,
    token: Option<String>,
    insecure: bool,
    db: Option<String>,
    user: Option<String>,
    force: bool,
) -> Result<(), ConfigError> {
    let mut config = Config::load()?;

    let alias_config = match (api, db) {
        (Some(url), None) => AliasConfig::api(url, token, insecure),
        (None, Some(database_url)) => AliasConfig::db(database_url, user, None),
        _ => {
            eprintln!(
                "{} Either --api or --db must be specified",
                "error:".red().bold()
            );
            std::process::exit(1);
        }
    };

    let is_update = config.aliases.contains_key(&name);
    config.set_alias(&name, alias_config.clone(), force)?;
    config.save()?;

    let action = if is_update { "Updated" } else { "Added" };
    let type_name = alias_config.type_name();

    println!(
        "{} {} alias '{}' ({})",
        action,
        type_name.cyan(),
        name.green().bold(),
        match alias_config {
            AliasConfig::Api { url, .. } => url,
            AliasConfig::Db { database_url, .. } => mask_password(&database_url),
        }
    );

    Ok(())
}

fn cmd_list() -> Result<(), ConfigError> {
    let config = Config::load()?;

    if config.aliases.is_empty() {
        println!("No aliases configured.");
        println!(
            "Run '{}' to add one.",
            "ow alias set <name> --api <url>".cyan()
        );
        return Ok(());
    }

    let default = config.default.as_deref();

    for (name, alias) in &config.aliases {
        let is_default = default == Some(name.as_str());
        let marker = if is_default {
            "*".green().bold().to_string()
        } else {
            " ".to_string()
        };

        let (type_str, detail) = match alias {
            AliasConfig::Api { url, token, .. } => {
                let auth = if token.is_some() { " (auth)" } else { "" };
                ("api".cyan(), format!("{}{}", url, auth.dimmed()))
            }
            AliasConfig::Db {
                database_url,
                user,
                storage,
            } => {
                let user_info = user
                    .as_ref()
                    .map(|u| format!(" @{}", u))
                    .unwrap_or_default();
                let storage_info = if storage.is_some() { " (storage)" } else { "" };
                (
                    "db".yellow(),
                    format!(
                        "{}{}{}",
                        mask_password(database_url),
                        user_info.cyan(),
                        storage_info.dimmed()
                    ),
                )
            }
        };

        println!(
            "{} {:12} {:4} {}",
            marker,
            name.bold(),
            type_str,
            detail.dimmed()
        );
    }

    if default.is_some() {
        println!();
        println!("{}", "* = default".dimmed());
    }

    Ok(())
}

fn cmd_remove(name: String) -> Result<(), ConfigError> {
    let mut config = Config::load()?;

    config.remove_alias(&name)?;
    config.save()?;

    println!("Removed alias '{}'", name.red().bold());

    Ok(())
}

fn cmd_set_default(name: String) -> Result<(), ConfigError> {
    let mut config = Config::load()?;

    config.set_default(&name)?;
    config.save()?;

    println!("Default alias set to '{}'", name.green().bold());

    Ok(())
}

/// Mask password in database URL for display
fn mask_password(url: &str) -> String {
    // postgres://user:password@host/db -> postgres://user:***@host/db
    // Use rfind to handle passwords containing @
    if let Some(scheme_end) = url.find("://") {
        let after_scheme = &url[scheme_end + 3..];

        // Find the last @ which separates credentials from host
        if let Some(at_pos) = after_scheme.rfind('@') {
            let credentials = &after_scheme[..at_pos];

            // Find the first : which separates user from password
            if let Some(colon_pos) = credentials.find(':') {
                let user = &credentials[..colon_pos];
                let host_and_rest = &after_scheme[at_pos..];

                return format!("{}://{}:***{}", &url[..scheme_end], user, host_and_rest);
            }
        }
    }

    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_password() {
        assert_eq!(
            mask_password("postgres://user:secret@host/db"),
            "postgres://user:***@host/db"
        );
        assert_eq!(
            mask_password("postgres://admin:p@ssw0rd@localhost:5432/openworkers"),
            "postgres://admin:***@localhost:5432/openworkers"
        );
        // No password
        assert_eq!(mask_password("postgres://host/db"), "postgres://host/db");
    }
}
