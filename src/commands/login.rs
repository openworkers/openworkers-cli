use crate::config::{AliasConfig, Config, ConfigError};
use colored::Colorize;
use std::io::{self, Write};

pub fn run(alias_name: &str) -> Result<(), ConfigError> {
    let mut config = Config::load()?;

    // Get existing alias
    let existing = config
        .get_alias(alias_name)
        .ok_or_else(|| ConfigError::AliasNotFound(alias_name.to_string()))?;

    // Must be an API alias
    let (url, insecure) = match existing {
        AliasConfig::Api { url, insecure, .. } => (url.clone(), *insecure),
        AliasConfig::Db { .. } => {
            eprintln!(
                "{} Alias '{}' is a database alias, not an API alias.",
                "Error:".red(),
                alias_name
            );
            return Ok(());
        }
    };

    // Prompt for token
    println!(
        "Logging into {} ({})",
        alias_name.cyan().bold(),
        url.dimmed()
    );
    print!("Enter API token: ");
    io::stdout().flush().unwrap();

    let mut token = String::new();
    io::stdin().read_line(&mut token).unwrap();
    let token = token.trim().to_string();

    if token.is_empty() {
        eprintln!("{} Token cannot be empty.", "Error:".red());
        return Ok(());
    }

    // Update alias with token
    config.set_alias(
        alias_name,
        AliasConfig::api(url, Some(token), insecure),
        true,
    )?;

    config.save()?;

    println!(
        "{} Token saved for alias '{}'.",
        "Success:".green(),
        alias_name.bold()
    );

    Ok(())
}
