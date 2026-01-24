use crate::backend::{Backend, BackendError, CreateKvInput};
use clap::Subcommand;
use colored::Colorize;

#[derive(Subcommand)]
pub enum KvCommand {
    /// List all KV namespaces
    #[command(alias = "ls")]
    List,

    /// Show KV namespace details
    #[command(after_help = "Example:\n  ow kv get my-cache")]
    Get {
        /// KV namespace name
        name: String,
    },

    /// Create a new KV namespace for key-value storage
    #[command(after_help = "Examples:\n  \
        ow kv create my-cache\n  \
        ow kv create sessions -d \"User sessions\"")]
    Create {
        /// KV namespace name
        name: String,

        /// Description of what this namespace stores
        #[arg(short, long)]
        description: Option<String>,
    },

    /// Delete a KV namespace and all its data
    #[command(alias = "rm", after_help = "Example:\n  ow kv delete old-cache")]
    Delete {
        /// KV namespace name to delete
        name: String,
    },
}

impl KvCommand {
    pub async fn run<B: Backend>(self, backend: &B) -> Result<(), BackendError> {
        match self {
            Self::List => cmd_list(backend).await,
            Self::Get { name } => cmd_get(backend, &name).await,
            Self::Create { name, description } => cmd_create(backend, name, description).await,
            Self::Delete { name } => cmd_delete(backend, &name).await,
        }
    }
}

async fn cmd_list<B: Backend>(backend: &B) -> Result<(), BackendError> {
    let namespaces = backend.list_kv().await?;

    if namespaces.is_empty() {
        println!("No KV namespaces found.");
        return Ok(());
    }

    println!("{}", "KV Namespaces".bold());
    println!("{}", "─".repeat(60));

    for ns in namespaces {
        let desc = ns
            .description
            .as_deref()
            .map(|d| format!(" - {}", d).dimmed().to_string())
            .unwrap_or_default();

        println!("  {}{}", ns.name.bold(), desc);
    }

    Ok(())
}

async fn cmd_get<B: Backend>(backend: &B, name: &str) -> Result<(), BackendError> {
    let ns = backend.get_kv(name).await?;

    println!("{:12} {}", "Name:".dimmed(), ns.name.bold());
    println!("{:12} {}", "ID:".dimmed(), ns.id);

    if let Some(desc) = &ns.description {
        println!("{:12} {}", "Description:".dimmed(), desc);
    }

    println!(
        "{:12} {}",
        "Created:".dimmed(),
        ns.created_at.format("%Y-%m-%d %H:%M:%S")
    );

    println!(
        "{:12} {}",
        "Updated:".dimmed(),
        ns.updated_at.format("%Y-%m-%d %H:%M:%S")
    );

    Ok(())
}

async fn cmd_create<B: Backend>(
    backend: &B,
    name: String,
    description: Option<String>,
) -> Result<(), BackendError> {
    let input = CreateKvInput {
        name,
        desc: description,
    };

    let ns = backend.create_kv(input).await?;

    println!(
        "{} KV namespace '{}' created.",
        "Created".green(),
        ns.name.bold()
    );

    Ok(())
}

async fn cmd_delete<B: Backend>(backend: &B, name: &str) -> Result<(), BackendError> {
    backend.delete_kv(name).await?;

    println!(
        "{} KV namespace '{}' deleted.",
        "Deleted".red(),
        name.bold()
    );

    Ok(())
}
