use crate::backend::{
    Backend, BackendError, CreateEnvironmentInput, EnvironmentValueInput, UpdateEnvironmentInput,
};
use clap::Subcommand;
use colored::Colorize;
use std::io::{self, Write};

#[derive(Subcommand)]
pub enum EnvCommand {
    /// List all environments with their variable/binding counts
    #[command(alias = "ls")]
    List,

    /// Show environment details including all variables and bindings
    #[command(after_help = "Example:\n  ow env get production")]
    Get {
        /// Environment name
        name: String,
    },

    /// Create a new environment for organizing variables and bindings
    #[command(after_help = "Examples:\n  \
        ow env create production\n  \
        ow env create staging -d \"Staging environment\"")]
    Create {
        /// Environment name
        name: String,

        /// Description of this environment's purpose
        #[arg(short, long)]
        description: Option<String>,
    },

    /// Delete an environment and all its variables/bindings
    #[command(alias = "rm", after_help = "Example:\n  ow env delete old-env")]
    Delete {
        /// Environment name to delete
        name: String,
    },

    /// Set a variable or secret in an environment
    #[command(after_help = "Examples:\n  \
        ow env set prod API_URL https://api.example.com\n  \
        ow env set prod API_KEY --secret\n  \
        ow env set prod DB_URL")]
    Set {
        /// Environment name
        env: String,

        /// Variable name (conventionally UPPER_SNAKE_CASE)
        key: String,

        /// Variable value (prompted interactively if omitted, masked for secrets)
        value: Option<String>,

        /// Store as secret (value is encrypted and masked in output)
        #[arg(short, long)]
        secret: bool,
    },

    /// Remove a variable or secret from an environment
    #[command(after_help = "Example:\n  ow env unset prod OLD_API_KEY")]
    Unset {
        /// Environment name
        env: String,

        /// Variable name to remove
        key: String,
    },

    /// Bind a resource (KV, database, storage) to an environment
    #[command(after_help = "Examples:\n  \
        ow env bind prod KV my-cache --type kv\n  \
        ow env bind prod DB my-database --type database\n  \
        ow env bind prod ASSETS my-storage --type assets\n  \
        ow env bind prod FILES my-storage --type storage")]
    Bind {
        /// Environment name
        env: String,

        /// Binding name (accessed as env.NAME in worker code)
        key: String,

        /// Resource name to bind (must exist)
        resource: String,

        /// Binding type: assets, storage, kv, or database
        #[arg(short = 't', long = "type", value_parser = ["assets", "storage", "kv", "database"])]
        binding_type: String,
    },
}

impl EnvCommand {
    pub async fn run<B: Backend>(self, backend: &B) -> Result<(), BackendError> {
        match self {
            Self::List => cmd_list(backend).await,
            Self::Get { name } => cmd_get(backend, &name).await,
            Self::Create { name, description } => cmd_create(backend, name, description).await,
            Self::Delete { name } => cmd_delete(backend, &name).await,
            Self::Set {
                env,
                key,
                value,
                secret,
            } => {
                let value = match value {
                    Some(v) => v,
                    None if secret => {
                        eprint!("{}: ", "Enter secret value".dimmed());
                        io::stderr().flush().ok();
                        rpassword::read_password().map_err(|e| {
                            BackendError::Api(format!("Failed to read input: {}", e))
                        })?
                    }
                    None => {
                        eprint!("{}: ", "Enter value".dimmed());
                        io::stderr().flush().ok();
                        let mut buf = String::new();
                        io::stdin().read_line(&mut buf).map_err(|e| {
                            BackendError::Api(format!("Failed to read input: {}", e))
                        })?;
                        buf.trim_end().to_string()
                    }
                };

                cmd_set(backend, &env, &key, &value, secret).await
            }
            Self::Unset { env, key } => cmd_unset(backend, &env, &key).await,
            Self::Bind {
                env,
                key,
                resource,
                binding_type,
            } => cmd_bind(backend, &env, &key, &resource, &binding_type).await,
        }
    }
}

async fn cmd_list<B: Backend>(backend: &B) -> Result<(), BackendError> {
    let environments = backend.list_environments().await?;

    if environments.is_empty() {
        println!("No environments found.");
        return Ok(());
    }

    println!("{}", "Environments".bold());
    println!("{}", "─".repeat(60));

    for env in environments {
        let vars_count = env.values.iter().filter(|v| v.value_type == "var").count();
        let secrets_count = env
            .values
            .iter()
            .filter(|v| v.value_type == "secret")
            .count();
        let bindings_count = env
            .values
            .iter()
            .filter(|v| !matches!(v.value_type.as_str(), "var" | "secret"))
            .count();

        let mut parts = Vec::new();

        if vars_count > 0 {
            parts.push(format!("{} vars", vars_count));
        }

        if secrets_count > 0 {
            parts.push(format!("{} secrets", secrets_count));
        }

        if bindings_count > 0 {
            parts.push(format!("{} bindings", bindings_count));
        }

        let summary = if parts.is_empty() {
            "empty".dimmed().to_string()
        } else {
            parts.join(", ")
        };

        println!("  {:30} {}", env.name.bold(), summary);
    }

    Ok(())
}

