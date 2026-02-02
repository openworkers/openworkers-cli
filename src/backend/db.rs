use super::{
    Backend, BackendError, CreateDatabaseInput, CreateEnvironmentInput, CreateKvInput,
    CreateStorageInput, CreateWorkerInput, Database, DeployInput, Deployment, Environment,
    EnvironmentValue, KvNamespace, StorageConfig, UpdateEnvironmentInput, UpdateWorkerInput,
    UploadResult, UploadWorkerInfo, UploadedCounts, Worker,
};
use crate::config::PlatformStorageConfig;
use crate::s3::{S3Client, S3Config, get_mime_type};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::io::Read;
use zip::ZipArchive;

#[derive(Debug, Deserialize)]
struct RoutesConfig {
    #[serde(default)]
    immutable: Vec<String>,
    #[serde(rename = "static", default)]
    static_routes: Vec<String>,
    #[serde(default)]
    prerendered: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)]
    functions: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)]
    ssr: Vec<String>,
}

pub struct DbBackend {
    pool: PgPool,
    user_id: uuid::Uuid,
    platform_storage: Option<PlatformStorageConfig>,
}

/// Helper to create storage routes for a project
async fn create_storage_routes(
    pool: &PgPool,
    project_id: uuid::Uuid,
    patterns: &[String],
    priority: i32,
) -> Result<(), BackendError> {
    for pattern in patterns {
        sqlx::query(
            r#"
            INSERT INTO project_routes (project_id, pattern, priority, backend_type)
            VALUES ($1, $2, $3, 'storage'::enum_backend_type)
            ON CONFLICT (project_id, pattern) DO UPDATE SET priority = $3, backend_type = 'storage'::enum_backend_type
            "#,
        )
        .bind(project_id)
        .bind(pattern)
        .bind(priority)
        .execute(pool)
        .await?;
    }
    Ok(())
}

impl DbBackend {
    pub async fn new(
        pool: PgPool,
        username: Option<String>,
        platform_storage: Option<PlatformStorageConfig>,
    ) -> Result<Self, BackendError> {
        let username = username.ok_or_else(|| {
            BackendError::Api(
                "No user configured for this DB alias. Use 'ow alias set <name> --db <url> --user <username>' to set a user.".to_string(),
            )
        })?;

        // Look up user by username
        let user_id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM users WHERE username = $1")
            .bind(&username)
            .fetch_optional(&pool)
            .await?
            .ok_or_else(|| {
                BackendError::NotFound(format!(
                    "User '{}' not found. Create an account first via the dashboard.",
                    username
                ))
            })?;

        Ok(Self {
            pool,
            user_id,
            platform_storage,
        })
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
            SELECT w.id, w.name, w."desc", w.current_version, w.created_at, w.updated_at,
                   e.id as env_id, e.name as env_name
            FROM workers w
            LEFT JOIN environments e ON e.id = w.environment_id
            WHERE w.user_id = $1
            ORDER BY w.name
            "#,
        )
        .bind(self.user_id)
        .fetch_all(&self.pool)
        .await?;

        let workers = rows
            .iter()
            .map(|row| {
                let env_id: Option<uuid::Uuid> = row.get("env_id");
                let env_name: Option<String> = row.get("env_name");
                let environment =
                    env_id
                        .zip(env_name)
                        .map(|(id, name)| super::WorkerEnvironmentRef {
                            id: id.to_string(),
                            name,
                        });

                Worker {
                    id: row.get::<uuid::Uuid, _>("id").to_string(),
                    name: row.get("name"),
                    description: row.get("desc"),
                    current_version: row.get("current_version"),
                    environment,
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                }
            })
            .collect();

