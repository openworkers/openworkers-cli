--
-- OpenWorkers Database Schema - Resource Limits as Dedicated Columns
--
-- Converts resource_limits jsonb to dedicated columns for better type safety
-- and adds new limits for kv and storage config bindings.
--

-- ============================================================================
-- ADD NEW COLUMNS
-- ============================================================================

ALTER TABLE users ADD COLUMN limit_workers integer NOT NULL DEFAULT 5;
ALTER TABLE users ADD COLUMN limit_environments integer NOT NULL DEFAULT 5;
ALTER TABLE users ADD COLUMN limit_databases integer NOT NULL DEFAULT 3;
ALTER TABLE users ADD COLUMN limit_kv integer NOT NULL DEFAULT 3;
ALTER TABLE users ADD COLUMN limit_storage integer NOT NULL DEFAULT 3;
ALTER TABLE users ADD COLUMN second_precision boolean NOT NULL DEFAULT false;

-- ============================================================================
-- MIGRATE DATA FROM JSONB
-- ============================================================================

UPDATE users SET
    limit_workers = COALESCE((resource_limits->>'workers')::integer, 5),
    limit_environments = COALESCE((resource_limits->>'environments')::integer, 5),
    limit_databases = COALESCE((resource_limits->>'databases')::integer, 3),
    second_precision = COALESCE((resource_limits->>'secondPrecision')::boolean, false);

-- ============================================================================
-- DROP JSONB COLUMN
-- ============================================================================

ALTER TABLE users DROP COLUMN resource_limits;

-- ============================================================================
-- COMMENTS
-- ============================================================================

COMMENT ON COLUMN users.limit_workers IS 'Max number of workers this user can create';
COMMENT ON COLUMN users.limit_environments IS 'Max number of environments this user can create';
COMMENT ON COLUMN users.limit_databases IS 'Max number of database bindings this user can create';
COMMENT ON COLUMN users.limit_kv IS 'Max number of KV namespaces this user can create';
COMMENT ON COLUMN users.limit_storage IS 'Max number of storage configs this user can create (used for both assets and storage bindings)';
COMMENT ON COLUMN users.second_precision IS 'Whether user can use second-precision cron expressions';
