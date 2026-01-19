--
-- OpenWorkers Database Schema - Binding Types
--
-- Transforms environment_values.secret (bool) into a type enum
-- and adds support for resource bindings (assets, storage, kv)
--
-- For bindings (assets, storage, kv), the `value` column stores the UUID
-- of the corresponding config table. No FK constraint, validated by CHECK.
--

BEGIN;

-- ============================================================================
-- ENUMS
-- ============================================================================

CREATE TYPE enum_binding_type AS ENUM (
    'var',      -- Plain environment variable (visible)
    'secret',   -- Secret environment variable (hidden)
    'assets',   -- Static assets binding (S3/R2) - READ ONLY
    'storage',  -- Object storage binding (R2/S3) - READ/WRITE
    'kv'        -- Key-value store binding
);

-- ============================================================================
-- TABLES: Config tables for each binding type
-- ============================================================================

-- Storage configs (S3/R2 credentials) - used for both assets and storage bindings
-- The binding type determines access: assets = read-only, storage = read/write
-- Modes:
--   - Shared: Uses platform R2 with prefix isolation (prefix NOT NULL, endpoint NULL)
--   - Custom: User provides their own S3/R2 credentials
CREATE TABLE storage_configs (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id uuid NOT NULL REFERENCES users(id) ON UPDATE CASCADE ON DELETE CASCADE,
    name character varying(255) NOT NULL,
    "desc" character varying(255),
    bucket character varying(255) NOT NULL,
    prefix character varying(255),                      -- path prefix for isolation
    access_key_id character varying(255) NOT NULL,      -- R2/S3 Access Key ID
    secret_access_key character varying(255) NOT NULL,  -- R2/S3 Secret Access Key (TODO: encrypt)
    endpoint character varying(255),                    -- NULL = default R2, or custom S3 endpoint
    region character varying(50),                       -- AWS region (not needed for R2)
    public_url character varying(255),                  -- optional CDN URL for public access
    created_at timestamp with time zone NOT NULL DEFAULT now(),
    updated_at timestamp with time zone NOT NULL DEFAULT now(),
    UNIQUE (user_id, name)
);

-- KV config (key-value namespace)
-- The config itself IS the namespace - kv_data references this
CREATE TABLE kv_configs (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id uuid NOT NULL REFERENCES users(id) ON UPDATE CASCADE ON DELETE CASCADE,
    name character varying(255) NOT NULL,
    "desc" character varying(255),
    created_at timestamp with time zone NOT NULL DEFAULT now(),
    updated_at timestamp with time zone NOT NULL DEFAULT now(),
    UNIQUE (user_id, name)
);

-- KV data (actual key-value pairs within a namespace)
CREATE TABLE kv_data (
    namespace_id uuid NOT NULL REFERENCES kv_configs(id) ON UPDATE CASCADE ON DELETE CASCADE,
    key text NOT NULL,
    value text NOT NULL,
    expires_at timestamp with time zone,
    created_at timestamp with time zone NOT NULL DEFAULT now(),
    updated_at timestamp with time zone NOT NULL DEFAULT now(),
    PRIMARY KEY (namespace_id, key)
);

-- ============================================================================
-- MIGRATION: environment_values - add type enum
-- ============================================================================

-- Step 1: Add type column
ALTER TABLE environment_values
ADD COLUMN type enum_binding_type;

-- Step 2: Migrate existing data (secret bool → type enum)
UPDATE environment_values
SET type = CASE
    WHEN secret = true THEN 'secret'::enum_binding_type
    ELSE 'var'::enum_binding_type
END;

-- Step 3: Make type NOT NULL
ALTER TABLE environment_values ALTER COLUMN type SET NOT NULL;

-- Step 4: Drop the old secret column
ALTER TABLE environment_values DROP COLUMN secret;

-- Step 5: Add constraint - validate value format based on type
-- For bindings, value must be a valid UUID (reference to config table)
ALTER TABLE environment_values
ADD CONSTRAINT check_binding_value CHECK (
    (type IN ('var', 'secret') AND value IS NOT NULL) OR
    (type IN ('assets', 'storage', 'kv') AND value ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$')
);

-- ============================================================================
-- INDEXES
-- ============================================================================

CREATE INDEX idx_storage_configs_user_id ON storage_configs(user_id);
CREATE INDEX idx_kv_configs_user_id ON kv_configs(user_id);
CREATE INDEX idx_kv_data_expires_at ON kv_data(expires_at) WHERE expires_at IS NOT NULL;

-- ============================================================================
-- COMMENTS
-- ============================================================================

COMMENT ON TABLE storage_configs IS 'S3/R2 storage config - used for both assets (read-only) and storage (read/write) bindings';
COMMENT ON TABLE kv_configs IS 'KV namespace - a bucket of key-value pairs';
COMMENT ON TABLE kv_data IS 'KV data - actual key-value pairs within a namespace';

COMMENT ON COLUMN storage_configs.prefix IS 'Path prefix for isolation (shared mode)';
COMMENT ON COLUMN storage_configs.access_key_id IS 'R2/S3 Access Key ID';
COMMENT ON COLUMN storage_configs.secret_access_key IS 'R2/S3 Secret Access Key';
COMMENT ON COLUMN storage_configs.endpoint IS 'NULL = default R2, or custom S3 endpoint URL';
COMMENT ON COLUMN storage_configs.public_url IS 'Optional CDN URL for public access';

COMMENT ON COLUMN environment_values.type IS 'Binding type: var, secret, assets, storage, kv';
COMMENT ON COLUMN environment_values.value IS 'For var/secret: the value. For bindings: UUID of config table';

COMMIT;