async fn cmd_get<B: Backend>(backend: &B, name: &str) -> Result<(), BackendError> {
    let env = backend.get_environment(name).await?;

    println!("{:12} {}", "Name:".dimmed(), env.name.bold());
    println!("{:12} {}", "ID:".dimmed(), env.id);

    if let Some(desc) = &env.description {
        println!("{:12} {}", "Description:".dimmed(), desc);
    }

    println!(
        "{:12} {}",
        "Created:".dimmed(),
        env.created_at.format("%Y-%m-%d %H:%M:%S")
    );

    println!(
        "{:12} {}",
        "Updated:".dimmed(),
        env.updated_at.format("%Y-%m-%d %H:%M:%S")
    );

    if !env.values.is_empty() {
        println!();
        println!("{}", "Bindings".bold());
        println!("{}", "─".repeat(40));

        for val in &env.values {
            let type_badge = match val.value_type.as_str() {
                "secret" => "[secret]".yellow(),
                "var" => "[var]".dimmed(),
                "kv" => "[kv]".cyan(),
                "assets" => "[assets]".green(),
                "storage" => "[storage]".blue(),
                "database" => "[database]".magenta(),
                _ => format!("[{}]", val.value_type).dimmed(),
            };

            let display_value = if val.value_type == "secret" {
                "****".to_string()
            } else {
                val.value.clone()
            };

            println!("  {} {} = {}", type_badge, val.key.bold(), display_value);
        }
    }

    Ok(())
}

async fn cmd_create<B: Backend>(
    backend: &B,
    name: String,
    description: Option<String>,
) -> Result<(), BackendError> {
    let input = CreateEnvironmentInput {
        name,
        desc: description,
    };

    let env = backend.create_environment(input).await?;

    println!(
        "{} Environment '{}' created.",
        "Created".green(),
        env.name.bold()
    );

    Ok(())
}

async fn cmd_delete<B: Backend>(backend: &B, name: &str) -> Result<(), BackendError> {
    backend.delete_environment(name).await?;

    println!("{} Environment '{}' deleted.", "Deleted".red(), name.bold());

    Ok(())
}

async fn cmd_set<B: Backend>(
    backend: &B,
    env_name: &str,
    key: &str,
    value: &str,
    secret: bool,
) -> Result<(), BackendError> {
    // Get current environment to find existing value ID
    let env = backend.get_environment(env_name).await?;

    let existing_id = env
        .values
        .iter()
        .find(|v| v.key == key)
        .map(|v| v.id.clone());

    let value_input = EnvironmentValueInput {
        id: existing_id,
        key: key.to_string(),
        value: Some(value.to_string()),
        value_type: if secret {
            "secret".to_string()
        } else {
            "var".to_string()
        },
    };

    let input = UpdateEnvironmentInput {
        name: None,
        values: Some(vec![value_input]),
    };

    backend.update_environment(env_name, input).await?;

    let type_str = if secret { "Secret" } else { "Variable" };
    println!(
        "{} {} '{}' set in environment '{}'.",
        "Updated".green(),
        type_str,
        key.bold(),
        env_name.bold()
    );

    Ok(())
}

async fn cmd_unset<B: Backend>(backend: &B, env_name: &str, key: &str) -> Result<(), BackendError> {
    // Get current environment to find existing value ID
    let env = backend.get_environment(env_name).await?;

    let existing = env.values.iter().find(|v| v.key == key);

    match existing {
        Some(val) => {
            let value_input = EnvironmentValueInput {
                id: Some(val.id.clone()),
                key: key.to_string(),
                value: None, // Setting value to null deletes it
                value_type: val.value_type.clone(),
            };

            let input = UpdateEnvironmentInput {
                name: None,
                values: Some(vec![value_input]),
            };

            backend.update_environment(env_name, input).await?;

            println!(
                "{} Variable '{}' removed from environment '{}'.",
                "Removed".red(),
                key.bold(),
                env_name.bold()
            );
        }
        None => {
            return Err(BackendError::NotFound(format!(
                "Variable '{}' not found in environment '{}'",
                key, env_name
            )));
        }
    }

    Ok(())
}

async fn cmd_bind<B: Backend>(
    backend: &B,
    env_name: &str,
    key: &str,
    resource: &str,
    binding_type: &str,
) -> Result<(), BackendError> {
    // Get resource ID based on type
    let resource_id = match binding_type {
        "assets" | "storage" => {
            let storage = backend.get_storage(resource).await?;
            storage.id
        }
        "kv" => {
            let kv = backend.get_kv(resource).await?;
            kv.id
        }
        "database" => {
            let db = backend.get_database(resource).await?;
            db.id
        }
        _ => {
            return Err(BackendError::Api(format!(
                "Unknown binding type: {}",
                binding_type
            )));
        }
    };

    // Get current environment to find existing binding
    let env = backend.get_environment(env_name).await?;

    let existing_id = env
        .values
        .iter()
        .find(|v| v.key == key)
        .map(|v| v.id.clone());

    let value_input = EnvironmentValueInput {
        id: existing_id,
        key: key.to_string(),
        value: Some(resource_id),
        value_type: binding_type.to_string(),
    };

    let input = UpdateEnvironmentInput {
        name: None,
        values: Some(vec![value_input]),
    };

    backend.update_environment(env_name, input).await?;

    println!(
        "{} Binding '{}' ({}) added to environment '{}'.",
        "Bound".green(),
        key.bold(),
        binding_type,
        env_name.bold()
    );

    Ok(())
}
