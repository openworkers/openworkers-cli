use crate::backend::{Backend, BackendError};
use clap::Subcommand;
use colored::Colorize;

#[derive(Subcommand)]
pub enum ProjectsCommand {
    /// List all projects
    #[command(alias = "ls")]
    List,

    /// Delete a project and all its workers
    #[command(alias = "rm")]
    Delete {
        /// Project name
        name: String,
    },
}

impl ProjectsCommand {
    pub async fn run<B: Backend>(self, backend: &B) -> Result<(), BackendError> {
        match self {
            Self::List => cmd_list(backend).await,
            Self::Delete { name } => cmd_delete(backend, &name).await,
        }
    }
}

async fn cmd_list<B: Backend>(backend: &B) -> Result<(), BackendError> {
    let projects = backend.list_projects().await?;

    if projects.is_empty() {
        println!("No projects found.");
        return Ok(());
    }

    println!("{}", "Projects".bold());
    println!("{}", "─".repeat(60));

    for project in projects {
        println!("  {}", project.name.bold());
    }

    Ok(())
}

async fn cmd_delete<B: Backend>(backend: &B, name: &str) -> Result<(), BackendError> {
    backend.delete_project(name).await?;

    println!(
        "{} Project '{}' and all its workers deleted.",
        "Deleted".red(),
        name.bold()
    );

    Ok(())
}
