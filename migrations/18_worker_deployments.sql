-- Migration: Worker Deployments
-- Adds versioned code storage with support for JavaScript, TypeScript, WebAssembly, and Snapshots

BEGIN;

-- Create new code_type enum (replaces enum_workers_language)
CREATE TYPE enum_code_type AS ENUM (
    'javascript',
    'typescript',
    'wasm',
    'snapshot'
);

-- Add current_version to workers
ALTER TABLE workers ADD COLUMN current_version INT;

-- Create worker_deployments table
CREATE TABLE worker_deployments (
    worker_id UUID NOT NULL REFERENCES workers(id) ON UPDATE CASCADE ON DELETE CASCADE,
    version INT NOT NULL,
    hash VARCHAR(64) NOT NULL,
    code_type enum_code_type NOT NULL,
    code BYTEA NOT NULL,
    deployed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deployed_by UUID REFERENCES users(id) ON UPDATE CASCADE ON DELETE SET NULL,
    message TEXT,
    PRIMARY KEY (worker_id, version)
);

-- Create index for fast lookups by hash
CREATE INDEX idx_worker_deployments_hash ON worker_deployments(worker_id, hash);

-- Migrate existing worker scripts to worker_deployments
INSERT INTO worker_deployments (worker_id, version, hash, code_type, code, deployed_at, deployed_by)
SELECT
    w.id,
    1,
    encode(sha256(convert_to(w.script, 'UTF8')), 'hex'),
    w.language::text::enum_code_type,
    convert_to(w.script, 'UTF8'),
    w.updated_at,
    w.user_id
FROM workers w
WHERE w.script IS NOT NULL AND w.script != '';

-- Set current_version for migrated workers
UPDATE workers SET current_version = 1
WHERE script IS NOT NULL AND script != '';

-- Drop old columns (script and language)
ALTER TABLE workers DROP COLUMN script;
ALTER TABLE workers DROP COLUMN language;

-- Drop old enum type
DROP TYPE enum_workers_language;

COMMIT;
