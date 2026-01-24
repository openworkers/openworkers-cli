use super::{
    Backend, BackendError, CreateDatabaseInput, CreateEnvironmentInput, CreateKvInput,
    CreateStorageInput, CreateWorkerInput, Database, DeployInput, Deployment, Environment,
    KvNamespace, StorageConfig, UpdateEnvironmentInput, UpdateWorkerInput, UploadResult, Worker,
};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};

pub struct DbBackend {
    pool: PgPool,
}

impl DbBackend {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl Backend for DbBackend {
    async fn list_workers(&self) -> Result<Vec<Worker>, BackendError> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, "desc", current_version, created_at, updated_at
            FROM workers
            ORDER BY name
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let workers = rows
            .iter()
            .map(|row| Worker {
                id: row.get::<uuid::Uuid, _>("id").to_string(),
                name: row.get("name"),
                description: row.get("desc"),
                current_version: row.get("current_version"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();

        Ok(workers)
    }

    async fn get_worker(&self, name: &str) -> Result<Worker, BackendError> {
        let row = sqlx::query(
            r#"
            SELECT id, name, "desc", current_version, created_at, updated_at
            FROM workers
            WHERE name = $1
            "#,
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| BackendError::NotFound(format!("Worker '{}' not found", name)))?;

        Ok(Worker {
            id: row.get::<uuid::Uuid, _>("id").to_string(),
            name: row.get("name"),
            description: row.get("desc"),
            current_version: row.get("current_version"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    async fn create_worker(&self, input: CreateWorkerInput) -> Result<Worker, BackendError> {
        // For CLI/admin mode, we need a user_id
        // For now, get or create an "admin" user
        let user_id: uuid::Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO users (id, username, created_at, updated_at)
            VALUES (gen_random_uuid(), 'cli-admin', now(), now())
            ON CONFLICT (username) DO UPDATE SET username = users.username
            RETURNING id
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        let row = sqlx::query(
            r#"
            INSERT INTO workers (name, "desc", user_id)
            VALUES ($1, $2, $3)
            RETURNING id, name, "desc", current_version, created_at, updated_at
            "#,
        )
        .bind(&input.name)
        .bind(&input.description)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        // Note: language is used by API to set initial deployment, DB backend ignores it for now

        Ok(Worker {
            id: row.get::<uuid::Uuid, _>("id").to_string(),
            name: row.get("name"),
            description: row.get("desc"),
            current_version: row.get("current_version"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    async fn delete_worker(&self, name: &str) -> Result<(), BackendError> {
        let result = sqlx::query("DELETE FROM workers WHERE name = $1")
            .bind(name)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(BackendError::NotFound(format!(
                "Worker '{}' not found",
                name
            )));
        }

        Ok(())
    }

    async fn update_worker(
        &self,
        name: &str,
        input: UpdateWorkerInput,
    ) -> Result<Worker, BackendError> {
        // Get environment_id if environment name is provided
        let env_id: Option<uuid::Uuid> = if let Some(env_name) = &input.environment {
            Some(
                sqlx::query_scalar("SELECT id FROM environments WHERE name = $1")
                    .bind(env_name)
                    .fetch_optional(&self.pool)
                    .await?
                    .ok_or_else(|| {
                        BackendError::NotFound(format!("Environment '{}' not found", env_name))
                    })?,
            )
        } else {
            None
        };

        let row = sqlx::query(
            r#"
            UPDATE workers
            SET environment_id = COALESCE($2, environment_id),
                updated_at = now()
            WHERE name = $1
            RETURNING id, name, "desc", current_version, created_at, updated_at
            "#,
        )
        .bind(name)
        .bind(env_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| BackendError::NotFound(format!("Worker '{}' not found", name)))?;

        Ok(Worker {
            id: row.get::<uuid::Uuid, _>("id").to_string(),
            name: row.get("name"),
            description: row.get("desc"),
            current_version: row.get("current_version"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    async fn deploy_worker(
        &self,
        name: &str,
        input: DeployInput,
    ) -> Result<Deployment, BackendError> {
        // Get worker ID
        let worker_id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM workers WHERE name = $1")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| BackendError::NotFound(format!("Worker '{}' not found", name)))?;

        // Calculate hash
        let mut hasher = Sha256::new();
        hasher.update(&input.code);
        let hash = hex::encode(hasher.finalize());

        // Get next version
        let current_version: Option<i32> =
            sqlx::query_scalar("SELECT MAX(version) FROM worker_deployments WHERE worker_id = $1")
                .bind(worker_id)
                .fetch_one(&self.pool)
                .await?;

        let next_version = current_version.unwrap_or(0) + 1;

        // Insert deployment
        let row = sqlx::query(
            r#"
            INSERT INTO worker_deployments (worker_id, version, hash, code_type, code, message)
            VALUES ($1, $2, $3, $4::enum_code_type, $5, $6)
            RETURNING worker_id, version, hash, code_type::text, deployed_at, message
            "#,
        )
        .bind(worker_id)
        .bind(next_version)
        .bind(&hash)
        .bind(&input.code_type)
        .bind(&input.code)
        .bind(&input.message)
        .fetch_one(&self.pool)
        .await?;

        // Update worker's current_version
        sqlx::query("UPDATE workers SET current_version = $1 WHERE id = $2")
            .bind(next_version)
            .bind(worker_id)
            .execute(&self.pool)
            .await?;

        Ok(Deployment {
            worker_id: row.get::<uuid::Uuid, _>("worker_id").to_string(),
            version: row.get("version"),
            hash: row.get("hash"),
            code_type: row.get("code_type"),
            deployed_at: row.get("deployed_at"),
            message: row.get("message"),
        })
    }

    async fn upload_worker(
        &self,
        _name: &str,
        _zip_data: Vec<u8>,
    ) -> Result<UploadResult, BackendError> {
        Err(BackendError::Api(
            "Upload requires API access (for S3 assets). Use an API alias.".to_string(),
        ))
    }

    async fn list_environments(&self) -> Result<Vec<Environment>, BackendError> {
        Err(BackendError::Api(
            "Environments require API access. Use an API alias.".to_string(),
        ))
    }

    async fn get_environment(&self, _name: &str) -> Result<Environment, BackendError> {
        Err(BackendError::Api(
            "Environments require API access. Use an API alias.".to_string(),
        ))
    }

    async fn create_environment(
        &self,
        _input: CreateEnvironmentInput,
    ) -> Result<Environment, BackendError> {
        Err(BackendError::Api(
            "Environments require API access. Use an API alias.".to_string(),
        ))
    }

    async fn update_environment(
        &self,
        _name: &str,
        _input: UpdateEnvironmentInput,
    ) -> Result<Environment, BackendError> {
        Err(BackendError::Api(
            "Environments require API access. Use an API alias.".to_string(),
        ))
    }

    async fn delete_environment(&self, _name: &str) -> Result<(), BackendError> {
        Err(BackendError::Api(
            "Environments require API access. Use an API alias.".to_string(),
        ))
    }

    // Storage methods
    async fn list_storage(&self) -> Result<Vec<StorageConfig>, BackendError> {
        Err(BackendError::Api(
            "Storage requires API access. Use an API alias.".to_string(),
        ))
    }

    async fn get_storage(&self, _name: &str) -> Result<StorageConfig, BackendError> {
        Err(BackendError::Api(
            "Storage requires API access. Use an API alias.".to_string(),
        ))
    }

    async fn create_storage(
        &self,
        _input: CreateStorageInput,
    ) -> Result<StorageConfig, BackendError> {
        Err(BackendError::Api(
            "Storage requires API access. Use an API alias.".to_string(),
        ))
    }

    async fn delete_storage(&self, _name: &str) -> Result<(), BackendError> {
        Err(BackendError::Api(
            "Storage requires API access. Use an API alias.".to_string(),
        ))
    }

    // KV methods
    async fn list_kv(&self) -> Result<Vec<KvNamespace>, BackendError> {
        Err(BackendError::Api(
            "KV requires API access. Use an API alias.".to_string(),
        ))
    }

    async fn get_kv(&self, _name: &str) -> Result<KvNamespace, BackendError> {
        Err(BackendError::Api(
            "KV requires API access. Use an API alias.".to_string(),
        ))
    }

    async fn create_kv(&self, _input: CreateKvInput) -> Result<KvNamespace, BackendError> {
        Err(BackendError::Api(
            "KV requires API access. Use an API alias.".to_string(),
        ))
    }

    async fn delete_kv(&self, _name: &str) -> Result<(), BackendError> {
        Err(BackendError::Api(
            "KV requires API access. Use an API alias.".to_string(),
        ))
    }

    // Database methods
    async fn list_databases(&self) -> Result<Vec<Database>, BackendError> {
        Err(BackendError::Api(
            "Databases require API access. Use an API alias.".to_string(),
        ))
    }

    async fn get_database(&self, _name: &str) -> Result<Database, BackendError> {
        Err(BackendError::Api(
            "Databases require API access. Use an API alias.".to_string(),
        ))
    }

    async fn create_database(&self, _input: CreateDatabaseInput) -> Result<Database, BackendError> {
        Err(BackendError::Api(
            "Databases require API access. Use an API alias.".to_string(),
        ))
    }

    async fn delete_database(&self, _name: &str) -> Result<(), BackendError> {
        Err(BackendError::Api(
            "Databases require API access. Use an API alias.".to_string(),
        ))
    }
}
