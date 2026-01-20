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

### Database Operations

Requires a `db` type alias.

```bash
# Run pending migrations
ow infra db migrate

# Check migration status
ow infra db status

# Baseline existing database (mark all migrations as applied)
ow infra db baseline
```

### Using Aliases

```bash
# Use default alias
ow workers list

# Use specific alias as first argument
ow prod workers list
ow dev workers get my-api
ow infra db migrate

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
    "infra": {
      "type": "db",
      "database_url": "postgres://user:pass@host/db"
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
