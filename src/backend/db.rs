use super::{
    Backend, BackendError, CreateDatabaseInput, CreateEnvironmentInput, CreateKvInput,
    CreateStorageInput, CreateWorkerInput, Database, DeployInput, Deployment, Environment,
    EnvironmentValue, KvNamespace, StorageConfig, UpdateEnvironmentInput, UpdateWorkerInput,
    UploadResult, UploadWorkerInfo, UploadedCounts, Worker,
};
use crate::s3::{S3Client, S3Config, get_mime_type};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::io::Read;
use zip::ZipArchive;

pub struct DbBackend {
    pool: PgPool,
    user_id: uuid::Uuid,
}

impl DbBackend {
    pub async fn new(pool: PgPool) -> Result<Self, BackendError> {
        // Get or create admin user on initialization
        let user_id: uuid::Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO users (id, username, created_at, updated_at)
            VALUES (gen_random_uuid(), 'cli-admin', now(), now())
            ON CONFLICT (username) DO UPDATE SET username = users.username
            RETURNING id
            "#,
        )
        .fetch_one(&pool)
        .await?;

        Ok(Self { pool, user_id })
    }

    async fn get_environment_values(
        &self,
        env_id: &uuid::Uuid,
    ) -> Result<Vec<EnvironmentValue>, BackendError> {
        let rows = sqlx::query(
            r#"
            SELECT id, key, value, type::text as value_type
            FROM environment_values
            WHERE environment_id = $1
            ORDER BY key
            "#,
        )
        .bind(env_id)
        .fetch_all(&self.pool)
        .await?;

        let values = rows
            .iter()
            .map(|row| EnvironmentValue {
                id: row.get::<uuid::Uuid, _>("id").to_string(),
                key: row.get("key"),
                value: row.get("value"),
                value_type: row.get("value_type"),
            })
            .collect();

        Ok(values)
    }
}

