use crate::backend::{Backend, BackendError, CreateWorkerInput, DeployInput, Worker};
use clap::Subcommand;
use colored::Colorize;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum WorkersCommand {
    /// List all workers
    #[command(alias = "ls")]
    List,

    /// Get worker details
    Get {
        /// Worker name
        name: String,
    },

    /// Create a new worker
    Create {
        /// Worker name
        name: String,

        /// Worker description
        #[arg(short, long)]
        description: Option<String>,
    },

    /// Delete a worker
    #[command(alias = "rm")]
    Delete {
        /// Worker name
        name: String,
    },

    /// Deploy code to a worker
    Deploy {
        /// Worker name
        name: String,

        /// Path to the source file (.js, .ts, or .wasm)
        file: PathBuf,

        /// Deployment message
        #[arg(short, long)]
        message: Option<String>,
    },
}

impl WorkersCommand {
    pub async fn run<B: Backend>(self, backend: &B) -> Result<(), BackendError> {
        match self {
            Self::List => cmd_list(backend).await,
            Self::Get { name } => cmd_get(backend, &name).await,
            Self::Create { name, description } => cmd_create(backend, name, description).await,
            Self::Delete { name } => cmd_delete(backend, &name).await,
            Self::Deploy {
                name,
                file,
                message,
            } => cmd_deploy(backend, &name, file, message).await,
        }
    }
}

async fn cmd_list<B: Backend>(backend: &B) -> Result<(), BackendError> {
    let workers = backend.list_workers().await?;

    if workers.is_empty() {
        println!("No workers found.");
        return Ok(());
    }

    println!("{}", "Workers".bold());
    println!("{}", "─".repeat(60));

    for worker in workers {
        let version = worker
            .current_version
            .map(|v| format!("v{}", v))
            .unwrap_or_else(|| "no deploy".dimmed().to_string());

        println!(
            "  {:30} {:10} {}",
            worker.name.bold(),
            version,
            worker.description.as_deref().unwrap_or("").dimmed()
        );
    }

    Ok(())
}

async fn cmd_get<B: Backend>(backend: &B, name: &str) -> Result<(), BackendError> {
    let worker = backend.get_worker(name).await?;

    print_worker(&worker);

    Ok(())
}

async fn cmd_create<B: Backend>(
    backend: &B,
    name: String,
    description: Option<String>,
) -> Result<(), BackendError> {
    let input = CreateWorkerInput { name, description };
    let worker = backend.create_worker(input).await?;

    println!(
        "{} Worker '{}' created.",
        "Created".green(),
        worker.name.bold()
    );
    println!();

    print_worker(&worker);

    Ok(())
}

async fn cmd_delete<B: Backend>(backend: &B, name: &str) -> Result<(), BackendError> {
    backend.delete_worker(name).await?;

    println!("{} Worker '{}' deleted.", "Deleted".red(), name.bold());

    Ok(())
}

fn print_worker(worker: &Worker) {
    println!("{:12} {}", "Name:".dimmed(), worker.name.bold());
    println!("{:12} {}", "ID:".dimmed(), worker.id);

    if let Some(desc) = &worker.description {
        println!("{:12} {}", "Description:".dimmed(), desc);
    }

    println!(
        "{:12} {}",
        "Version:".dimmed(),
        worker
            .current_version
            .map(|v| v.to_string())
            .unwrap_or_else(|| "none".to_string())
    );

    println!(
        "{:12} {}",
        "Created:".dimmed(),
        worker.created_at.format("%Y-%m-%d %H:%M:%S")
    );

    println!(
        "{:12} {}",
        "Updated:".dimmed(),
        worker.updated_at.format("%Y-%m-%d %H:%M:%S")
    );
}

async fn cmd_deploy<B: Backend>(
    backend: &B,
    name: &str,
    file: PathBuf,
    message: Option<String>,
) -> Result<(), BackendError> {
    // Read file
    let code = std::fs::read(&file).map_err(|e| {
        BackendError::Api(format!("Failed to read file '{}': {}", file.display(), e))
    })?;

    // Determine code type from extension
    let code_type = match file.extension().and_then(|e| e.to_str()) {
        Some("js") => "javascript",
        Some("ts") => "typescript",
        Some("wasm") => "wasm",
        _ => {
            return Err(BackendError::Api(
                "Unknown file type. Use .js, .ts, or .wasm".to_string(),
            ));
        }
    };

    let input = DeployInput {
        code,
        code_type: code_type.to_string(),
        message,
    };

    let deployment = backend.deploy_worker(name, input).await?;

    println!(
        "{} Deployed '{}' v{}",
        "Deployed".green(),
        name.bold(),
        deployment.version
    );

    println!();
    println!("{:12} {}", "Version:".dimmed(), deployment.version);
    println!("{:12} {}", "Hash:".dimmed(), &deployment.hash[..16]);
    println!("{:12} {}", "Type:".dimmed(), deployment.code_type);
    println!(
        "{:12} {}",
        "Deployed:".dimmed(),
        deployment.deployed_at.format("%Y-%m-%d %H:%M:%S")
    );

    if let Some(msg) = &deployment.message {
        println!("{:12} {}", "Message:".dimmed(), msg);
    }

    Ok(())
}
