pub mod api;
pub mod db;

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Worker representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Worker {
    pub id: Uuid,
    pub name: String,
    pub code: String,
    pub env: Option<HashMap<String, String>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub date: DateTime<Utc>,
    pub worker_id: Uuid,
    pub level: String,
    pub message: String,
}

/// Deploy options
#[derive(Debug, Clone)]
pub struct DeployOptions {
    pub name: String,
    pub code: String,
    pub env: Option<HashMap<String, String>>,
}

/// Backend trait - implemented by both API and DB backends
#[async_trait]
pub trait Backend: Send + Sync {
    /// List all workers
    async fn list_workers(&self) -> Result<Vec<Worker>>;

    /// Get a worker by ID or name
    async fn get_worker(&self, id_or_name: &str) -> Result<Option<Worker>>;

    /// Deploy a new worker
    async fn deploy(&self, options: DeployOptions) -> Result<Worker>;

    /// Update worker code
    async fn update_code(&self, id: Uuid, code: &str) -> Result<Worker>;

    /// Update worker environment variables
    async fn update_env(&self, id: Uuid, env: HashMap<String, String>) -> Result<Worker>;

    /// Delete a worker
    async fn delete(&self, id: Uuid) -> Result<()>;

    /// Get logs for a worker
    async fn logs(&self, worker_id: Uuid, limit: i64) -> Result<Vec<LogEntry>>;
}