impl Backend for DbBackend {
    async fn list_workers(&self) -> Result<Vec<Worker>, BackendError> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, "desc", current_version, created_at, updated_at
            FROM workers
            WHERE user_id = $1
            ORDER BY name
            "#,
        )
        .bind(self.user_id)
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
            WHERE name = $1 AND user_id = $2
            "#,
        )
        .bind(name)
        .bind(self.user_id)
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
        let row = sqlx::query(
            r#"
            INSERT INTO workers (name, "desc", user_id)
            VALUES ($1, $2, $3)
            RETURNING id, name, "desc", current_version, created_at, updated_at
            "#,
        )
        .bind(&input.name)
        .bind(&input.description)
        .bind(self.user_id)
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
        let result = sqlx::query("DELETE FROM workers WHERE name = $1 AND user_id = $2")
            .bind(name)
            .bind(self.user_id)
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
        // Get environment_id if environment is provided (accepts name or UUID)
        let env_id: Option<uuid::Uuid> = if let Some(env_ref) = &input.environment {
            // Try parsing as UUID first, then lookup by name
            if let Ok(uuid) = env_ref.parse::<uuid::Uuid>() {
                Some(uuid)
            } else {
                Some(
                    sqlx::query_scalar(
                        "SELECT id FROM environments WHERE name = $1 AND user_id = $2",
                    )
                    .bind(env_ref)
                    .bind(self.user_id)
                    .fetch_optional(&self.pool)
                    .await?
                    .ok_or_else(|| {
                        BackendError::NotFound(format!("Environment '{}' not found", env_ref))
                    })?,
                )
            }
        } else {
            None
        };

        let row = sqlx::query(
            r#"
            UPDATE workers
            SET environment_id = COALESCE($2, environment_id),
                updated_at = now()
            WHERE name = $1 AND user_id = $3
            RETURNING id, name, "desc", current_version, created_at, updated_at
            "#,
        )
        .bind(name)
        .bind(env_id)
        .bind(self.user_id)
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
        name: &str,
        zip_data: Vec<u8>,
    ) -> Result<UploadResult, BackendError> {
        // 1. Get worker by name
        let worker = self.get_worker(name).await?;
        let worker_id: uuid::Uuid = worker
            .id
            .parse()
            .map_err(|_| BackendError::Api(format!("Invalid worker ID: {}", worker.id)))?;

        // 2. Get ASSETS binding for this worker
        let assets_binding = sqlx::query(
            r#"
            SELECT
                sc.id as storage_config_id,
                sc.bucket,
                sc.prefix,
                sc.access_key_id,
                sc.secret_access_key,
                sc.endpoint,
                sc.region
            FROM workers w
            JOIN environment_values ev ON ev.environment_id = w.environment_id
            JOIN storage_configs sc ON sc.id = ev.value::uuid
            WHERE w.id = $1 AND ev.type = 'assets'
            LIMIT 1
            "#,
        )
        .bind(worker_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| {
            BackendError::Api(
                "Worker has no ASSETS binding. Add an assets binding to the worker environment first.".to_string(),
            )
        })?;

        let bucket: String = assets_binding.get("bucket");
        let prefix: Option<String> = assets_binding.get("prefix");
        let access_key_id: String = assets_binding.get("access_key_id");
        let secret_access_key: String = assets_binding.get("secret_access_key");
        let endpoint: Option<String> = assets_binding.get("endpoint");
        let region: Option<String> = assets_binding.get("region");

        let endpoint = endpoint
            .ok_or_else(|| BackendError::Api("Storage endpoint not configured".to_string()))?;

        // 3. Extract zip
        let cursor = std::io::Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor)
            .map_err(|e| BackendError::Api(format!("Failed to read zip archive: {}", e)))?;

        let mut worker_script: Option<String> = None;
        let mut language = "javascript";
        let mut assets: Vec<(String, Vec<u8>)> = Vec::new();

        for i in 0..archive.len() {
            let mut file = archive
                .by_index(i)
                .map_err(|e| BackendError::Api(format!("Failed to read zip entry: {}", e)))?;

            if file.is_dir() {
                continue;
            }

            let filename = file.name().to_string();

            // Normalize path (remove leading directory if present)
            let normalized = filename
                .split('/')
                .skip_while(|s| !s.contains('.'))
                .collect::<Vec<_>>()
                .join("/");

            let check_name = if normalized.is_empty() {
                &filename
            } else {
                &normalized
            };

            if check_name == "worker.js"
                || check_name == "worker.ts"
                || check_name == "_worker.js"
                || check_name == "_worker.ts"
            {
                let mut content = String::new();
                file.read_to_string(&mut content).map_err(|e| {
                    BackendError::Api(format!("Failed to read worker script: {}", e))
                })?;
                worker_script = Some(content);
                language = if check_name.ends_with(".ts") {
                    "typescript"
                } else {
                    "javascript"
                };
            } else if filename.contains("assets/") {
                // Extract asset path after "assets/"
                if let Some(pos) = filename.find("assets/") {
                    let asset_path = &filename[pos + 7..];

                    if !asset_path.is_empty() {
                        let mut content = Vec::new();
                        file.read_to_end(&mut content).map_err(|e| {
                            BackendError::Api(format!("Failed to read asset: {}", e))
                        })?;
                        assets.push((asset_path.to_string(), content));
                    }
                }
            }
        }

        let script = worker_script.ok_or_else(|| {
            BackendError::Api("No worker.js or worker.ts found in zip archive".to_string())
        })?;

        // 4. Update worker script in DB
        let script_bytes = script.as_bytes();
        let mut hasher = Sha256::new();
        hasher.update(script_bytes);
        let hash = hex::encode(hasher.finalize());

        // Get next version
        let current_version: Option<i32> =
            sqlx::query_scalar("SELECT MAX(version) FROM worker_deployments WHERE worker_id = $1")
                .bind(worker_id)
                .fetch_one(&self.pool)
                .await?;

        let next_version = current_version.unwrap_or(0) + 1;

        // Insert deployment
        sqlx::query(
            r#"
            INSERT INTO worker_deployments (worker_id, version, hash, code_type, code, message)
            VALUES ($1, $2, $3, $4::enum_code_type, $5, 'Upload via CLI')
            "#,
        )
        .bind(worker_id)
        .bind(next_version)
        .bind(&hash)
        .bind(language)
        .bind(script_bytes)
        .fetch_optional(&self.pool)
        .await?;

        // Update worker's current_version
        sqlx::query("UPDATE workers SET current_version = $1 WHERE id = $2")
            .bind(next_version)
            .bind(worker_id)
            .execute(&self.pool)
            .await?;

        // 5. Upload assets to S3
        let s3_client = S3Client::new(S3Config {
            bucket,
            endpoint,
            access_key_id,
            secret_access_key,
            region: region.unwrap_or_else(|| "auto".to_string()),
            prefix,
        });

        let mut uploaded_count = 0;

        for (path, content) in assets {
            let content_type = get_mime_type(&path);

            match s3_client.put(&path, content, content_type).await {
                Ok(true) => uploaded_count += 1,
                Ok(false) => eprintln!("Failed to upload {}", path),
                Err(e) => eprintln!("Error uploading {}: {}", path, e),
            }
        }

        Ok(UploadResult {
            success: true,
            worker: UploadWorkerInfo {
                id: worker.id,
                name: worker.name,
                url: format!("https://{}.workers.rocks", name),
            },
            uploaded: UploadedCounts {
                script: true,
                assets: uploaded_count,
            },
        })
    }

    async fn list_environments(&self) -> Result<Vec<Environment>, BackendError> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, "desc", created_at, updated_at
            FROM environments
            WHERE user_id = $1
            ORDER BY name
            "#,
        )
        .bind(self.user_id)
        .fetch_all(&self.pool)
        .await?;

        let mut environments = Vec::new();

        for row in rows {
            let id: uuid::Uuid = row.get("id");
            let values = self.get_environment_values(&id).await?;

            environments.push(Environment {
                id: id.to_string(),
                name: row.get("name"),
                description: row.get("desc"),
                values,
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            });
        }

        Ok(environments)
    }

    async fn get_environment(&self, name: &str) -> Result<Environment, BackendError> {
        let row = sqlx::query(
            r#"
            SELECT id, name, "desc", created_at, updated_at
            FROM environments
            WHERE name = $1 AND user_id = $2
            "#,
        )
        .bind(name)
        .bind(self.user_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| BackendError::NotFound(format!("Environment '{}' not found", name)))?;

        let id: uuid::Uuid = row.get("id");
        let values = self.get_environment_values(&id).await?;

        Ok(Environment {
            id: id.to_string(),
            name: row.get("name"),
            description: row.get("desc"),
            values,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    async fn create_environment(
        &self,
        input: CreateEnvironmentInput,
    ) -> Result<Environment, BackendError> {
        let row = sqlx::query(
            r#"
            INSERT INTO environments (name, "desc", user_id)
            VALUES ($1, $2, $3)
            RETURNING id, name, "desc", created_at, updated_at
            "#,
        )
        .bind(&input.name)
        .bind(&input.desc)
        .bind(self.user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(Environment {
            id: row.get::<uuid::Uuid, _>("id").to_string(),
            name: row.get("name"),
            description: row.get("desc"),
            values: vec![],
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    async fn update_environment(
        &self,
        name: &str,
        input: UpdateEnvironmentInput,
    ) -> Result<Environment, BackendError> {
        // Get environment ID
        let row = sqlx::query("SELECT id FROM environments WHERE name = $1 AND user_id = $2")
            .bind(name)
            .bind(self.user_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| BackendError::NotFound(format!("Environment '{}' not found", name)))?;

        let env_id: uuid::Uuid = row.get("id");

        // Update name if provided
        if let Some(new_name) = &input.name {
            sqlx::query("UPDATE environments SET name = $1, updated_at = now() WHERE id = $2")
                .bind(new_name)
                .bind(env_id)
                .execute(&self.pool)
                .await?;
        }

        // Update values if provided
        if let Some(values) = &input.values {
            for value in values {
                if let Some(id) = &value.id {
                    // Update existing value
                    let value_id: uuid::Uuid = id
                        .parse()
                        .map_err(|_| BackendError::Api(format!("Invalid value ID: {}", id)))?;

                    if let Some(val) = &value.value {
                        sqlx::query(
                            r#"
                            UPDATE environment_values
                            SET key = $1, value = $2, type = $3::enum_binding_type
                            WHERE id = $4
                            "#,
                        )
                        .bind(&value.key)
                        .bind(val)
                        .bind(&value.value_type)
                        .bind(value_id)
                        .execute(&self.pool)
                        .await?;
                    }
                } else if let Some(val) = &value.value {
                    // Create new value
                    sqlx::query(
                        r#"
                        INSERT INTO environment_values (environment_id, user_id, key, value, type)
                        VALUES ($1, $2, $3, $4, $5::enum_binding_type)
                        "#,
                    )
                    .bind(env_id)
                    .bind(self.user_id)
                    .bind(&value.key)
                    .bind(val)
                    .bind(&value.value_type)
                    .execute(&self.pool)
                    .await?;
                }
            }
        }

        // Return updated environment
        let final_name = input.name.as_deref().unwrap_or(name);
        self.get_environment(final_name).await
    }

    async fn delete_environment(&self, name: &str) -> Result<(), BackendError> {
        let result = sqlx::query("DELETE FROM environments WHERE name = $1 AND user_id = $2")
            .bind(name)
            .bind(self.user_id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(BackendError::NotFound(format!(
                "Environment '{}' not found",
                name
            )));
        }

        Ok(())
    }

    // Storage methods
    async fn list_storage(&self) -> Result<Vec<StorageConfig>, BackendError> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, "desc", 'r2' as provider, bucket, prefix, endpoint, region, public_url, created_at, updated_at
            FROM storage_configs
            WHERE user_id = $1
            ORDER BY name
            "#,
        )
        .bind(self.user_id)
        .fetch_all(&self.pool)
        .await?;

        let configs = rows
            .iter()
            .map(|row| StorageConfig {
                id: row.get::<uuid::Uuid, _>("id").to_string(),
                name: row.get("name"),
                description: row.get("desc"),
                provider: row.get("provider"),
                bucket: row.get("bucket"),
                prefix: row.get("prefix"),
                endpoint: row.get("endpoint"),
                region: row.get("region"),
                public_url: row.get("public_url"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();

        Ok(configs)
    }

    async fn get_storage(&self, name: &str) -> Result<StorageConfig, BackendError> {
        let row = sqlx::query(
            r#"
            SELECT id, name, "desc", 'r2' as provider, bucket, prefix, endpoint, region, public_url, created_at, updated_at
            FROM storage_configs
            WHERE name = $1 AND user_id = $2
            "#,
        )
        .bind(name)
        .bind(self.user_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| BackendError::NotFound(format!("Storage config '{}' not found", name)))?;

        Ok(StorageConfig {
            id: row.get::<uuid::Uuid, _>("id").to_string(),
            name: row.get("name"),
            description: row.get("desc"),
            provider: row.get("provider"),
            bucket: row.get("bucket"),
            prefix: row.get("prefix"),
            endpoint: row.get("endpoint"),
            region: row.get("region"),
            public_url: row.get("public_url"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    async fn create_storage(
        &self,
        input: CreateStorageInput,
    ) -> Result<StorageConfig, BackendError> {
        let row = sqlx::query(
            r#"
            INSERT INTO storage_configs (name, "desc", user_id, bucket, prefix, access_key_id, secret_access_key, endpoint, region, public_url)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id, name, "desc", bucket, prefix, endpoint, region, public_url, created_at, updated_at
            "#,
        )
        .bind(&input.name)
        .bind(&input.desc)
        .bind(self.user_id)
        .bind(&input.bucket)
        .bind(&input.prefix)
        .bind(&input.access_key_id)
        .bind(&input.secret_access_key)
        .bind(&input.endpoint)
        .bind(&input.region)
        .bind(&input.public_url)
        .fetch_one(&self.pool)
        .await?;

        Ok(StorageConfig {
            id: row.get::<uuid::Uuid, _>("id").to_string(),
            name: row.get("name"),
            description: row.get("desc"),
            provider: input.provider,
            bucket: row.get("bucket"),
            prefix: row.get("prefix"),
            endpoint: row.get("endpoint"),
            region: row.get("region"),
            public_url: row.get("public_url"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    async fn delete_storage(&self, name: &str) -> Result<(), BackendError> {
        let result = sqlx::query("DELETE FROM storage_configs WHERE name = $1 AND user_id = $2")
            .bind(name)
            .bind(self.user_id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(BackendError::NotFound(format!(
                "Storage config '{}' not found",
                name
            )));
        }

        Ok(())
    }

    // KV methods
    async fn list_kv(&self) -> Result<Vec<KvNamespace>, BackendError> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, "desc", created_at, updated_at
            FROM kv_configs
            WHERE user_id = $1
            ORDER BY name
            "#,
        )
        .bind(self.user_id)
        .fetch_all(&self.pool)
        .await?;

        let namespaces = rows
            .iter()
            .map(|row| KvNamespace {
                id: row.get::<uuid::Uuid, _>("id").to_string(),
                name: row.get("name"),
                description: row.get("desc"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();

        Ok(namespaces)
    }

    async fn get_kv(&self, name: &str) -> Result<KvNamespace, BackendError> {
        let row = sqlx::query(
            r#"
            SELECT id, name, "desc", created_at, updated_at
            FROM kv_configs
            WHERE name = $1 AND user_id = $2
            "#,
        )
        .bind(name)
        .bind(self.user_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| BackendError::NotFound(format!("KV namespace '{}' not found", name)))?;

        Ok(KvNamespace {
            id: row.get::<uuid::Uuid, _>("id").to_string(),
            name: row.get("name"),
            description: row.get("desc"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    async fn create_kv(&self, input: CreateKvInput) -> Result<KvNamespace, BackendError> {
        let row = sqlx::query(
            r#"
            INSERT INTO kv_configs (name, "desc", user_id)
            VALUES ($1, $2, $3)
            RETURNING id, name, "desc", created_at, updated_at
            "#,
        )
        .bind(&input.name)
        .bind(&input.desc)
        .bind(self.user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(KvNamespace {
            id: row.get::<uuid::Uuid, _>("id").to_string(),
            name: row.get("name"),
            description: row.get("desc"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    async fn delete_kv(&self, name: &str) -> Result<(), BackendError> {
        let result = sqlx::query("DELETE FROM kv_configs WHERE name = $1 AND user_id = $2")
            .bind(name)
            .bind(self.user_id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(BackendError::NotFound(format!(
                "KV namespace '{}' not found",
                name
            )));
        }

        Ok(())
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
