use super::{
    Backend, BackendError, CreateEnvironmentInput, CreateWorkerInput, DeployInput, Deployment,
    Environment, UpdateEnvironmentInput, Worker,
};
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct MockState {
    workers: HashMap<String, Worker>,
    deployments: HashMap<String, Vec<Deployment>>,
    environments: HashMap<String, Environment>,
}

#[derive(Default, Clone)]
pub struct MockBackend {
    state: Arc<Mutex<MockState>>,
}

impl MockBackend {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_worker(self, name: &str, description: Option<&str>) -> Self {
        let worker = Worker {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
            current_version: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let mut state = self.state.lock().unwrap();
        state.workers.insert(name.to_string(), worker);
        drop(state);

        self
    }

    pub fn with_deployed_worker(self, name: &str, version: i32) -> Self {
        let worker = Worker {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            description: None,
            current_version: Some(version),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let mut state = self.state.lock().unwrap();
        state.workers.insert(name.to_string(), worker);
        drop(state);

        self
    }
}

impl Backend for MockBackend {
    async fn list_workers(&self) -> Result<Vec<Worker>, BackendError> {
        let state = self.state.lock().unwrap();
        let mut workers: Vec<Worker> = state.workers.values().cloned().collect();
        workers.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(workers)
    }

    async fn get_worker(&self, name: &str) -> Result<Worker, BackendError> {
        let state = self.state.lock().unwrap();
        state
            .workers
            .get(name)
            .cloned()
            .ok_or_else(|| BackendError::NotFound(format!("Worker '{}' not found", name)))
    }

    async fn create_worker(&self, input: CreateWorkerInput) -> Result<Worker, BackendError> {
        let mut state = self.state.lock().unwrap();

        if state.workers.contains_key(&input.name) {
            return Err(BackendError::Api(format!(
                "Worker '{}' already exists",
                input.name
            )));
        }

        // Note: language is used by API to set initial deployment, mock ignores it
        let worker = Worker {
            id: uuid::Uuid::new_v4().to_string(),
            name: input.name.clone(),
            description: input.description,
            current_version: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        state.workers.insert(input.name, worker.clone());
        Ok(worker)
    }

    async fn delete_worker(&self, name: &str) -> Result<(), BackendError> {
        let mut state = self.state.lock().unwrap();

        if state.workers.remove(name).is_none() {
            return Err(BackendError::NotFound(format!(
                "Worker '{}' not found",
                name
            )));
        }

        state.deployments.remove(name);
        Ok(())
    }

    async fn deploy_worker(
        &self,
        name: &str,
        input: DeployInput,
    ) -> Result<Deployment, BackendError> {
        let mut state = self.state.lock().unwrap();

        if !state.workers.contains_key(name) {
            return Err(BackendError::NotFound(format!(
                "Worker '{}' not found",
                name
            )));
        }

        let deployments = state.deployments.entry(name.to_string()).or_default();
        let next_version = deployments.len() as i32 + 1;

        let mut hasher = Sha256::new();
        hasher.update(&input.code);
        let hash = hex::encode(hasher.finalize());

        let worker = state.workers.get_mut(name).unwrap();
        let worker_id = worker.id.clone();

        worker.current_version = Some(next_version);
        worker.updated_at = Utc::now();

        let deployment = Deployment {
            worker_id,
            version: next_version,
            hash,
            code_type: input.code_type,
            deployed_at: Utc::now(),
            message: input.message,
        };

        state
            .deployments
            .get_mut(name)
            .unwrap()
            .push(deployment.clone());

        Ok(deployment)
    }

    async fn list_environments(&self) -> Result<Vec<Environment>, BackendError> {
        let state = self.state.lock().unwrap();
        let mut environments: Vec<Environment> = state.environments.values().cloned().collect();
        environments.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(environments)
    }

    async fn get_environment(&self, name: &str) -> Result<Environment, BackendError> {
        let state = self.state.lock().unwrap();
        state
            .environments
            .get(name)
            .cloned()
            .ok_or_else(|| BackendError::NotFound(format!("Environment '{}' not found", name)))
    }

    async fn create_environment(
        &self,
        input: CreateEnvironmentInput,
    ) -> Result<Environment, BackendError> {
        let mut state = self.state.lock().unwrap();

        if state.environments.contains_key(&input.name) {
            return Err(BackendError::Api(format!(
                "Environment '{}' already exists",
                input.name
            )));
        }

        let environment = Environment {
            id: uuid::Uuid::new_v4().to_string(),
            name: input.name.clone(),
            description: input.desc,
            values: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        state.environments.insert(input.name, environment.clone());
        Ok(environment)
    }

    async fn update_environment(
        &self,
        name: &str,
        input: UpdateEnvironmentInput,
    ) -> Result<Environment, BackendError> {
        let mut state = self.state.lock().unwrap();

        let environment = state
            .environments
            .get_mut(name)
            .ok_or_else(|| BackendError::NotFound(format!("Environment '{}' not found", name)))?;

        if let Some(new_name) = input.name {
            environment.name = new_name;
        }

        environment.updated_at = Utc::now();

        Ok(environment.clone())
    }

    async fn delete_environment(&self, name: &str) -> Result<(), BackendError> {
        let mut state = self.state.lock().unwrap();

        if state.environments.remove(name).is_none() {
            return Err(BackendError::NotFound(format!(
                "Environment '{}' not found",
                name
            )));
        }

        Ok(())
    }
}
