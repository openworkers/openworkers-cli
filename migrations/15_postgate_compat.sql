--
-- OpenWorkers Database Schema - Postgate Compatibility
--
-- This migration:
--   1. Adds database_tokens table (for Postgate token auth)
--   2. Creates system user (UUID zero) for internal resources
--   3. Creates views so Postgate can work with OpenWorkers schema
--

-- ============================================================================
-- TABLE: database_tokens
-- ============================================================================
-- Equivalent to postgate_tokens, but linked to OpenWorkers database_configs

CREATE TABLE database_tokens (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    database_id uuid NOT NULL REFERENCES database_configs(id) ON UPDATE CASCADE ON DELETE CASCADE,
    name character varying(100) NOT NULL DEFAULT 'default',

    -- Token authentication
    token_hash character varying(64) NOT NULL,
    token_prefix character varying(8) NOT NULL,

    -- Permissions: SELECT, INSERT, UPDATE, DELETE, CREATE, ALTER, DROP
    allowed_operations text[] NOT NULL DEFAULT ARRAY['SELECT', 'INSERT', 'UPDATE', 'DELETE'],

    created_at timestamp with time zone NOT NULL DEFAULT now(),
    last_used_at timestamp with time zone,

    UNIQUE (database_id, name)
);

-- ============================================================================
-- INDEXES
-- ============================================================================

CREATE INDEX idx_database_tokens_database_id ON database_tokens(database_id);
CREATE INDEX idx_database_tokens_hash ON database_tokens(token_hash);

-- ============================================================================
-- SYSTEM USER (UUID zero)
-- ============================================================================
-- Used for internal/system resources that don't belong to a specific user

INSERT INTO users (id, username, limit_workers, limit_environments, limit_databases, limit_kv, limit_storage)
VALUES (
    '00000000-0000-0000-0000-000000000000',
    '__system__',
    0, 0, 1000, 0, 0  -- Only databases, for Postgate compat
)
ON CONFLICT (id) DO NOTHING;

-- ============================================================================
-- DATABASE CONFIG: API access to OpenWorkers database
-- ============================================================================
-- The dashboard API uses Postgate HTTP to access the OpenWorkers database.
-- Uses 'platform' provider with 'public' schema to access main tables.
-- After running migrations, generate a token with:
--   postgate gen-token aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa api

INSERT INTO database_configs (id, user_id, name, provider, schema_name, max_rows)
VALUES (
    'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa',
    '00000000-0000-0000-0000-000000000000',
    'openworkers-api',
    'platform',
    'public',
    10000
)
ON CONFLICT (id) DO NOTHING;

-- ============================================================================
-- VIEW: postgate_databases
-- ============================================================================
-- Maps OpenWorkers database_configs to Postgate's expected schema

CREATE OR REPLACE VIEW postgate_databases AS
SELECT
    id,
    name,
    CASE provider::text
        WHEN 'platform' THEN 'schema'
        WHEN 'postgres' THEN 'dedicated'
        ELSE 'schema'
    END::varchar(20) AS backend_type,
    schema_name,
    connection_string,
    max_rows,
    created_at
FROM database_configs;

-- ============================================================================
-- VIEW: postgate_tokens
-- ============================================================================
-- Maps OpenWorkers database_tokens to Postgate's expected schema

CREATE OR REPLACE VIEW postgate_tokens AS
SELECT
    id,
    database_id,
    name,
    token_hash,
    token_prefix,
    allowed_operations,
    created_at,
    last_used_at
FROM database_tokens;

-- ============================================================================
-- FUNCTION: create_tenant_database (for Postgate CLI compatibility)
-- ============================================================================
-- Postgate's create-db command calls this function

CREATE OR REPLACE FUNCTION create_tenant_database(p_name text, p_max_rows integer)
RETURNS TABLE(id uuid, schema_name text)
LANGUAGE plpgsql
SECURITY DEFINER
AS $$
DECLARE
    v_schema_name text;
    v_id uuid;
BEGIN
    -- Generate unique schema name
    v_schema_name := 'tenant_' || replace(gen_random_uuid()::text, '-', '');

    -- Create the schema
    EXECUTE format('CREATE SCHEMA IF NOT EXISTS %I', v_schema_name);

    -- Insert into database_configs (owned by system user)
    INSERT INTO database_configs (user_id, name, provider, schema_name, max_rows)
    VALUES (
        '00000000-0000-0000-0000-000000000000',
        p_name,
        'platform',
        v_schema_name,
        p_max_rows
    )
    RETURNING database_configs.id INTO v_id;

    RETURN QUERY SELECT v_id, v_schema_name;
END;
$$;

-- ============================================================================
-- COMMENTS
-- ============================================================================

-- ============================================================================
-- TRIGGER: Make postgate_tokens view updatable (for last_used_at updates)
-- ============================================================================

CREATE OR REPLACE FUNCTION postgate_tokens_update()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    UPDATE database_tokens
    SET last_used_at = NEW.last_used_at
    WHERE id = OLD.id;
    RETURN NEW;
END;
$$;

CREATE TRIGGER postgate_tokens_update_trigger
    INSTEAD OF UPDATE ON postgate_tokens
    FOR EACH ROW
    EXECUTE FUNCTION postgate_tokens_update();

-- ============================================================================
-- TRIGGER: Make postgate_tokens view insertable (for token creation)
-- ============================================================================

CREATE OR REPLACE FUNCTION postgate_tokens_insert()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    v_id uuid;
BEGIN
    INSERT INTO database_tokens (id, database_id, name, token_hash, token_prefix, allowed_operations, created_at, last_used_at)
    VALUES (
        COALESCE(NEW.id, gen_random_uuid()),
        NEW.database_id,
        NEW.name,
        NEW.token_hash,
        NEW.token_prefix,
        NEW.allowed_operations,
        COALESCE(NEW.created_at, now()),
        NEW.last_used_at
    )
    ON CONFLICT (database_id, name) DO UPDATE SET
        token_hash = EXCLUDED.token_hash,
        token_prefix = EXCLUDED.token_prefix,
        allowed_operations = EXCLUDED.allowed_operations,
        created_at = NOW()
    RETURNING id INTO v_id;

    NEW.id := v_id;
    RETURN NEW;
END;
$$;

CREATE TRIGGER postgate_tokens_insert_trigger
    INSTEAD OF INSERT ON postgate_tokens
    FOR EACH ROW
    EXECUTE FUNCTION postgate_tokens_insert();

-- ============================================================================
-- TRIGGER: Make postgate_tokens view deletable
-- ============================================================================

CREATE OR REPLACE FUNCTION postgate_tokens_delete()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    DELETE FROM database_tokens WHERE id = OLD.id;
    RETURN OLD;
END;
$$;

CREATE TRIGGER postgate_tokens_delete_trigger
    INSTEAD OF DELETE ON postgate_tokens
    FOR EACH ROW
    EXECUTE FUNCTION postgate_tokens_delete();

-- ============================================================================
-- COMMENTS
-- ============================================================================

COMMENT ON TABLE database_tokens IS 'Authentication tokens for database access via Postgate';
COMMENT ON VIEW postgate_databases IS 'Compatibility view for Postgate - maps to database_configs';
COMMENT ON VIEW postgate_tokens IS 'Compatibility view for Postgate - maps to database_tokens';
COMMENT ON FUNCTION create_tenant_database IS 'Creates a tenant database (used by Postgate CLI)';
