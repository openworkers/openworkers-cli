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

// Storage types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageConfig {
    pub id: String,
    pub name: String,
    #[serde(alias = "desc")]
    pub description: Option<String>,
    pub provider: String,
    pub bucket: Option<String>,
    pub prefix: Option<String>,
    pub endpoint: Option<String>,
    pub region: Option<String>,
    pub public_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateStorageInput {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bucket: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_key_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_access_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_url: Option<String>,
}

// KV types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KvNamespace {
    pub id: String,
    pub name: String,
    #[serde(alias = "desc")]
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateKvInput {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
}

// Database types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Database {
    pub id: String,
    pub name: String,
    #[serde(alias = "desc")]
    pub description: Option<String>,
    pub provider: String,
    pub max_rows: i32,
    pub timeout_seconds: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDatabaseInput {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_string: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_rows: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<i32>,
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

    // Storage methods
    fn list_storage(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<StorageConfig>, BackendError>> + Send;

    fn get_storage(
        &self,
        name: &str,
    ) -> impl std::future::Future<Output = Result<StorageConfig, BackendError>> + Send;

    fn create_storage(
        &self,
        input: CreateStorageInput,
    ) -> impl std::future::Future<Output = Result<StorageConfig, BackendError>> + Send;

    fn delete_storage(
        &self,
        name: &str,
    ) -> impl std::future::Future<Output = Result<(), BackendError>> + Send;

    // KV methods
    fn list_kv(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<KvNamespace>, BackendError>> + Send;

    fn get_kv(
        &self,
        name: &str,
    ) -> impl std::future::Future<Output = Result<KvNamespace, BackendError>> + Send;

    fn create_kv(
        &self,
        input: CreateKvInput,
    ) -> impl std::future::Future<Output = Result<KvNamespace, BackendError>> + Send;

    fn delete_kv(
        &self,
        name: &str,
    ) -> impl std::future::Future<Output = Result<(), BackendError>> + Send;

    // Database methods
    fn list_databases(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<Database>, BackendError>> + Send;

    fn get_database(
        &self,
        name: &str,
    ) -> impl std::future::Future<Output = Result<Database, BackendError>> + Send;

    fn create_database(
        &self,
        input: CreateDatabaseInput,
    ) -> impl std::future::Future<Output = Result<Database, BackendError>> + Send;

    fn delete_database(
        &self,
        name: &str,
    ) -> impl std::future::Future<Output = Result<(), BackendError>> + Send;
}
