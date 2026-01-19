--
-- OpenWorkers Database Schema - Database Configs (Multi-Provider)
--
-- Adds database binding support with multi-provider architecture:
--   - platform: schema-based multi-tenant on shared PostgreSQL pool
--   - postgres: direct connection (can be Supabase, PlanetScale, Neon, etc.)
--

-- ============================================================================
-- TABLE: database_configs
-- ============================================================================

CREATE TABLE database_configs (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id uuid NOT NULL REFERENCES users(id) ON UPDATE CASCADE ON DELETE CASCADE,
    name character varying(100) NOT NULL,
    "desc" character varying(255),

    -- Provider: 'platform' (shared multi-tenant) | 'postgres' (direct connection)
    provider character varying(50) NOT NULL DEFAULT 'platform',

    -- Connection string (for postgres provider)
    connection_string text,

    -- Schema name (for platform provider: multi-tenant on shared pool)
    schema_name character varying(100),

    -- Query limits
    max_rows integer NOT NULL DEFAULT 1000,
    timeout_seconds integer NOT NULL DEFAULT 30,

    created_at timestamp with time zone NOT NULL DEFAULT now(),
    updated_at timestamp with time zone NOT NULL DEFAULT now(),

    UNIQUE (user_id, name),

    -- Validation: platform needs schema_name, postgres needs connection_string
    CONSTRAINT check_database_config CHECK (
        (provider = 'platform' AND schema_name IS NOT NULL)
        OR
        (provider = 'postgres' AND connection_string IS NOT NULL)
    )
);

-- ============================================================================
-- INDEXES
-- ============================================================================

CREATE INDEX idx_database_configs_user_id ON database_configs(user_id);
CREATE UNIQUE INDEX idx_database_configs_schema_name ON database_configs(schema_name)
    WHERE schema_name IS NOT NULL;

-- ============================================================================
-- TRIGGERS
-- ============================================================================

CREATE TRIGGER update_database_configs_updated_at_trigger
    BEFORE UPDATE ON database_configs
    FOR EACH ROW
    EXECUTE FUNCTION auto_updated_at();

-- ============================================================================
-- UPDATE CONSTRAINT: Add 'database' to environment_values check
-- ============================================================================

-- Drop old constraint
ALTER TABLE environment_values DROP CONSTRAINT check_binding_value;

