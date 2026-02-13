use crate::backend::{
    Backend, BackendError, CreateWorkerInput, DeployInput, UpdateWorkerInput, Worker,
};
use clap::Subcommand;
use colored::Colorize;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum WorkersCommand {
    /// List all workers with their version and description
    #[command(alias = "ls")]
    List,

    /// Show detailed information about a worker
    #[command(after_help = "Example:\n  ow workers get my-api")]
    Get {
        /// Worker name
        name: String,
    },

    /// Create a new worker (available at https://<name>.workers.rocks)
    #[command(after_help = "Examples:\n  \
        ow workers create my-api\n  \
        ow workers create my-api -d \"REST API for users\"\n  \
        ow workers create my-api --language javascript")]
    Create {
        /// Worker name (becomes part of the URL)
        name: String,

        /// Short description of what this worker does
        #[arg(short, long)]
        description: Option<String>,

        /// Source language: javascript or typescript
        #[arg(short, long, default_value = "typescript")]
        language: String,
    },

    /// Delete a worker permanently
    #[command(alias = "rm", after_help = "Example:\n  ow workers delete my-api")]
    Delete {
        /// Worker name to delete
        name: String,
    },

    /// Deploy a single source file to a worker
    #[command(after_help = "Examples:\n  \
        ow workers deploy my-api worker.ts\n  \
        ow workers deploy my-api dist/worker.js -m \"Fix auth bug\"")]
    Deploy {
        /// Worker name to deploy to
        name: String,

        /// Source file (.js, .ts, or .wasm)
        file: PathBuf,

        /// Deployment message (shown in version history)
        #[arg(short, long)]
        message: Option<String>,
    },

    /// Link an environment to a worker (for bindings and secrets)
    #[command(after_help = "Example:\n  ow workers link my-api my-env")]
    Link {
        /// Worker name
        name: String,

        /// Environment name to link
        env: String,
    },

    /// Upload a folder with worker.js and static assets
    #[command(after_help = "Examples:\n  \
        ow workers upload my-app ./dist\n  \
        ow workers upload my-app ./build.zip\n\n\
        Note: Worker must have an ASSETS binding configured.\n\
        The folder should contain worker.js at the root.")]
    Upload {
        /// Worker name to upload to
        name: String,

        /// Path to folder or .zip archive containing worker.js and assets
        path: PathBuf,
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
            Self::Upload { name, path } => cmd_upload(backend, &name, path).await,
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

    if let Some(env) = &worker.environment {
        println!("{:12} {}", "Environment:".dimmed(), env.name.cyan());
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
    path: PathBuf,
) -> Result<(), BackendError> {
    let zip_data = if path.is_dir() {
        // Create zip from folder
        println!("{} Creating archive from {}...", "→".blue(), path.display());
        create_zip_from_folder(&path)?
    } else if path.extension().and_then(|e| e.to_str()) == Some("zip") {
        // Read existing zip file
        std::fs::read(&path).map_err(|e| {
            BackendError::Api(format!("Failed to read file '{}': {}", path.display(), e))
        })?
    } else {
        return Err(BackendError::Api(
            "Path must be a .zip archive or a folder".to_string(),
        ));
    };

    let size_kb = zip_data.len() / 1024;
    println!(
        "{} Uploading {} ({} KB)...",
        "→".blue(),
        path.display(),
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
    // Show URL based on backend type and URL format
    // - If URL starts with http, use it as-is (custom domain or cloud API)
    // - If URL doesn't start with http (just worker name) and backend is default cloud, show workers.rocks
    // - Otherwise, just show worker name
    if result.worker.url.starts_with("http") {
        println!("{:12} {}", "URL:".dimmed(), result.worker.url);
    } else if backend.is_default_cloud() {
        println!(
            "{:12} https://{}.workers.rocks",
            "URL:".dimmed(),
            result.worker.url
        );
    } else {
        println!("{:12} {}", "Worker:".dimmed(), result.worker.url);
    }
    println!(
        "{:12} {}",
        "Script:".dimmed(),
        if result.uploaded.script { "✓" } else { "✗" }
    );
    println!("{:12} {} files", "Assets:".dimmed(), result.uploaded.assets);

    Ok(())
}

fn create_zip_from_folder(folder: &PathBuf) -> Result<Vec<u8>, BackendError> {
    use std::io::{Cursor, Write};
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    let mut buffer = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(&mut buffer);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    fn add_directory(
        zip: &mut ZipWriter<&mut Cursor<Vec<u8>>>,
        folder: &PathBuf,
        base: &PathBuf,
        options: SimpleFileOptions,
    ) -> Result<(), BackendError> {
        for entry in std::fs::read_dir(folder).map_err(|e| {
            BackendError::Api(format!(
                "Failed to read directory '{}': {}",
                folder.display(),
                e
            ))
        })? {
            let entry =
                entry.map_err(|e| BackendError::Api(format!("Failed to read entry: {}", e)))?;
            let path = entry.path();
            let relative = path
                .strip_prefix(base)
                .map_err(|e| BackendError::Api(format!("Path error: {}", e)))?;

            if path.is_dir() {
                add_directory(zip, &path, base, options)?;
            } else {
                let content = std::fs::read(&path).map_err(|e| {
                    BackendError::Api(format!("Failed to read file '{}': {}", path.display(), e))
                })?;

                let relative_path = relative.to_string_lossy().replace('\\', "/");
                zip.start_file(relative_path, options)
                    .map_err(|e| BackendError::Api(format!("Zip error: {}", e)))?;

                zip.write_all(&content)
                    .map_err(|e| BackendError::Api(format!("Zip write error: {}", e)))?;
            }
        }

        Ok(())
    }

    add_directory(&mut zip, folder, folder, options)?;
    zip.finish()
        .map_err(|e| BackendError::Api(format!("Zip finish error: {}", e)))?;

    Ok(buffer.into_inner())
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
