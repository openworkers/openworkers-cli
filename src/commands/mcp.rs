use rmcp::{
    ServerHandler, ServiceExt, handler::server::tool::ToolRouter,
    handler::server::wrapper::Parameters, model::*, tool, tool_handler, tool_router,
    transport::stdio,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;

use crate::backend::{
    Backend, CreateDatabaseInput, CreateKvInput, CreateStorageInput, CreateWorkerInput,
    DatabaseProvider, DeployInput, EnvironmentValueInput, UpdateEnvironmentInput, api::ApiBackend,
    db::DbBackend,
};
use crate::config::{AliasConfig, Config};

// Wrapper enum to make Backend usable without dyn
enum BackendWrapper {
    Api(ApiBackend),
    Db(DbBackend),
}

macro_rules! backend_call {
    ($backend:expr, $method:ident $(, $arg:expr)*) => {
        match &$backend {
            BackendWrapper::Api(b) => b.$method($($arg),*).await,
            BackendWrapper::Db(b) => b.$method($($arg),*).await,
        }
    };
}

// Helper macro for tool calls that return JSON results
macro_rules! tool_call {
    ($self:expr, $operation:expr, $method:ident $(, $arg:expr)*) => {{
        let backend = match $self.get_backend().await {
            Ok(b) => b,
            Err(e) => return format!("Error: {}", e),
        };

        match backend_call!(backend, $method $(, $arg)*) {
            Ok(result) => serde_json::to_string_pretty(&result).unwrap(),
            Err(e) => format!("Failed to {}: {}", $operation, e),
        }
    }};
}

// Helper macro for tool calls that return success messages
macro_rules! tool_call_success {
    ($self:expr, $operation:expr, $item:expr, $method:ident $(, $arg:expr)*) => {{
        let backend = match $self.get_backend().await {
            Ok(b) => b,
            Err(e) => return format!("Error: {}", e),
        };

        match backend_call!(backend, $method $(, $arg)*) {
            Ok(_) => format!("{{\"success\": true, \"message\": \"{} deleted\"}}", $item),
            Err(e) => format!("Failed to {} {}: {}", $operation, $item, e),
        }
    }};
}

#[derive(Clone)]
pub struct McpHandler {
    config: Config,
    alias: Option<String>,
    tool_router: ToolRouter<Self>,
}

// Request types

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct WorkersListRequest {}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct WorkersGetRequest {
    name: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct WorkersCreateRequest {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(default = "default_language")]
    language: String,
}

fn default_language() -> String {
    "typescript".to_string()
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct WorkersDeleteRequest {
    name: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct EnvListRequest {}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct KvListRequest {}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct KvCreateRequest {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct KvDeleteRequest {
    name: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct WorkersDeployRequest {
    name: String,
    file_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct WorkersLinkRequest {
    worker_name: String,
    env_name: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct EnvSetRequest {
    env_name: String,
    key: String,
    value: String,
    #[serde(default)]
    is_secret: bool,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct EnvBindRequest {
    env_name: String,
    key: String,
    resource_name: String,
    resource_type: String, // "assets", "storage", "kv", "database"
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct StorageListRequest {}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct StorageCreateRequest {
    name: String,
    provider: String,
    bucket: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    access_key_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    secret_access_key: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct StorageDeleteRequest {
    name: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct DatabasesListRequest {}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct DatabasesCreateRequest {
    name: String,
    provider: DatabaseProvider,
    #[serde(skip_serializing_if = "Option::is_none")]
    connection_string: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct DatabasesDeleteRequest {
    name: String,
}

#[tool_router]
impl McpHandler {
    fn new(config: Config, alias: Option<String>) -> Self {
        Self {
            config,
            alias,
            tool_router: Self::tool_router(),
        }
    }

    async fn get_backend(&self) -> Result<BackendWrapper, String> {
        let alias_name = self
            .alias
            .clone()
            .or(self.config.default.clone())
            .ok_or("No alias specified and no default configured")?;

        let alias_config = self
            .config
            .get_alias(&alias_name)
            .ok_or_else(|| format!("Alias '{}' not found", alias_name))?;

        match alias_config {
            AliasConfig::Db {
                database_url,
                user,
                storage,
            } => {
                let pool = PgPoolOptions::new()
                    .max_connections(1)
                    .connect(database_url)
                    .await
                    .map_err(|e| format!("Database connection error: {}", e))?;

                let backend = DbBackend::new(pool, user.clone(), storage.clone())
                    .await
                    .map_err(|e| format!("Backend error: {}", e))?;

                Ok(BackendWrapper::Db(backend))
            }

            AliasConfig::Api {
                url,
                token,
                insecure,
            } => {
                let backend = ApiBackend::new(url.clone(), token.clone(), *insecure);
                Ok(BackendWrapper::Api(backend))
            }
        }
    }

    #[tool(description = "List all workers")]
    async fn workers_list(&self, Parameters(_params): Parameters<WorkersListRequest>) -> String {
        tool_call!(self, "list workers", list_workers)
    }

    #[tool(description = "Get details of a specific worker")]
    async fn workers_get(
        &self,
        Parameters(WorkersGetRequest { name }): Parameters<WorkersGetRequest>,
    ) -> String {
        tool_call!(self, "get worker", get_worker, &name)
    }

    #[tool(description = "Create a new worker")]
    async fn workers_create(
        &self,
        Parameters(WorkersCreateRequest {
            name,
            description,
            language,
        }): Parameters<WorkersCreateRequest>,
    ) -> String {
        tool_call!(
            self,
            "create worker",
            create_worker,
            CreateWorkerInput {
                name,
                description,
                language,
            }
        )
    }

    #[tool(description = "Delete a worker")]
    async fn workers_delete(
        &self,
        Parameters(WorkersDeleteRequest { name }): Parameters<WorkersDeleteRequest>,
    ) -> String {
        tool_call_success!(self, "delete", &name, delete_worker, &name)
    }

    #[tool(description = "List all environments")]
    async fn env_list(&self, Parameters(_params): Parameters<EnvListRequest>) -> String {
        tool_call!(self, "list environments", list_environments)
    }

    #[tool(description = "List all KV namespaces")]
    async fn kv_list(&self, Parameters(_params): Parameters<KvListRequest>) -> String {
        tool_call!(self, "list KV namespaces", list_kv)
    }

    #[tool(description = "Create a new KV namespace")]
    async fn kv_create(
        &self,
        Parameters(KvCreateRequest { name, description }): Parameters<KvCreateRequest>,
    ) -> String {
        tool_call!(
            self,
            "create KV namespace",
            create_kv,
            CreateKvInput {
                name,
                desc: description,
            }
        )
    }

    #[tool(description = "Delete a KV namespace")]
    async fn kv_delete(
        &self,
        Parameters(KvDeleteRequest { name }): Parameters<KvDeleteRequest>,
    ) -> String {
        tool_call_success!(self, "delete", &name, delete_kv, &name)
    }

    #[tool(description = "Deploy code to a worker")]
    async fn workers_deploy(
        &self,
        Parameters(WorkersDeployRequest {
            name,
            file_path,
            message,
        }): Parameters<WorkersDeployRequest>,
    ) -> String {
        use std::path::PathBuf;

        let path = PathBuf::from(&file_path);

        if !path.exists() {
            return format!("Error: File not found: {}", file_path);
        }

        let code = match std::fs::read(&path) {
            Ok(c) => c,
            Err(e) => return format!("Error: Failed to read file: {}", e),
        };

        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");

        let code_type = match ext {
            "js" => "javascript",
            "ts" => "typescript",
            "wasm" => "wasm",
            _ => {
                return format!(
                    "Error: Unsupported file extension '{}'. Supported: .js, .ts, .wasm",
                    ext
                );
            }
        }
        .to_string();

        tool_call!(
            self,
            "deploy worker",
            deploy_worker,
            &name,
            DeployInput {
                code,
                code_type,
                message,
            }
        )
    }

    #[tool(description = "Link an environment to a worker")]
    async fn workers_link(
        &self,
        Parameters(WorkersLinkRequest {
            worker_name,
            env_name,
        }): Parameters<WorkersLinkRequest>,
    ) -> String {
        tool_call!(
            self,
            "link environment to worker",
            update_worker,
            &worker_name,
            crate::backend::UpdateWorkerInput {
                name: None,
                environment: Some(env_name),
            }
        )
    }

    #[tool(description = "Set an environment variable or secret")]
    async fn env_set(
        &self,
        Parameters(EnvSetRequest {
            env_name,
            key,
            value,
            is_secret,
        }): Parameters<EnvSetRequest>,
    ) -> String {
        let value_type = if is_secret { "secret" } else { "plain" }.to_string();

        tool_call!(
            self,
            "set environment variable",
            update_environment,
            &env_name,
            UpdateEnvironmentInput {
                name: None,
                values: Some(vec![EnvironmentValueInput {
                    id: None,
                    key,
                    value: Some(value),
                    value_type,
                }]),
            }
        )
    }

    #[tool(description = "Bind a resource (KV, database, storage) to an environment")]
    async fn env_bind(
        &self,
        Parameters(EnvBindRequest {
            env_name,
            key,
            resource_name,
            resource_type,
        }): Parameters<EnvBindRequest>,
    ) -> String {
        // Get resource ID based on type (matching CLI behavior)
        let backend = match self.get_backend().await {
            Ok(b) => b,
            Err(e) => return format!("Error: {}", e),
        };

        let resource_id = match resource_type.as_str() {
            "assets" | "storage" => match backend_call!(backend, get_storage, &resource_name) {
                Ok(storage) => storage.id,
                Err(e) => return format!("Failed to get storage '{}': {}", resource_name, e),
            },
            "kv" => match backend_call!(backend, get_kv, &resource_name) {
                Ok(kv) => kv.id,
                Err(e) => return format!("Failed to get KV '{}': {}", resource_name, e),
            },
            "database" => match backend_call!(backend, get_database, &resource_name) {
                Ok(db) => db.id,
                Err(e) => return format!("Failed to get database '{}': {}", resource_name, e),
            },
            _ => {
                return format!(
                    "Error: Invalid resource type '{}'. Valid types: assets, storage, kv, database",
                    resource_type
                );
            }
        };

        // Get current environment to find existing binding
        let env = match backend_call!(backend, get_environment, &env_name) {
            Ok(e) => e,
            Err(e) => return format!("Failed to get environment '{}': {}", env_name, e),
        };

        let existing_id = env
            .values
            .iter()
            .find(|v| v.key == key)
            .map(|v| v.id.clone());

        // Use resource_type directly as value_type (matching CLI)
        match backend_call!(
            backend,
            update_environment,
            &env_name,
            UpdateEnvironmentInput {
                name: None,
                values: Some(vec![EnvironmentValueInput {
                    id: existing_id,
                    key,
                    value: Some(resource_id),
                    value_type: resource_type,
                }]),
            }
        ) {
            Ok(result) => serde_json::to_string_pretty(&result).unwrap(),
            Err(e) => format!("Failed to bind resource to environment: {}", e),
        }
    }

    #[tool(description = "List all storage configurations")]
    async fn storage_list(&self, Parameters(_params): Parameters<StorageListRequest>) -> String {
        tool_call!(self, "list storage", list_storage)
    }

    #[tool(description = "Create a new storage configuration")]
    async fn storage_create(
        &self,
        Parameters(StorageCreateRequest {
            name,
            provider,
            bucket,
            endpoint,
            access_key_id,
            secret_access_key,
        }): Parameters<StorageCreateRequest>,
    ) -> String {
        tool_call!(
            self,
            "create storage",
            create_storage,
            CreateStorageInput {
                name,
                desc: None,
                provider,
                bucket: Some(bucket),
                prefix: None,
                access_key_id,
                secret_access_key,
                endpoint,
                region: None,
                public_url: None,
            }
        )
    }

    #[tool(description = "Delete a storage configuration")]
    async fn storage_delete(
        &self,
        Parameters(StorageDeleteRequest { name }): Parameters<StorageDeleteRequest>,
    ) -> String {
        tool_call_success!(self, "delete", &name, delete_storage, &name)
    }

    #[tool(description = "List all databases")]
    async fn databases_list(
        &self,
        Parameters(_params): Parameters<DatabasesListRequest>,
    ) -> String {
        tool_call!(self, "list databases", list_databases)
    }

    #[tool(description = "Create a new database")]
    async fn databases_create(
        &self,
        Parameters(DatabasesCreateRequest {
            name,
            provider,
            connection_string,
        }): Parameters<DatabasesCreateRequest>,
    ) -> String {
        tool_call!(
            self,
            "create database",
            create_database,
            CreateDatabaseInput {
                name,
                desc: None,
                provider,
                connection_string,
                max_rows: None,
                timeout_seconds: None,
            }
        )
    }

    #[tool(description = "Delete a database")]
    async fn databases_delete(
        &self,
        Parameters(DatabasesDeleteRequest { name }): Parameters<DatabasesDeleteRequest>,
    ) -> String {
        tool_call_success!(self, "delete", &name, delete_database, &name)
    }
}

#[tool_handler]
impl ServerHandler for McpHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            server_info: Implementation {
                name: "openworkers-cli".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                description: Some("OpenWorkers CLI MCP Server - Manage workers, environments, KV namespaces, storage, and databases".to_string()),
                website_url: None,
                title: Some("OpenWorkers CLI".to_string()),
                icons: None,
            },
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            instructions: Some("This server provides tools to manage OpenWorkers platform resources including workers (serverless functions), environments (configuration sets), and KV namespaces (key-value storage). The server uses the configured alias for authentication - if no alias is specified, it uses the default from the CLI config.".to_string()),
        }
    }
}

pub async fn run(alias: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load()?;
    let handler = McpHandler::new(config, alias);
    let service = handler.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
