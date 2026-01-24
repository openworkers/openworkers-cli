use super::{
    Backend, BackendError, CreateDatabaseInput, CreateEnvironmentInput, CreateKvInput,
    CreateStorageInput, CreateWorkerInput, Database, DeployInput, Deployment, Environment,
    KvNamespace, StorageConfig, UpdateEnvironmentInput, UpdateWorkerInput, UploadResult, Worker,
};
use reqwest::Client;

pub struct ApiBackend {
    client: Client,
    base_url: String,
    token: Option<String>,
}

impl ApiBackend {
    pub fn new(base_url: String, token: Option<String>, insecure: bool) -> Self {
        let client = Client::builder()
            .danger_accept_invalid_certs(insecure)
            .build()
            .expect("Failed to build HTTP client");

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
        zip_data: Vec<u8>,
    ) -> Result<UploadResult, BackendError> {
        use reqwest::multipart::{Form, Part};

        // First resolve worker name to ID
        let worker = self.get_worker(name).await?;

        let part = Part::bytes(zip_data)
            .file_name("upload.zip")
            .mime_str("application/zip")
            .map_err(|e| BackendError::Api(e.to_string()))?;

        let form = Form::new().part("file", part);

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
