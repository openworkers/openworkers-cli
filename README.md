# OpenWorkers CLI

Command-line interface for managing OpenWorkers deployments.

## Installation

```bash
cargo install --path .
```

## Configuration

The CLI uses aliases to connect to different backends. Config is stored in `~/.openworkers/config.json`.

### Alias Types

- **API**: Connect via REST API (hosted or self-hosted)
- **DB**: Direct PostgreSQL connection (for infra/migrations)

### Managing Aliases

```bash
# Add API alias
ow alias set prod --api https://dash.openworkers.com/api/v1 --token <token>
ow alias set dev --api http://localhost:7000 --token <token>

# Add DB alias (for infrastructure operations)
ow alias set infra --db postgres://user:pass@host/db

# List aliases
ow alias list

# Set default
ow alias set-default prod

# Remove alias
ow alias rm old-alias
```

### Default Alias

On first run, a `default` alias pointing to `https://dash.openworkers.com/api/v1` is created as default.

## Command Shortcuts

| Command | Alias | Description |
|---------|-------|-------------|
| `workers list` | `workers ls` | List all workers |
| `workers delete` | `workers rm` | Delete a worker |
| `env list` | `env ls` | List environments |
| `env delete` | `env rm` | Delete environment |
| `kv list` | `kv ls` | List KV namespaces |
| `kv delete` | `kv rm` | Delete KV namespace |
| `storage list` | `storage ls` | List storage configs |
| `storage delete` | `storage rm` | Delete storage config |
| `databases list` | `databases ls` | List databases |
| `databases delete` | `databases rm` | Delete database |
| `alias list` | `alias ls` | List aliases |
| `alias remove` | `alias rm` | Remove alias |

## Commands

### Login

```bash
# Set token on default alias
ow login

# Set token on specific alias
ow dev login
```

### Workers

```bash
# List workers
ow workers list
ow workers ls

# Create a worker
ow workers create my-api -d "My API worker"
ow workers create my-api --language javascript

# Get worker details
ow workers get my-api

# Deploy code to a worker
ow workers deploy my-api ./src/index.ts
ow workers deploy my-api ./src/index.ts -m "Fix bug"

# Delete a worker
ow workers delete my-api
ow workers rm my-api
```

Supported file types: `.js`, `.ts`, `.wasm`

### Environments

Manage environment variables and secrets.

```bash
# List environments
ow env list

# Get environment details
ow env get production

# Create an environment
ow env create production -d "Production environment"

# Set a variable
ow env set production API_URL "https://api.example.com"

# Set a secret (value will be masked)
ow env set production API_KEY "secret-key" --secret

# Remove a variable
ow env unset production API_URL

# Delete an environment
ow env delete production
```

### Storage

Manage S3/R2 storage configurations.

```bash
# List storage configs
ow storage list

# Get storage details
ow storage get my-storage

# Create platform storage (shared R2)
ow storage create my-storage

# Create S3 storage
ow storage create my-s3 --provider s3 \
  --bucket my-bucket \
  --access-key-id AKIAXXXXXXXX \
  --secret-access-key xxxxx \
  --endpoint https://s3.amazonaws.com

# Delete storage
ow storage delete my-storage
```

### KV

Manage key-value namespaces.

```bash
# List KV namespaces
ow kv list

# Get KV details
ow kv get my-kv

# Create KV namespace
ow kv create my-kv -d "Cache storage"

# Delete KV namespace
ow kv delete my-kv
```

### Databases

Manage database bindings.

```bash
# List databases
ow databases list

# Get database details
ow databases get my-db

# Create platform database (shared)
ow databases create my-db

# Create Postgres binding
ow databases create my-pg --provider postgres \
  --connection-string "postgres://user:pass@host/db"

# Delete database
ow databases delete my-db
```

### Migrations

Database schema migrations (requires `db` type alias).

```bash
# Check migration status
ow local migrate status

# Run pending migrations
ow local migrate run

# Baseline existing database (mark all migrations as applied)
ow local migrate baseline
```

### Using Aliases

```bash
# Use default alias
ow workers list

# Use specific alias as first argument
ow prod workers list
ow dev workers get my-api
ow local migrate run

# Or use --alias flag
ow --alias prod workers list
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
    "dev": {
      "type": "api",
      "url": "http://localhost:7000",
      "token": "ow_xxx"
    },
    "local": {
      "type": "db",
      "database_url": "postgres://user:pass@host/db",
      "user": "admin@example.com"
    }
  }
}
```

## Development

```bash
cargo build
cargo run -- workers list
cargo run -- dev workers list
```
