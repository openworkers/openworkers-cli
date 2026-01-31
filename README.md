# OpenWorkers CLI

Command-line interface for managing OpenWorkers deployments.

## Installation

**Prebuilt binaries (recommended):**

```bash
# Using cargo-binstall (auto-detects prebuilt binaries)
cargo install cargo-binstall
cargo binstall openworkers-cli

# Or download manually from GitHub Releases
curl -L https://github.com/openworkers/openworkers-cli/releases/latest/download/ow-linux-x86_64.tar.gz | tar xz
sudo mv ow /usr/local/bin/

# macOS (Intel)
curl -L https://github.com/openworkers/openworkers-cli/releases/latest/download/ow-macos-x86_64.tar.gz | tar xz
sudo mv ow /usr/local/bin/

# macOS (Apple Silicon)
curl -L https://github.com/openworkers/openworkers-cli/releases/latest/download/ow-macos-aarch64.tar.gz | tar xz
sudo mv ow /usr/local/bin/
```

**Docker:**

```bash
docker run --rm ghcr.io/openworkers/openworkers-cli --help
```

**Build from source:**

```bash
cargo install --git https://github.com/openworkers/openworkers-cli
```

## Quick Start

```bash
# Configure your API backend
ow alias set prod --api https://dash.openworkers.com

# Login (prompts for API token)
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
| `users`     | `u`   | User management (DB only)      |
| `alias`     |       | Backend connection aliases     |
| `login`     |       | Authenticate with API          |
| `migrate`   |       | Database schema migrations     |

Common operations: `list` (`ls`), `get`, `create`, `delete` (`rm`)

## Workers

Workers are serverless functions deployed to the edge.

```bash
ow workers list
ow workers create my-api -d "REST API"
ow workers get my-api

# Deploy a single file
ow workers deploy my-api ./worker.ts -m "Initial deploy"

# Deploy a folder with worker.js + static assets (SvelteKit, etc.)
ow workers upload my-app ./dist

ow workers delete my-api
```

Supported file types: `.js`, `.ts`, `.wasm`

## Environments

Environments group configuration for your workers: variables, secrets, and bindings to resources.

```bash
ow env list
ow env create my-env -d "Production"
ow env get my-env

# Variables (plain text, visible in logs)
ow env set my-env API_URL "https://api.example.com"

# Secrets (encrypted, masked in output)
ow env set my-env API_KEY "secret" --secret

ow env unset my-env OLD_VAR

# Bindings connect resources to your worker code (accessible via env.CACHE, env.DB, etc.)
ow env bind my-env CACHE my-kv --type kv
ow env bind my-env DB my-db --type database
ow env bind my-env ASSETS my-storage --type assets

# Link environment to a worker
ow workers link my-api my-env

ow env delete old-env
```

## Storage

S3-compatible object storage for files, images, and static assets.

```bash
ow storage list

# Platform-managed storage
ow storage create my-storage

# Bring your own S3/R2 bucket
ow storage create my-s3 --provider s3 \
  --bucket my-bucket \
  --endpoint https://xxx.r2.cloudflarestorage.com \
  --access-key-id AKIA... \
  --secret-access-key ...

ow storage delete my-storage
```

## KV

Fast key-value store for caching, sessions, and feature flags.

```bash
ow kv list
ow kv create my-kv -d "API cache"
ow kv get my-kv
ow kv delete my-kv
```

## Databases

SQL databases for persistent data. Query with `env.DB.execute()` in your worker.

```bash
ow databases list

# Platform-managed database
ow databases create my-db

# Bring your own Postgres
ow databases create my-pg --provider postgres \
  --connection-string "postgres://user:pass@host/db"

ow databases delete my-db
```

## Aliases

Aliases let you manage multiple backends (production, staging, local) from the same CLI.

```bash
# API backend (hosted platform)
ow alias set prod --api https://dash.openworkers.com

# API backend (self-hosted, skip TLS verification)
ow alias set dev --api http://localhost:8080 --insecure

# DB backend (direct PostgreSQL access for migrations)
ow alias set local --db postgres://user:pass@localhost/ow --user admin@example.com

ow alias list
ow alias set-default prod
ow alias rm old-alias
```

Prefix any command with an alias name:

```bash
ow workers list           # Uses default alias
ow prod workers list      # Uses 'prod' alias
ow dev workers get my-api # Uses 'dev' alias
```

Config stored in `~/.openworkers/config.json`.

## Migrations

Database schema migrations for self-hosted deployments. Requires a DB alias.

```bash
ow local migrate status    # Show pending migrations
ow local migrate run       # Apply pending migrations
ow local migrate baseline  # Mark all as applied (for existing databases)
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
