# OpenWorkers CLI

Command-line interface for managing OpenWorkers deployments.

## Quick Start

```bash
# Install
cargo install --path .

# Login (opens browser)
ow login

# Create and deploy a worker
ow workers create my-api
ow workers deploy my-api worker.ts

# Your worker is live at https://my-api.workers.rocks
```

## Commands

| Command     | Short | Description                    |
| ----------- | ----- | ------------------------------ |
| `workers`   | `w`   | Create, deploy, manage workers |
| `env`       | `e`   | Environment variables/secrets  |
| `storage`   | `s`   | S3/R2 storage configurations   |
| `kv`        | `k`   | Key-value namespaces           |
| `databases` | `d`   | SQL database bindings          |
| `alias`     |       | Backend connection aliases     |
| `login`     |       | Authenticate with API          |
| `migrate`   |       | Database schema migrations     |

Common operations: `list` (`ls`), `get`, `create`, `delete` (`rm`)

## Workers

```bash
ow workers list
ow workers create my-api -d "REST API"
ow workers get my-api
ow workers deploy my-api ./worker.ts -m "Initial deploy"
ow workers upload my-app ./dist      # Upload folder with assets
ow workers link my-api --env my-env  # Link environment
ow workers delete my-api
```

Supported file types: `.js`, `.ts`, `.wasm`

## Environments

Manage variables, secrets, and resource bindings.

```bash
ow env list
ow env create my-env -d "Production"
ow env get my-env

# Variables and secrets
ow env set my-env API_URL "https://api.example.com"
ow env set my-env API_KEY "secret" --secret
ow env unset my-env OLD_VAR

# Bind resources
ow env bind my-env CACHE my-kv --type kv
ow env bind my-env DB my-db --type database
ow env bind my-env ASSETS my-storage --type assets

ow env delete old-env
```

## Storage

S3-compatible storage (R2, MinIO, AWS S3).

```bash
ow storage list
ow storage create my-storage
ow storage create my-s3 --provider s3 \
  --bucket my-bucket \
  --endpoint https://xxx.r2.cloudflarestorage.com \
  --access-key-id AKIA... \
  --secret-access-key ...
ow storage delete my-storage
```

## KV

Key-value namespaces for caching and sessions.

```bash
ow kv list
ow kv create cache -d "API cache"
ow kv get cache
ow kv delete cache
```

## Databases

SQL database bindings.

```bash
ow databases list
ow databases create my-db
ow databases create my-pg --provider postgres \
  --connection-string "postgres://user:pass@host/db"
ow databases delete my-db
```

## Aliases

The CLI supports multiple backends via aliases. Config is stored in `~/.openworkers/config.json`.

```bash
# API backend (hosted or self-hosted)
ow alias set prod --api https://dash.openworkers.com
ow alias set dev --api http://localhost:8080 --insecure

# DB backend (direct PostgreSQL, for migrations)
ow alias set local --db postgres://user:pass@localhost/ow --user admin@example.com

# Manage aliases
ow alias list
ow alias set-default prod
ow alias rm old-alias
```

Use an alias by prefixing commands:

```bash
ow workers list           # Uses default alias
ow prod workers list      # Uses 'prod' alias
ow local migrate run      # Uses 'local' alias
```

## Migrations

Database schema migrations (requires DB alias).

```bash
ow local migrate status    # Show pending migrations
ow local migrate run       # Apply migrations
ow local migrate baseline  # Mark all as applied (existing DB)
```

## Config File

```json
{
  "version": 1,
  "default": "prod",
  "aliases": {
    "prod": {
      "type": "api",
      "url": "https://dash.openworkers.com/api/v1",
      "token": "ow_xxx"
    },
    "local": {
      "type": "db",
      "database_url": "postgres://localhost/openworkers",
      "user": "admin@example.com"
    }
  }
}
```

## Development

```bash
cargo build
cargo run -- workers list
cargo run -- local migrate status
```
