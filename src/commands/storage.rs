use crate::backend::{Backend, BackendError, CreateStorageInput};
use clap::Subcommand;
use colored::Colorize;

#[derive(Subcommand)]
pub enum StorageCommand {
    /// List all storage configs
    #[command(alias = "ls")]
    List,

    /// Get storage config details
    Get {
        /// Storage name
        name: String,
    },

    /// Create a new storage config
    Create {
        /// Storage name
        name: String,

        /// Provider: platform (default) or s3
        #[arg(long, default_value = "platform")]
        provider: String,

        /// S3 bucket name (required for s3 provider)
        #[arg(long)]
        bucket: Option<String>,

        /// S3 access key ID (required for s3 provider)
        #[arg(long)]
        access_key_id: Option<String>,

        /// S3 secret access key (required for s3 provider)
        #[arg(long)]
        secret_access_key: Option<String>,

        /// S3 endpoint URL
        #[arg(long)]
        endpoint: Option<String>,

        /// S3 region
        #[arg(long)]
        region: Option<String>,

        /// Key prefix
        #[arg(long)]
        prefix: Option<String>,

        /// Public URL for assets
        #[arg(long)]
        public_url: Option<String>,

        /// Description
        #[arg(short, long)]
        description: Option<String>,
    },

    /// Delete a storage config
    #[command(alias = "rm")]
    Delete {
        /// Storage name
        name: String,
    },
}

impl StorageCommand {
    pub async fn run<B: Backend>(self, backend: &B) -> Result<(), BackendError> {
        match self {
            Self::List => cmd_list(backend).await,
            Self::Get { name } => cmd_get(backend, &name).await,
            Self::Create {
                name,
                provider,
                bucket,
                access_key_id,
                secret_access_key,
                endpoint,
                region,
                prefix,
                public_url,
                description,
            } => {
                cmd_create(
                    backend,
                    name,
                    provider,
                    bucket,
                    access_key_id,
                    secret_access_key,
                    endpoint,
                    region,
                    prefix,
                    public_url,
                    description,
                )
                .await
            }
            Self::Delete { name } => cmd_delete(backend, &name).await,
        }
    }
}

async fn cmd_list<B: Backend>(backend: &B) -> Result<(), BackendError> {
    let configs = backend.list_storage().await?;

    if configs.is_empty() {
        println!("No storage configs found.");
        return Ok(());
    }

    println!("{}", "Storage Configs".bold());
    println!("{}", "─".repeat(60));

    for config in configs {
        let provider_badge = match config.provider.as_str() {
            "platform" => "[platform]".cyan(),
            "s3" => "[s3]".yellow(),
            _ => format!("[{}]", config.provider).dimmed(),
        };

        println!("  {} {:30}", provider_badge, config.name.bold());
    }

    Ok(())
}

async fn cmd_get<B: Backend>(backend: &B, name: &str) -> Result<(), BackendError> {
    let config = backend.get_storage(name).await?;

    println!("{:12} {}", "Name:".dimmed(), config.name.bold());
    println!("{:12} {}", "ID:".dimmed(), config.id);
    println!("{:12} {}", "Provider:".dimmed(), config.provider);

    if let Some(desc) = &config.description {
        println!("{:12} {}", "Description:".dimmed(), desc);
    }

    if config.provider == "s3" {
        if let Some(bucket) = &config.bucket {
            println!("{:12} {}", "Bucket:".dimmed(), bucket);
        }

        if let Some(endpoint) = &config.endpoint {
            println!("{:12} {}", "Endpoint:".dimmed(), endpoint);
        }

        if let Some(region) = &config.region {
            println!("{:12} {}", "Region:".dimmed(), region);
        }

        if let Some(prefix) = &config.prefix {
            println!("{:12} {}", "Prefix:".dimmed(), prefix);
        }

        if let Some(public_url) = &config.public_url {
            println!("{:12} {}", "Public URL:".dimmed(), public_url);
        }
    }

    println!(
        "{:12} {}",
        "Created:".dimmed(),
        config.created_at.format("%Y-%m-%d %H:%M:%S")
    );

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn cmd_create<B: Backend>(
    backend: &B,
    name: String,
    provider: String,
    bucket: Option<String>,
    access_key_id: Option<String>,
    secret_access_key: Option<String>,
    endpoint: Option<String>,
    region: Option<String>,
    prefix: Option<String>,
    public_url: Option<String>,
    description: Option<String>,
) -> Result<(), BackendError> {
    if provider == "s3" {
        if bucket.is_none() {
            return Err(BackendError::Api(
                "--bucket is required for s3 provider".to_string(),
            ));
        }

        if access_key_id.is_none() {
            return Err(BackendError::Api(
                "--access-key-id is required for s3 provider".to_string(),
            ));
        }

        if secret_access_key.is_none() {
            return Err(BackendError::Api(
                "--secret-access-key is required for s3 provider".to_string(),
            ));
        }
    }

    let input = CreateStorageInput {
        name,
        desc: description,
        provider: provider.clone(),
        bucket,
        prefix,
        access_key_id,
        secret_access_key,
        endpoint,
        region,
        public_url,
    };

    let config = backend.create_storage(input).await?;

    println!(
        "{} Storage '{}' created ({} provider).",
        "Created".green(),
        config.name.bold(),
        provider
    );

    Ok(())
}

async fn cmd_delete<B: Backend>(backend: &B, name: &str) -> Result<(), BackendError> {
    backend.delete_storage(name).await?;

    println!("{} Storage '{}' deleted.", "Deleted".red(), name.bold());

    Ok(())
}
