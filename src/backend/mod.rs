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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Environment {
    pub id: String,
    pub name: String,
    #[serde(alias = "desc")]
    pub description: Option<String>,
    #[serde(default)]
    pub values: Vec<EnvironmentValue>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentValue {
    pub id: String,
    pub key: String,
    pub value: String,
    #[serde(rename = "type")]
    pub value_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateEnvironmentInput {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateEnvironmentInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<EnvironmentValueInput>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentValueInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(rename = "type")]
    pub value_type: String,
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

    // Environment methods
    fn list_environments(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<Environment>, BackendError>> + Send;

    fn get_environment(
        &self,
        name: &str,
    ) -> impl std::future::Future<Output = Result<Environment, BackendError>> + Send;

    fn create_environment(
        &self,
        input: CreateEnvironmentInput,
    ) -> impl std::future::Future<Output = Result<Environment, BackendError>> + Send;

    fn update_environment(
        &self,
        name: &str,
        input: UpdateEnvironmentInput,
    ) -> impl std::future::Future<Output = Result<Environment, BackendError>> + Send;

    fn delete_environment(
        &self,
        name: &str,
    ) -> impl std::future::Future<Output = Result<(), BackendError>> + Send;
}
