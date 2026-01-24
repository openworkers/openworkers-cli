use crate::backend::{
    Backend, BackendError, CreateWorkerInput, DeployInput, UpdateWorkerInput, Worker,
};
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

        /// Language (javascript or typescript)
        #[arg(short, long, default_value = "typescript")]
        language: String,
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

    /// Link an environment to a worker
    Link {
        /// Worker name
        name: String,

        /// Environment name
        #[arg(short, long)]
        env: String,
    },

    /// Upload a zip archive with worker.js and assets
    Upload {
        /// Worker name
        name: String,

        /// Path to the zip file
        file: PathBuf,
    },
}

impl WorkersCommand {
    pub async fn run<B: Backend>(self, backend: &B) -> Result<(), BackendError> {
        match self {
            Self::List => cmd_list(backend).await,
            Self::Get { name } => cmd_get(backend, &name).await,
            Self::Create {
                name,
                description,
                language,
            } => cmd_create(backend, name, description, language).await,
            Self::Delete { name } => cmd_delete(backend, &name).await,
            Self::Deploy {
                name,
                file,
                message,
            } => cmd_deploy(backend, &name, file, message).await,
            Self::Link { name, env } => cmd_link(backend, &name, &env).await,
            Self::Upload { name, file } => cmd_upload(backend, &name, file).await,
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
    language: String,
) -> Result<(), BackendError> {
    let input = CreateWorkerInput {
        name,
        description,
        language,
    };
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

async fn cmd_link<B: Backend>(backend: &B, name: &str, env: &str) -> Result<(), BackendError> {
    // Verify environment exists
    let environment = backend.get_environment(env).await?;

    let input = UpdateWorkerInput {
        name: None,
        environment: Some(environment.id),
    };

    backend.update_worker(name, input).await?;

    println!(
        "{} Worker '{}' linked to environment '{}'.",
        "Linked".green(),
        name.bold(),
        env.bold()
    );

    Ok(())
}

async fn cmd_upload<B: Backend>(
    backend: &B,
    name: &str,
    file: PathBuf,
) -> Result<(), BackendError> {
    // Verify it's a zip file
    if file.extension().and_then(|e| e.to_str()) != Some("zip") {
        return Err(BackendError::Api("File must be a .zip archive".to_string()));
    }

    // Read zip file
    let zip_data = std::fs::read(&file).map_err(|e| {
        BackendError::Api(format!("Failed to read file '{}': {}", file.display(), e))
    })?;

    let size_kb = zip_data.len() / 1024;
    println!(
        "{} Uploading {} ({} KB)...",
        "→".blue(),
        file.display(),
        size_kb
    );

    let result = backend.upload_worker(name, zip_data).await?;

    println!(
        "{} Uploaded to '{}' (v{})",
        "Uploaded".green(),
        result.worker.name.bold(),
        result.uploaded.assets
    );

    println!();
    println!("{:12} {}", "URL:".dimmed(), result.worker.url);
    println!(
        "{:12} {}",
        "Script:".dimmed(),
        if result.uploaded.script { "✓" } else { "✗" }
    );
    println!("{:12} {} files", "Assets:".dimmed(), result.uploaded.assets);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::mock::MockBackend;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_list_empty() {
        let backend = MockBackend::new();

        let result = WorkersCommand::List.run(&backend).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_with_workers() {
        let backend = MockBackend::new()
            .with_worker("api", Some("API worker"))
            .with_deployed_worker("web", 3);

        let result = WorkersCommand::List.run(&backend).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_existing() {
        let backend = MockBackend::new().with_worker("my-worker", Some("Test worker"));

        let result = WorkersCommand::Get {
            name: "my-worker".to_string(),
        }
        .run(&backend)
        .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_not_found() {
        let backend = MockBackend::new();

        let result = WorkersCommand::Get {
            name: "nonexistent".to_string(),
        }
        .run(&backend)
        .await;

        assert!(matches!(result, Err(BackendError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_create() {
        let backend = MockBackend::new();

        let result = WorkersCommand::Create {
            name: "new-worker".to_string(),
            description: Some("A new worker".to_string()),
            language: "typescript".to_string(),
        }
        .run(&backend)
        .await;

        assert!(result.is_ok());

        // Verify the worker was created
        let worker = backend.get_worker("new-worker").await.unwrap();
        assert_eq!(worker.name, "new-worker");
        assert_eq!(worker.description, Some("A new worker".to_string()));
    }

    #[tokio::test]
    async fn test_create_without_description() {
        let backend = MockBackend::new();

        let result = WorkersCommand::Create {
            name: "simple-worker".to_string(),
            description: None,
            language: "javascript".to_string(),
        }
        .run(&backend)
        .await;

        assert!(result.is_ok());

        let worker = backend.get_worker("simple-worker").await.unwrap();
        assert!(worker.description.is_none());
    }

    #[tokio::test]
    async fn test_delete_existing() {
        let backend = MockBackend::new().with_worker("to-delete", None);

        let result = WorkersCommand::Delete {
            name: "to-delete".to_string(),
        }
        .run(&backend)
        .await;

        assert!(result.is_ok());

        // Verify it's gone
        let get_result = backend.get_worker("to-delete").await;
        assert!(matches!(get_result, Err(BackendError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_delete_not_found() {
        let backend = MockBackend::new();

        let result = WorkersCommand::Delete {
            name: "nonexistent".to_string(),
        }
        .run(&backend)
        .await;

        assert!(matches!(result, Err(BackendError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_deploy_typescript() {
        let backend = MockBackend::new().with_worker("ts-worker", None);

        let mut temp_file = NamedTempFile::with_suffix(".ts").unwrap();
        writeln!(
            temp_file,
            "export default {{ fetch() {{ return new Response('Hello') }} }}"
        )
        .unwrap();

        let result = WorkersCommand::Deploy {
            name: "ts-worker".to_string(),
            file: temp_file.path().to_path_buf(),
            message: Some("Initial deploy".to_string()),
        }
        .run(&backend)
        .await;

        assert!(result.is_ok());

        // Verify version was updated
        let worker = backend.get_worker("ts-worker").await.unwrap();
        assert_eq!(worker.current_version, Some(1));
    }

    #[tokio::test]
    async fn test_deploy_javascript() {
        let backend = MockBackend::new().with_worker("js-worker", None);

        let mut temp_file = NamedTempFile::with_suffix(".js").unwrap();
        writeln!(
            temp_file,
            "export default {{ fetch() {{ return new Response('Hello') }} }}"
        )
        .unwrap();

        let result = WorkersCommand::Deploy {
            name: "js-worker".to_string(),
            file: temp_file.path().to_path_buf(),
            message: None,
        }
        .run(&backend)
        .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_deploy_increments_version() {
        let backend = MockBackend::new().with_worker("versioned-worker", None);

        let mut temp_file = NamedTempFile::with_suffix(".ts").unwrap();
        writeln!(
            temp_file,
            "export default {{ fetch() {{ return new Response('v1') }} }}"
        )
        .unwrap();

        // First deploy
        WorkersCommand::Deploy {
            name: "versioned-worker".to_string(),
            file: temp_file.path().to_path_buf(),
            message: Some("v1".to_string()),
        }
        .run(&backend)
        .await
        .unwrap();

        let worker = backend.get_worker("versioned-worker").await.unwrap();
        assert_eq!(worker.current_version, Some(1));

        // Second deploy
        writeln!(temp_file, "// v2").unwrap();
        WorkersCommand::Deploy {
            name: "versioned-worker".to_string(),
            file: temp_file.path().to_path_buf(),
            message: Some("v2".to_string()),
        }
        .run(&backend)
        .await
        .unwrap();

        let worker = backend.get_worker("versioned-worker").await.unwrap();
        assert_eq!(worker.current_version, Some(2));
    }

    #[tokio::test]
    async fn test_deploy_invalid_extension() {
        let backend = MockBackend::new().with_worker("worker", None);

        let temp_file = NamedTempFile::with_suffix(".txt").unwrap();

        let result = WorkersCommand::Deploy {
            name: "worker".to_string(),
            file: temp_file.path().to_path_buf(),
            message: None,
        }
        .run(&backend)
        .await;

        assert!(matches!(result, Err(BackendError::Api(_))));
    }

    #[tokio::test]
    async fn test_deploy_worker_not_found() {
        let backend = MockBackend::new();

        let mut temp_file = NamedTempFile::with_suffix(".ts").unwrap();
        writeln!(temp_file, "export default {{}}").unwrap();

        let result = WorkersCommand::Deploy {
            name: "nonexistent".to_string(),
            file: temp_file.path().to_path_buf(),
            message: None,
        }
        .run(&backend)
        .await;

        assert!(matches!(result, Err(BackendError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_deploy_file_not_found() {
        let backend = MockBackend::new().with_worker("worker", None);

        let result = WorkersCommand::Deploy {
            name: "worker".to_string(),
            file: PathBuf::from("/nonexistent/path/file.ts"),
            message: None,
        }
        .run(&backend)
        .await;

        assert!(matches!(result, Err(BackendError::Api(_))));
    }
}
