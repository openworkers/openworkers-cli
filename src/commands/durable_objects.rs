use crate::backend::{Backend, BackendError, CreateDurableObjectInput};
use clap::Subcommand;
use colored::Colorize;

#[derive(Subcommand)]
pub enum DurableObjectsCommand {
    /// List the Durable Object namespaces
    #[command(alias = "ls")]
    List,

    /// Show a Durable Object namespace
    #[command(alias = "info", after_help = "Example:\n  ow do get counters")]
    Get {
        /// Namespace name
        name: String,
    },

    /// Create a namespace for a class that a worker exports
    #[command(
        after_help = "The worker exports the class: export class Counter { ... }\n\
        Bind the namespace with: ow env bind <env> COUNTER counters -t durable_object\n\n\
        Example:\n  ow do create counters --worker my-app --class Counter"
    )]
    Create {
        /// Namespace name
        name: String,

        /// Name or ID of the worker that exports the class
        #[arg(short, long)]
        worker: String,

        /// The name the worker exports the class under
        #[arg(short, long = "class")]
        class_name: String,

        /// Description of what the objects do
        #[arg(short, long)]
        description: Option<String>,
    },

    /// Delete a namespace and the storage of all its objects
    #[command(alias = "rm", after_help = "Example:\n  ow do delete counters")]
    Delete {
        /// Namespace name
        name: String,
    },
}

impl DurableObjectsCommand {
    pub async fn run<B: Backend>(self, backend: &B) -> Result<(), BackendError> {
        match self {
            Self::List => cmd_list(backend).await,
            Self::Get { name } => cmd_get(backend, &name).await,
            Self::Create {
                name,
                worker,
                class_name,
                description,
            } => {
                let input = CreateDurableObjectInput {
                    name,
                    desc: description,
                    worker,
                    class_name,
                };

                cmd_create(backend, input).await
            }
            Self::Delete { name } => cmd_delete(backend, &name).await,
        }
    }
}

/// The worker and class of a namespace, as `worker.Class`
fn source(worker_name: Option<&str>, class_name: &str) -> String {
    format!(
        "{}.{}",
        worker_name.unwrap_or("(deleted worker)"),
        class_name
    )
}

async fn cmd_list<B: Backend>(backend: &B) -> Result<(), BackendError> {
    let namespaces = backend.list_durable_objects().await?;

    if namespaces.is_empty() {
        println!("No Durable Object namespaces found.");
        return Ok(());
    }

    println!("{}", "Durable Object namespaces".bold());
    println!("{}", "-".repeat(60));

    for namespace in namespaces {
        let class = source(namespace.worker_name.as_deref(), &namespace.class_name);

        println!("  {} {}", namespace.name.bold(), class.dimmed());
    }

    Ok(())
}

async fn cmd_get<B: Backend>(backend: &B, name: &str) -> Result<(), BackendError> {
    let namespace = backend.get_durable_object(name).await?;

    println!("{:12} {}", "Name:".dimmed(), namespace.name.bold());
    println!("{:12} {}", "ID:".dimmed(), namespace.id);
    println!(
        "{:12} {}",
        "Class:".dimmed(),
        source(namespace.worker_name.as_deref(), &namespace.class_name)
    );

    if let Some(desc) = &namespace.description {
        println!("{:12} {}", "Description:".dimmed(), desc);
    }

    println!(
        "{:12} {}",
        "Created:".dimmed(),
        namespace.created_at.format("%Y-%m-%d %H:%M:%S")
    );

    Ok(())
}

async fn cmd_create<B: Backend>(
    backend: &B,
    input: CreateDurableObjectInput,
) -> Result<(), BackendError> {
    let namespace = backend.create_durable_object(input).await?;

    println!(
        "{} Durable Object namespace '{}' created for {} ({}).",
        "Created".green(),
        namespace.name.bold(),
        source(namespace.worker_name.as_deref(), &namespace.class_name),
        namespace.id.dimmed()
    );

    Ok(())
}

async fn cmd_delete<B: Backend>(backend: &B, name: &str) -> Result<(), BackendError> {
    backend.delete_durable_object(name).await?;

    println!(
        "{} Durable Object namespace '{}' deleted; the runner removes its storage.",
        "Deleted".red(),
        name.bold()
    );

    Ok(())
}
