use super::{Backend, DeployOptions, LogEntry, Worker};
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// API backend - communicates with openworkers API
pub struct ApiBackend {
    client: Client,
    base_url: String,
    api_key: Option<String>,
}

impl ApiBackend {
    pub fn new(base_url: &str, api_key: Option<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
        }
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.client.request(method, &url);
        if let Some(ref key) = self.api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }
        req
    }
}

#[async_trait]
impl Backend for ApiBackend {
    async fn list_workers(&self) -> Result<Vec<Worker>> {
        let response = self
            .request(reqwest::Method::GET, "/api/workers")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "API error: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            ));
        }

        let workers: Vec<ApiWorker> = response.json().await?;
        Ok(workers.into_iter().map(|w| w.into()).collect())
    }

    async fn get_worker(&self, id_or_name: &str) -> Result<Option<Worker>> {
        let response = self
            .request(
                reqwest::Method::GET,
                &format!("/api/workers/{}", id_or_name),
            )
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if !response.status().is_success() {
            return Err(anyhow!(
                "API error: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            ));
        }

        let worker: ApiWorker = response.json().await?;
        Ok(Some(worker.into()))
    }

    async fn deploy(&self, options: DeployOptions) -> Result<Worker> {
        let payload = DeployPayload {
            name: options.name,
            code: options.code,
            env: options.env,
        };

        let response = self
            .request(reqwest::Method::POST, "/api/workers")
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "API error: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            ));
        }

        let worker: ApiWorker = response.json().await?;
        Ok(worker.into())
    }

    async fn update_code(&self, id: Uuid, code: &str) -> Result<Worker> {
        let payload = UpdateCodePayload {
            code: code.to_string(),
        };

        let response = self
            .request(reqwest::Method::PATCH, &format!("/api/workers/{}", id))
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "API error: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            ));
        }

        let worker: ApiWorker = response.json().await?;
        Ok(worker.into())
    }

    async fn update_env(&self, id: Uuid, env: HashMap<String, String>) -> Result<Worker> {
        let payload = UpdateEnvPayload { env };

        let response = self
            .request(reqwest::Method::PATCH, &format!("/api/workers/{}/env", id))
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "API error: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            ));
        }

        let worker: ApiWorker = response.json().await?;
        Ok(worker.into())
    }

    async fn delete(&self, id: Uuid) -> Result<()> {
        let response = self
            .request(reqwest::Method::DELETE, &format!("/api/workers/{}", id))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "API error: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            ));
        }

        Ok(())
    }

    async fn logs(&self, worker_id: Uuid, limit: i64) -> Result<Vec<LogEntry>> {
        let response = self
            .request(
                reqwest::Method::GET,
                &format!("/api/workers/{}/logs?limit={}", worker_id, limit),
            )
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "API error: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            ));
        }

        let logs: Vec<ApiLogEntry> = response.json().await?;
        Ok(logs.into_iter().map(|l| l.into()).collect())
    }
}

// API response types
#[derive(Debug, Deserialize)]
struct ApiWorker {
    id: Uuid,
    name: String,
    #[serde(alias = "script")]
    code: String,
    env: Option<HashMap<String, String>>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<ApiWorker> for Worker {
    fn from(w: ApiWorker) -> Self {
        Worker {
            id: w.id,
            name: w.name,
            code: w.code,
            env: w.env,
            created_at: w.created_at,
            updated_at: w.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ApiLogEntry {
    date: chrono::DateTime<chrono::Utc>,
    worker_id: Uuid,
    level: String,
    message: String,
}

impl From<ApiLogEntry> for LogEntry {
    fn from(l: ApiLogEntry) -> Self {
        LogEntry {
            date: l.date,
            worker_id: l.worker_id,
            level: l.level,
            message: l.message,
        }
    }
}

// Request payload types
#[derive(Debug, Serialize)]
struct DeployPayload {
    name: String,
    code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    env: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize)]
struct UpdateCodePayload {
    code: String,
}

#[derive(Debug, Serialize)]
struct UpdateEnvPayload {
    env: HashMap<String, String>,
}