        Ok(workers)
    }

    async fn get_worker(&self, name: &str) -> Result<Worker, BackendError> {
        let row = sqlx::query(
            r#"
            SELECT w.id, w.name, w."desc", w.current_version, w.created_at, w.updated_at,
                   e.id as env_id, e.name as env_name
            FROM workers w
            LEFT JOIN environments e ON e.id = w.environment_id
            WHERE w.name = $1 AND w.user_id = $2
            "#,
        )
        .bind(name)
        .bind(self.user_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| BackendError::NotFound(format!("Worker '{}' not found", name)))?;

        let env_id: Option<uuid::Uuid> = row.get("env_id");
        let env_name: Option<String> = row.get("env_name");
        let environment = env_id
            .zip(env_name)
            .map(|(id, name)| super::WorkerEnvironmentRef {
                id: id.to_string(),
                name,
            });

        Ok(Worker {
            id: row.get::<uuid::Uuid, _>("id").to_string(),
            name: row.get("name"),
            description: row.get("desc"),
            current_version: row.get("current_version"),
            environment,
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
            environment: None,
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

        let result = sqlx::query(
            r#"
            UPDATE workers
            SET environment_id = COALESCE($2, environment_id),
                updated_at = now()
            WHERE name = $1 AND user_id = $3
            RETURNING id
            "#,
        )
        .bind(name)
        .bind(env_id)
        .bind(self.user_id)
        .fetch_optional(&self.pool)
        .await?;

        if result.is_none() {
            return Err(BackendError::NotFound(format!(
                "Worker '{}' not found",
                name
            )));
        }

        // Fetch updated worker with environment info
        self.get_worker(name).await
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

        // 2. Check for ASSETS binding (like API does)
        let assets_binding = sqlx::query(
            r#"
            SELECT
                sc.bucket,
                sc.prefix,
                sc.access_key_id,
                sc.secret_access_key,
                sc.endpoint,
                sc.region
            FROM workers w
            JOIN environment_values ev ON ev.environment_id = w.environment_id
            JOIN storage_configs sc ON sc.id = ev.value::uuid
            WHERE w.id = $1 AND w.user_id = $2 AND ev.type = 'assets'
            LIMIT 1
            "#,
        )
        .bind(worker_id)
        .bind(self.user_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| {
            BackendError::Api(
                "Worker has no ASSETS binding. Add an assets binding to the worker environment first.".to_string(),
            )
        })?;

        // 3. Get storage credentials from binding, with platform endpoint as fallback
        let bucket: String = assets_binding.get("bucket");
        let prefix: Option<String> = assets_binding.get("prefix");
        let access_key_id: String = assets_binding.get("access_key_id");
        let secret_access_key: String = assets_binding.get("secret_access_key");
        let region: String = assets_binding
            .get::<Option<String>, _>("region")
            .unwrap_or_else(|| "auto".to_string());

        // Use binding's endpoint, or fall back to platform storage endpoint
        let binding_endpoint: Option<String> = assets_binding.get("endpoint");
        let endpoint = binding_endpoint
            .or_else(|| self.platform_storage.as_ref().map(|ps| ps.endpoint.clone()))
            .ok_or_else(|| BackendError::Api("Storage endpoint not configured".to_string()))?;

        // 4. Extract zip
        let cursor = std::io::Cursor::new(zip_data);
        let mut archive = ZipArchive::new(cursor)
            .map_err(|e| BackendError::Api(format!("Failed to read zip archive: {}", e)))?;

        let mut worker_script: Option<String> = None;
        let mut language = "javascript";
        let mut assets: Vec<(String, Vec<u8>)> = Vec::new();
        let mut routes_json: Option<String> = None;

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
            } else if check_name == "_routes.json" {
                let mut content = String::new();
                file.read_to_string(&mut content).map_err(|e| {
                    BackendError::Api(format!("Failed to read _routes.json: {}", e))
                })?;
                routes_json = Some(content);
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

        // 5. Update worker script in DB
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

        // 6. Upload assets to S3
        let s3_client = S3Client::new(S3Config {
            bucket,
            endpoint,
            access_key_id,
            secret_access_key,
            region,
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

        // 7. Ensure worker is in a project with routes
        let project_id: Option<uuid::Uuid> =
            sqlx::query_scalar("SELECT project_id FROM workers WHERE id = $1")
                .bind(worker_id)
                .fetch_one(&self.pool)
                .await?;

        let proj_id = if let Some(pid) = project_id {
            // Worker already in project
            pid
        } else {
            // Upgrade worker to project (creates project with same ID and catch-all route)
            sqlx::query("SELECT upgrade_worker_to_project($1)")
                .bind(worker_id)
                .execute(&self.pool)
                .await?;

            // Get the project_id (same as worker_id)
            worker_id
        };

        // 8. Parse routes.json if present and create routes
        if let Some(routes_content) = routes_json {
            let routes: RoutesConfig = serde_json::from_str(&routes_content)
                .map_err(|e| BackendError::Api(format!("Failed to parse routes.json: {}", e)))?;

            // Delete existing routes (except catch-all at priority 0)
            sqlx::query("DELETE FROM project_routes WHERE project_id = $1 AND priority > 0")
                .bind(proj_id)
                .execute(&self.pool)
                .await?;

            // Create storage routes with different priorities
            create_storage_routes(&self.pool, proj_id, &routes.immutable, 3).await?;
            create_storage_routes(&self.pool, proj_id, &routes.static_routes, 2).await?;
            create_storage_routes(&self.pool, proj_id, &routes.prerendered, 1).await?;

            // SSR routes are handled by the catch-all route at priority 0 created by upgrade_worker_to_project
        }

        // 9. Try to find custom domain for this worker or project
        let custom_domain: Option<String> = sqlx::query_scalar(
            r#"
            SELECT name FROM domains
            WHERE worker_id = $1 OR project_id = $1
            LIMIT 1
            "#,
        )
        .bind(worker_id)
        .fetch_optional(&self.pool)
        .await?;

        // Build URL: custom domain if available, otherwise just worker name (no URL)
        let url = if let Some(domain) = custom_domain {
            format!("https://{}", domain)
        } else {
            // No URL - will be handled by CLI layer
            name.to_string()
        };

        Ok(UploadResult {
            success: true,
            worker: UploadWorkerInfo {
                id: worker.id,
                name: worker.name,
                url,
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
