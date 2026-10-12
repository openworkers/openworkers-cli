use super::{
    AssetManifestEntry, Backend, BackendError, CreateDatabaseInput, CreateEnvironmentInput,
    CreateKvInput, CreateStorageInput, CreateWorkerInput, Database, DeployInput, Deployment,
    Environment, KvNamespace, Project, StorageConfig, UpdateEnvironmentInput, UpdateWorkerInput,
    UploadResult, Worker,
};
use super::{CreateD1Input, D1Database};
use super::{CreateDurableObjectInput, DurableObjectNamespace};
use crate::config::DEFAULT_API_URL;
use reqwest::Client;

pub struct ApiBackend {
    client: Client,
    base_url: String,
    token: Option<String>,
}

/// The body of a `/d1` response, or the error its status means
async fn d1_response<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
    name: &str,
) -> Result<T, BackendError> {
    match response.status() {
        reqwest::StatusCode::NOT_FOUND => Err(BackendError::NotFound(format!(
            "D1 database '{}' not found",
            name
        ))),
        reqwest::StatusCode::UNAUTHORIZED => Err(BackendError::Unauthorized),
        reqwest::StatusCode::NO_CONTENT => serde_json::from_value(serde_json::Value::Null)
            .map_err(|e| BackendError::Api(e.to_string())),
        status if !status.is_success() => {
            Err(BackendError::Api(response.text().await.unwrap_or_default()))
        }
        _ => Ok(response.json().await?),
    }
}

async fn namespace_response<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
    name: &str,
) -> Result<T, BackendError> {
    match response.status() {
        reqwest::StatusCode::NOT_FOUND => Err(BackendError::NotFound(format!(
            "Durable Object namespace '{}' not found",
            name
        ))),
        reqwest::StatusCode::UNAUTHORIZED => Err(BackendError::Unauthorized),
        reqwest::StatusCode::NO_CONTENT => serde_json::from_value(serde_json::Value::Null)
            .map_err(|e| BackendError::Api(e.to_string())),
        status if !status.is_success() => {
            Err(BackendError::Api(response.text().await.unwrap_or_default()))
        }
        _ => Ok(response.json().await?),
    }
}

impl ApiBackend {
    pub fn new(base_url: String, token: Option<String>, insecure: bool) -> Self {
        let mut builder = Client::builder().danger_accept_invalid_certs(insecure);

        // Resolve *.localhost domains to 127.0.0.1 (RFC 6761)
        if let Ok(url) = reqwest::Url::parse(&base_url)
            && let Some(host) = url.host_str()
            && host.ends_with(".localhost")
        {
            let port = url.port_or_known_default().unwrap_or(443);
            let addr: std::net::SocketAddr = ([127, 0, 0, 1], port).into();
            builder = builder.resolve(host, addr);
        }

        let client = builder.build().expect("Failed to build HTTP client");

        Self {
            client,
            base_url,
            token,
        }
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.client.request(method, &url);

        if let Some(token) = &self.token {
            req = req.bearer_auth(token);
        }

        req
    }
}

impl Backend for ApiBackend {
    fn is_default_cloud(&self) -> bool {
        self.base_url == DEFAULT_API_URL
    }