-- Add new constraint including 'database'
ALTER TABLE environment_values
ADD CONSTRAINT check_binding_value CHECK (
    (type IN ('var', 'secret') AND value IS NOT NULL) OR
    (type IN ('assets', 'storage', 'kv', 'database') AND value ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$')
);

-- ============================================================================
-- CLEANUP: Drop old databases table (was referencing external postgate)
-- ============================================================================

DROP TABLE IF EXISTS databases;

-- ============================================================================
-- FUNCTIONS: Schema management for platform provider
-- ============================================================================

-- Create a tenant schema (called from API via postgate)
CREATE OR REPLACE FUNCTION create_tenant_schema(p_schema_name text)
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
AS $$
BEGIN
    EXECUTE format('CREATE SCHEMA IF NOT EXISTS %I', p_schema_name);
END;
$$;

-- Drop a tenant schema (called from API via postgate)
CREATE OR REPLACE FUNCTION drop_tenant_schema(p_schema_name text)
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
AS $$
BEGIN
    EXECUTE format('DROP SCHEMA IF EXISTS %I CASCADE', p_schema_name);
END;
$$;

-- List tables in a tenant schema (with ownership check)
CREATE OR REPLACE FUNCTION list_tenant_tables(p_user_id uuid, p_schema_name text)
RETURNS TABLE(table_name text, row_count bigint)
LANGUAGE plpgsql
SECURITY DEFINER
AS $$
DECLARE
    tbl record;
    cnt bigint;
BEGIN
    -- Verify ownership
    IF NOT EXISTS (
        SELECT 1 FROM database_configs
        WHERE user_id = p_user_id AND schema_name = p_schema_name
    ) THEN
        RAISE EXCEPTION 'Access denied';
    END IF;

    FOR tbl IN SELECT tablename FROM pg_tables WHERE schemaname = p_schema_name
    LOOP
        EXECUTE format('SELECT count(*) FROM %I.%I', p_schema_name, tbl.tablename) INTO cnt;
        table_name := tbl.tablename;
        row_count := cnt;
        RETURN NEXT;
    END LOOP;
END;
$$;

-- Describe columns of a tenant table (with ownership check)
CREATE OR REPLACE FUNCTION describe_tenant_table(p_user_id uuid, p_schema_name text, p_table_name text)
RETURNS TABLE(
    column_name text,
    data_type text,
    is_nullable boolean,
    column_default text,
    is_primary_key boolean
)
LANGUAGE plpgsql
SECURITY DEFINER
AS $$
BEGIN
    -- Verify ownership
    IF NOT EXISTS (
        SELECT 1 FROM database_configs
        WHERE user_id = p_user_id AND schema_name = p_schema_name
    ) THEN
        RAISE EXCEPTION 'Access denied';
    END IF;

    RETURN QUERY
    SELECT
        c.column_name::text,
        c.data_type::text,
        (c.is_nullable = 'YES')::boolean,
        c.column_default::text,
        COALESCE(
            (SELECT true FROM information_schema.table_constraints tc
             JOIN information_schema.key_column_usage kcu
                ON tc.constraint_name = kcu.constraint_name
                AND tc.table_schema = kcu.table_schema
             WHERE tc.constraint_type = 'PRIMARY KEY'
                AND tc.table_schema = p_schema_name
                AND tc.table_name = p_table_name
                AND kcu.column_name = c.column_name
             LIMIT 1),
            false
        )::boolean
    FROM information_schema.columns c
    WHERE c.table_schema = p_schema_name
        AND c.table_name = p_table_name
    ORDER BY c.ordinal_position;
END;
$$;

-- Create a table in a tenant schema (with ownership check)
-- columns_json format: [{"name": "id", "type": "SERIAL", "primaryKey": true}, {"name": "email", "type": "TEXT", "notNull": true}]
CREATE OR REPLACE FUNCTION create_tenant_table(
    p_user_id uuid,
    p_schema_name text,
    p_table_name text,
    p_columns jsonb
)
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
AS $$
DECLARE
    col jsonb;
    col_defs text[] := '{}';
    col_def text;
    pk_cols text[] := '{}';
BEGIN
    -- Verify ownership
    IF NOT EXISTS (
        SELECT 1 FROM database_configs
        WHERE user_id = p_user_id AND schema_name = p_schema_name
    ) THEN
        RAISE EXCEPTION 'Access denied';
    END IF;

    -- Build column definitions
    FOR col IN SELECT * FROM jsonb_array_elements(p_columns)
    LOOP
        col_def := format('%I %s', col->>'name', col->>'type');

        -- Add NOT NULL if specified
        IF (col->>'notNull')::boolean IS TRUE THEN
            col_def := col_def || ' NOT NULL';
        END IF;

        -- Add UNIQUE if specified
        IF (col->>'unique')::boolean IS TRUE THEN
            col_def := col_def || ' UNIQUE';
        END IF;

        -- Add DEFAULT if specified
        IF col->>'default' IS NOT NULL THEN
            col_def := col_def || ' DEFAULT ' || (col->>'default');
        END IF;

        col_defs := array_append(col_defs, col_def);

        -- Collect primary key columns
        IF (col->>'primaryKey')::boolean IS TRUE THEN
            pk_cols := array_append(pk_cols, format('%I', col->>'name'));
        END IF;
    END LOOP;

    -- Add primary key constraint if any
    IF array_length(pk_cols, 1) > 0 THEN
        col_defs := array_append(col_defs, 'PRIMARY KEY (' || array_to_string(pk_cols, ', ') || ')');
    END IF;

    -- Execute CREATE TABLE
    EXECUTE format(
        'CREATE TABLE %I.%I (%s)',
        p_schema_name,
        p_table_name,
        array_to_string(col_defs, ', ')
    );
END;
$$;

-- Drop a table in a tenant schema (with ownership check)
CREATE OR REPLACE FUNCTION drop_tenant_table(p_user_id uuid, p_schema_name text, p_table_name text)
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
AS $$
BEGIN
    -- Verify ownership
    IF NOT EXISTS (
        SELECT 1 FROM database_configs
        WHERE user_id = p_user_id AND schema_name = p_schema_name
    ) THEN
        RAISE EXCEPTION 'Access denied';
    END IF;

    EXECUTE format('DROP TABLE IF EXISTS %I.%I CASCADE', p_schema_name, p_table_name);
END;
$$;

-- Add a column to a tenant table (with ownership check)
-- p_column_def format: {"name": "email", "type": "TEXT", "notNull": true, "unique": true}
CREATE OR REPLACE FUNCTION add_tenant_column(
    p_user_id uuid,
    p_schema_name text,
    p_table_name text,
    p_column_def jsonb
)
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
AS $$
DECLARE
    col_sql text;
BEGIN
    -- Verify ownership
    IF NOT EXISTS (
        SELECT 1 FROM database_configs
        WHERE user_id = p_user_id AND schema_name = p_schema_name
    ) THEN
        RAISE EXCEPTION 'Access denied';
    END IF;

    -- Build column definition
    col_sql := format('%I %s', p_column_def->>'name', p_column_def->>'type');

    -- Add NOT NULL if specified
    IF (p_column_def->>'notNull')::boolean IS TRUE THEN
        col_sql := col_sql || ' NOT NULL';
    END IF;

    -- Add UNIQUE if specified
    IF (p_column_def->>'unique')::boolean IS TRUE THEN
        col_sql := col_sql || ' UNIQUE';
    END IF;

    -- Add DEFAULT if specified
    IF p_column_def->>'default' IS NOT NULL THEN
        col_sql := col_sql || ' DEFAULT ' || (p_column_def->>'default');
    END IF;

    -- Execute ALTER TABLE ADD COLUMN
    EXECUTE format('ALTER TABLE %I.%I ADD COLUMN %s', p_schema_name, p_table_name, col_sql);
END;
$$;

-- Drop a column from a tenant table (with ownership check)
CREATE OR REPLACE FUNCTION drop_tenant_column(
    p_user_id uuid,
    p_schema_name text,
    p_table_name text,
    p_column_name text
)
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
AS $$
BEGIN
    -- Verify ownership
    IF NOT EXISTS (
        SELECT 1 FROM database_configs
        WHERE user_id = p_user_id AND schema_name = p_schema_name
    ) THEN
        RAISE EXCEPTION 'Access denied';
    END IF;

    EXECUTE format('ALTER TABLE %I.%I DROP COLUMN %I', p_schema_name, p_table_name, p_column_name);
END;
$$;

-- ============================================================================
-- COMMENTS
-- ============================================================================

COMMENT ON TABLE database_configs IS 'Database configs - multi-provider (platform, postgres)';

COMMENT ON COLUMN database_configs.provider IS 'Database provider: platform (shared multi-tenant) or postgres (direct connection)';
COMMENT ON COLUMN database_configs.connection_string IS 'PostgreSQL connection string (for postgres provider)';
COMMENT ON COLUMN database_configs.schema_name IS 'Schema name for isolation on shared pool (for platform provider)';
COMMENT ON COLUMN database_configs.max_rows IS 'Maximum rows returned per query';
COMMENT ON COLUMN database_configs.timeout_seconds IS 'Query timeout in seconds';

COMMENT ON COLUMN environment_values.value IS 'For var/secret: the value. For bindings: UUID of config table (storage_configs, kv_configs, database_configs)';
