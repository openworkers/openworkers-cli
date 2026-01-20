use super::{
    Backend, BackendError, CreateEnvironmentInput, CreateWorkerInput, DeployInput, Deployment,
    Environment, UpdateEnvironmentInput, Worker,
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
}
