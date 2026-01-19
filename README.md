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
ow alias set cloud --api https://dash.openworkers.com/api/v1 --token <token>
ow alias set local --api http://localhost:3000 --token dev

# Add DB alias
ow alias set infra --db postgres://user:pass@host/db

# List aliases
ow alias list

# Set default
ow alias set-default cloud

# Remove alias
ow alias rm old-alias
```

### Default Alias

On first run, a `cloud` alias pointing to `https://dash.openworkers.com/api/v1` is created as default.

## Commands

### Database Operations

Requires a `db` type alias.

```bash
# Run pending migrations
ow infra db migrate

# Check migration status
ow infra db status

# Baseline existing database (mark all migrations as applied without running them)
ow infra db baseline
```

### Workers

Works with both `api` and `db` aliases.

```bash
# List workers
ow workers list
ow workers ls

# Create a worker
ow workers create my-api -d "My API worker"

# Get worker details
ow workers get my-api

# Deploy code to a worker
ow workers deploy my-api ./src/index.ts -m "Initial deploy"
ow workers deploy my-api ./src/index.ts -m "Fix bug"  # v2, v3, ...

# Delete a worker
ow workers delete my-api
ow workers rm my-api
```

Supported file types: `.js`, `.ts`, `.wasm`

### Using Aliases

```bash
# Use default alias
ow workers list

# Use specific alias (mc-style, alias as first argument)
ow prod workers list
ow infra db migrate
```

## Config File Format

```json
{
  "version": 1,
  "default": "cloud",
  "aliases": {
    "cloud": {
      "type": "api",
      "url": "https://dash.openworkers.com/api/v1",
      "token": "owk_xxx"
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
# Build
cargo build

# Run
cargo run -- alias list
cargo run -- --alias infra db status
```
