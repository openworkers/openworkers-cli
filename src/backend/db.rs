use super::{Backend, DeployOptions, LogEntry, Worker};
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use sqlx::{FromRow, Row, postgres::PgPool};
use std::collections::HashMap;
use uuid::Uuid;

/// Direct database backend
pub struct DbBackend {
    pool: PgPool,
}

impl DbBackend {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPool::connect(database_url).await?;
        Ok(Self { pool })
    }
}

#[async_trait]
impl Backend for DbBackend {
    async fn list_workers(&self) -> Result<Vec<Worker>> {
        let rows: Vec<WorkerRow> = sqlx::query_as(
            r#"
            SELECT
                id,
                name,
                script as code,
                created_at,
                updated_at
            FROM workers
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    async fn get_worker(&self, id_or_name: &str) -> Result<Option<Worker>> {
        // Try as UUID first
        if let Ok(uuid) = Uuid::parse_str(id_or_name) {
            let row: Option<WorkerRow> = sqlx::query_as(
                r#"
                SELECT
                    id,
                    name,
                    script as code,
                    created_at,
                    updated_at
                FROM workers
                WHERE id = $1
                "#,
            )
            .bind(uuid)
            .fetch_optional(&self.pool)
            .await?;

            return Ok(row.map(|r| r.into()));
        }

        // Try as name
        let row: Option<WorkerRow> = sqlx::query_as(
            r#"
            SELECT
                id,
                name,
                script as code,
                created_at,
                updated_at
            FROM workers
            WHERE name = $1
            "#,
        )
        .bind(id_or_name)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into()))
    }

    async fn deploy(&self, options: DeployOptions) -> Result<Worker> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();

        sqlx::query(
            r#"
            INSERT INTO workers (id, name, script, language, created_at, updated_at)
            VALUES ($1, $2, $3, 'javascript', $4, $5)
            "#,
        )
        .bind(id)
        .bind(&options.name)
        .bind(&options.code)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        // TODO: Handle env vars via environments table

        Ok(Worker {
            id,
            name: options.name,
            code: options.code,
            env: options.env,
            created_at: now,
            updated_at: now,
        })
    }

    async fn update_code(&self, id: Uuid, code: &str) -> Result<Worker> {
        let now = chrono::Utc::now();

        let row = sqlx::query(
            r#"
            UPDATE workers
            SET script = $1, updated_at = $2
            WHERE id = $3
            RETURNING id, name, script as code, created_at, updated_at
            "#,
        )
        .bind(code)
        .bind(now)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Worker {
                id: row.get("id"),
                name: row.get("name"),
                code: row.get("code"),
                env: None,
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            }),
            None => Err(anyhow!("Worker not found")),
        }
    }

    async fn update_env(&self, _id: Uuid, _env: HashMap<String, String>) -> Result<Worker> {
        // TODO: Implement via environments table
        Err(anyhow!(
            "Environment update not yet implemented for DB backend"
        ))
    }

    async fn delete(&self, id: Uuid) -> Result<()> {
        let result = sqlx::query("DELETE FROM workers WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow!("Worker not found"));
        }

        Ok(())
    }

    async fn logs(&self, worker_id: Uuid, limit: i64) -> Result<Vec<LogEntry>> {
        let rows: Vec<LogRow> = sqlx::query_as(
            r#"
            SELECT
                date,
                worker_id,
                level::text as level,
                message
            FROM logs
            WHERE worker_id = $1
            ORDER BY date DESC
            LIMIT $2
            "#,
        )
        .bind(worker_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }
}

// Internal row types for sqlx
#[derive(FromRow)]
struct WorkerRow {
    id: Uuid,
    name: String,
    code: String,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<WorkerRow> for Worker {
    fn from(row: WorkerRow) -> Self {
        Worker {
            id: row.id,
            name: row.name,
            code: row.code,
            env: None,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(FromRow)]
struct LogRow {
    date: chrono::DateTime<chrono::Utc>,
    worker_id: Uuid,
    level: String,
    message: String,
}

impl From<LogRow> for LogEntry {
    fn from(row: LogRow) -> Self {
        LogEntry {
            date: row.date,
            worker_id: row.worker_id,
            level: row.level,
            message: row.message,
        }
    }
}