    async fn list_workers(&self) -> Result<Vec<Worker>, BackendError> {
        let response = self
            .request(reqwest::Method::GET, "/workers")
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(BackendError::Unauthorized);
        }

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::Api(text));
        }

        let workers: Vec<Worker> = response.json().await?;
        Ok(workers)
    }

    async fn get_worker(&self, name: &str) -> Result<Worker, BackendError> {
        let response = self
            .request(reqwest::Method::GET, &format!("/workers/{}", name))
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(BackendError::NotFound(format!(
                "Worker '{}' not found",
                name
            )));
        }

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(BackendError::Unauthorized);
        }

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::Api(text));
        }

        let worker: Worker = response.json().await?;
        Ok(worker)
    }

    async fn create_worker(&self, input: CreateWorkerInput) -> Result<Worker, BackendError> {
        let response = self
            .request(reqwest::Method::POST, "/workers")
            .json(&input)
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(BackendError::Unauthorized);
        }

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::Api(text));
        }

        let worker: Worker = response.json().await?;
        Ok(worker)
    }

    async fn delete_worker(&self, name: &str) -> Result<(), BackendError> {
        let response = self
            .request(reqwest::Method::DELETE, &format!("/workers/{}", name))
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(BackendError::NotFound(format!(
                "Worker '{}' not found",
                name
            )));
        }

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(BackendError::Unauthorized);
        }

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::Api(text));
        }

        Ok(())
    }

    async fn update_worker(
        &self,
        name: &str,
        input: UpdateWorkerInput,
    ) -> Result<Worker, BackendError> {
        let response = self
            .request(reqwest::Method::PATCH, &format!("/workers/{}", name))
            .json(&input)
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(BackendError::NotFound(format!(
                "Worker '{}' not found",
                name
            )));
        }

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(BackendError::Unauthorized);
        }

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::Api(text));
        }

        let worker: Worker = response.json().await?;
        Ok(worker)
    }

    async fn link_worker_environment(
        &self,
        worker_id: &str,
        environment_id: &str,
    ) -> Result<(), BackendError> {
        let response = self
            .request(reqwest::Method::PATCH, &format!("/workers/{}", worker_id))
            .json(&serde_json::json!({ "environment": environment_id }))
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(BackendError::NotFound(format!(
                "Worker '{}' or environment '{}' not found",
                worker_id, environment_id
            )));
        }

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(BackendError::Unauthorized);
        }

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::Api(text));
        }

        Ok(())
    }

    async fn deploy_worker(
        &self,
        name: &str,
        input: DeployInput,
    ) -> Result<Deployment, BackendError> {
        let response = self
            .request(reqwest::Method::POST, &format!("/workers/{}/deploy", name))
            .json(&input)
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(BackendError::NotFound(format!(
                "Worker '{}' not found",
                name
            )));
        }

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(BackendError::Unauthorized);
        }

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::Api(text));
        }

        let deployment: Deployment = response.json().await?;
        Ok(deployment)
    }

    async fn upload_worker(
        &self,
        name: &str,
        _path: &std::path::Path,
        zip_data: Vec<u8>,
        assets_manifest: &[AssetManifestEntry],
    ) -> Result<UploadResult, BackendError> {
        use reqwest::multipart::{Form, Part};

        // First resolve worker name to ID
        let worker = self.get_worker(name).await?;

        let part = Part::bytes(zip_data)
            .file_name("upload.zip")
            .mime_str("application/zip")
            .map_err(|e| BackendError::Api(e.to_string()))?;

        let mut form = Form::new().part("file", part);

        if !assets_manifest.is_empty() {
            let manifest_json = serde_json::to_string(assets_manifest)
                .map_err(|e| BackendError::Api(e.to_string()))?;
            form = form.text("assets", manifest_json);
        }

        let response = self
            .request(
                reqwest::Method::POST,
                &format!("/workers/{}/upload", worker.id),
            )
            .multipart(form)
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(BackendError::NotFound(format!(
                "Worker '{}' not found",
                name
            )));
        }

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(BackendError::Unauthorized);
        }

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::Api(text));
        }

        let result: UploadResult = response.json().await?;
        Ok(result)
    }

    // Project methods
    async fn list_projects(&self) -> Result<Vec<Project>, BackendError> {
        Err(BackendError::Api(
            "Projects require DB access. Use a DB alias.".to_string(),
        ))
    }

    async fn delete_project(&self, _name: &str) -> Result<(), BackendError> {
        Err(BackendError::Api(
            "Projects require DB access. Use a DB alias.".to_string(),
        ))
    }

    async fn list_environments(&self) -> Result<Vec<Environment>, BackendError> {
        let response = self
            .request(reqwest::Method::GET, "/environments")
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(BackendError::Unauthorized);
        }

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::Api(text));
        }

        let environments: Vec<Environment> = response.json().await?;
        Ok(environments)
    }

    async fn get_environment(&self, name: &str) -> Result<Environment, BackendError> {
        let response = self
            .request(reqwest::Method::GET, &format!("/environments/{}", name))
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(BackendError::NotFound(format!(
                "Environment '{}' not found",
                name
            )));
        }

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(BackendError::Unauthorized);
        }

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::Api(text));
        }

        let environment: Environment = response.json().await?;
        Ok(environment)
    }

    async fn create_environment(
        &self,
        input: CreateEnvironmentInput,
    ) -> Result<Environment, BackendError> {
        let response = self
            .request(reqwest::Method::POST, "/environments")
            .json(&input)
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(BackendError::Unauthorized);
        }

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::Api(text));
        }

        let environment: Environment = response.json().await?;
        Ok(environment)
    }

    async fn update_environment(
        &self,
        name: &str,
        input: UpdateEnvironmentInput,
    ) -> Result<Environment, BackendError> {
        let response = self
            .request(reqwest::Method::PATCH, &format!("/environments/{}", name))
            .json(&input)
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(BackendError::NotFound(format!(
                "Environment '{}' not found",
                name
            )));
        }

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(BackendError::Unauthorized);
        }

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::Api(text));
        }

        let environment: Environment = response.json().await?;
        Ok(environment)
    }

    async fn delete_environment(&self, name: &str) -> Result<(), BackendError> {
        let response = self
            .request(reqwest::Method::DELETE, &format!("/environments/{}", name))
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(BackendError::NotFound(format!(
                "Environment '{}' not found",
                name
            )));
        }

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(BackendError::Unauthorized);
        }

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::Api(text));
        }

        Ok(())
    }

    // Storage methods
    async fn list_storage(&self) -> Result<Vec<StorageConfig>, BackendError> {
        let response = self
            .request(reqwest::Method::GET, "/storage")
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(BackendError::Unauthorized);
        }

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::Api(text));
        }

        let configs: Vec<StorageConfig> = response.json().await?;
        Ok(configs)
    }

    async fn get_storage(&self, name: &str) -> Result<StorageConfig, BackendError> {
        let response = self
            .request(reqwest::Method::GET, &format!("/storage/{}", name))
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(BackendError::NotFound(format!(
                "Storage '{}' not found",
                name
            )));
        }

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(BackendError::Unauthorized);
        }

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::Api(text));
        }

        let config: StorageConfig = response.json().await?;
        Ok(config)
    }

    async fn create_storage(
        &self,
        input: CreateStorageInput,
    ) -> Result<StorageConfig, BackendError> {
        let response = self
            .request(reqwest::Method::POST, "/storage")
            .json(&input)
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(BackendError::Unauthorized);
        }

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::Api(text));
        }

        let config: StorageConfig = response.json().await?;
        Ok(config)
    }

    async fn delete_storage(&self, name: &str) -> Result<(), BackendError> {
        let response = self
            .request(reqwest::Method::DELETE, &format!("/storage/{}", name))
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(BackendError::NotFound(format!(
                "Storage '{}' not found",
                name
            )));
        }

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(BackendError::Unauthorized);
        }

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::Api(text));
        }

        Ok(())
    }

    // KV methods
    async fn list_kv(&self) -> Result<Vec<KvNamespace>, BackendError> {
        let response = self.request(reqwest::Method::GET, "/kv").send().await?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(BackendError::Unauthorized);
        }

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::Api(text));
        }

        let namespaces: Vec<KvNamespace> = response.json().await?;
        Ok(namespaces)
    }

    async fn get_kv(&self, name: &str) -> Result<KvNamespace, BackendError> {
        let response = self
            .request(reqwest::Method::GET, &format!("/kv/{}", name))
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(BackendError::NotFound(format!(
                "KV namespace '{}' not found",
                name
            )));
        }

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(BackendError::Unauthorized);
        }

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::Api(text));
        }

        let namespace: KvNamespace = response.json().await?;
        Ok(namespace)
    }

    async fn create_kv(&self, input: CreateKvInput) -> Result<KvNamespace, BackendError> {
        let response = self
            .request(reqwest::Method::POST, "/kv")
            .json(&input)
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(BackendError::Unauthorized);
        }

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::Api(text));
        }

        let namespace: KvNamespace = response.json().await?;
        Ok(namespace)
    }

    async fn delete_kv(&self, name: &str) -> Result<(), BackendError> {
        let response = self
            .request(reqwest::Method::DELETE, &format!("/kv/{}", name))
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(BackendError::NotFound(format!(
                "KV namespace '{}' not found",
                name
            )));
        }

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(BackendError::Unauthorized);
        }

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::Api(text));
        }

        Ok(())
    }

    // D1 methods
    async fn list_d1(&self) -> Result<Vec<D1Database>, BackendError> {
        let response = self.request(reqwest::Method::GET, "/d1").send().await?;

        d1_response(response, "").await
    }

    async fn get_d1(&self, name: &str) -> Result<D1Database, BackendError> {
        let response = self
            .request(reqwest::Method::GET, &format!("/d1/{}", name))
            .send()
            .await?;

        d1_response(response, name).await
    }

    async fn create_d1(&self, input: CreateD1Input) -> Result<D1Database, BackendError> {
        let response = self
            .request(reqwest::Method::POST, "/d1")
            .json(&input)
            .send()
            .await?;

        d1_response(response, "").await
    }

    async fn delete_d1(&self, name: &str) -> Result<(), BackendError> {
        let response = self
            .request(reqwest::Method::DELETE, &format!("/d1/{}", name))
            .send()
            .await?;

        d1_response::<serde_json::Value>(response, name)
            .await
            .map(|_| ())
    }

    // Durable Object methods
    async fn list_durable_objects(&self) -> Result<Vec<DurableObjectNamespace>, BackendError> {
        let response = self
            .request(reqwest::Method::GET, "/durable-objects")
            .send()
            .await?;

        namespace_response(response, "").await
    }

    async fn get_durable_object(&self, name: &str) -> Result<DurableObjectNamespace, BackendError> {
        let response = self
            .request(reqwest::Method::GET, &format!("/durable-objects/{}", name))
            .send()
            .await?;

        namespace_response(response, name).await
    }

    async fn create_durable_object(
        &self,
        input: CreateDurableObjectInput,
    ) -> Result<DurableObjectNamespace, BackendError> {
        let response = self
            .request(reqwest::Method::POST, "/durable-objects")
            .json(&input)
            .send()
            .await?;

        namespace_response(response, "").await
    }

    async fn delete_durable_object(&self, name: &str) -> Result<(), BackendError> {
        let response = self
            .request(
                reqwest::Method::DELETE,
                &format!("/durable-objects/{}", name),
            )
            .send()
            .await?;

        namespace_response::<serde_json::Value>(response, name)
            .await
            .map(|_| ())
    }

    async fn d1_request(
        &self,
        name: &str,
        request: serde_json::Value,
    ) -> Result<serde_json::Value, BackendError> {
        let response = self
            .request(reqwest::Method::POST, &format!("/d1/{}/requests", name))
            .json(&request)
            .send()
            .await?;

        d1_response(response, name).await
    }

    // Database methods
    async fn list_databases(&self) -> Result<Vec<Database>, BackendError> {
        let response = self
            .request(reqwest::Method::GET, "/databases")
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(BackendError::Unauthorized);
        }

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::Api(text));
        }

        let databases: Vec<Database> = response.json().await?;
        Ok(databases)
    }

    async fn get_database(&self, name: &str) -> Result<Database, BackendError> {
        let response = self
            .request(reqwest::Method::GET, &format!("/databases/{}", name))
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(BackendError::NotFound(format!(
                "Database '{}' not found",
                name
            )));
        }

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(BackendError::Unauthorized);
        }

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::Api(text));
        }

        let database: Database = response.json().await?;
        Ok(database)
    }

    async fn create_database(&self, input: CreateDatabaseInput) -> Result<Database, BackendError> {
        let response = self
            .request(reqwest::Method::POST, "/databases")
            .json(&input)
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(BackendError::Unauthorized);
        }

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::Api(text));
        }

        let database: Database = response.json().await?;
        Ok(database)
    }

    async fn delete_database(&self, name: &str) -> Result<(), BackendError> {
        let response = self
            .request(reqwest::Method::DELETE, &format!("/databases/{}", name))
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(BackendError::NotFound(format!(
                "Database '{}' not found",
                name
            )));
        }

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(BackendError::Unauthorized);
        }

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::Api(text));
        }

        Ok(())
    }
}
