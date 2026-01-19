pub mod api;
pub mod db;

#[cfg(test)]
pub mod mock;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BackendError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("API error: {0}")]
    Api(String),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unauthorized")]
    Unauthorized,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Worker {
    pub id: String,
    pub name: String,
    #[serde(alias = "desc")]
    pub description: Option<String>,
    pub current_version: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkerInput {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Deployment {
    pub worker_id: String,
    pub version: i32,
    pub hash: String,
    pub code_type: String,
    pub deployed_at: DateTime<Utc>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployInput {
    pub code: Vec<u8>,
    pub code_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

pub trait Backend: Send + Sync {
    fn list_workers(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<Worker>, BackendError>> + Send;

    fn get_worker(
        &self,
        name: &str,
    ) -> impl std::future::Future<Output = Result<Worker, BackendError>> + Send;

    fn create_worker(
        &self,
        input: CreateWorkerInput,
    ) -> impl std::future::Future<Output = Result<Worker, BackendError>> + Send;

    fn delete_worker(
        &self,
        name: &str,
    ) -> impl std::future::Future<Output = Result<(), BackendError>> + Send;

    fn deploy_worker(
        &self,
        name: &str,
        input: DeployInput,
    ) -> impl std::future::Future<Output = Result<Deployment, BackendError>> + Send;
}
