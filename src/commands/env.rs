use crate::backend::{
    Backend, BackendError, CreateEnvironmentInput, EnvironmentValueInput, UpdateEnvironmentInput,
};
use clap::Subcommand;
use colored::Colorize;

#[derive(Subcommand)]
pub enum EnvCommand {
    /// List all environments
    #[command(alias = "ls")]
    List,

    /// Get environment details
    Get {
        /// Environment name
        name: String,
    },

    /// Create a new environment
    Create {
        /// Environment name
        name: String,

        /// Environment description
        #[arg(short, long)]
        description: Option<String>,
    },

    /// Delete an environment
    #[command(alias = "rm")]
    Delete {
        /// Environment name
        name: String,
    },

    /// Set a variable or secret
    Set {
        /// Environment name
        env: String,

        /// Variable key
        key: String,

        /// Variable value
        value: String,

        /// Mark as secret (value will be masked)
        #[arg(short, long)]
        secret: bool,
    },

    /// Remove a variable or secret
    Unset {
        /// Environment name
        env: String,

        /// Variable key
        key: String,
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
            } => cmd_set(backend, &env, &key, &value, secret).await,
            Self::Unset { env, key } => cmd_unset(backend, &env, &key).await,
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
        let vars_count = env.values.len();
        let secrets_count = env
            .values
            .iter()
            .filter(|v| v.value_type == "secret")
            .count();

        println!(
            "  {:30} {} vars, {} secrets",
            env.name.bold(),
            vars_count - secrets_count,
            secrets_count
        );
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
        println!("{}", "Variables".bold());
        println!("{}", "─".repeat(40));

        for val in &env.values {
            let type_badge = if val.value_type == "secret" {
                "[secret]".yellow()
            } else {
                "[var]".dimmed()
            };

            println!("  {} {} = {}", type_badge, val.key.bold(), val.value);
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
