--
-- OpenWorkers Database Schema - Database Provider Enum
--
-- Converts database_configs.provider from VARCHAR to ENUM for type safety
--

-- ============================================================================
-- ENUM: Database provider
-- ============================================================================

CREATE TYPE enum_database_provider AS ENUM (
    'platform',  -- Shared multi-tenant PostgreSQL
    'postgres'   -- External PostgreSQL connection (Neon, Supabase, etc.)
);

-- ============================================================================
-- MIGRATION: database_configs.provider VARCHAR → ENUM
-- ============================================================================

-- Step 1: Rename existing column
ALTER TABLE database_configs RENAME COLUMN provider TO provider_old;

-- Step 2: Add new enum column
ALTER TABLE database_configs ADD COLUMN provider enum_database_provider;

-- Step 3: Migrate data
UPDATE database_configs
SET provider = provider_old::enum_database_provider;

-- Step 4: Make NOT NULL and set default
ALTER TABLE database_configs ALTER COLUMN provider SET NOT NULL;
ALTER TABLE database_configs ALTER COLUMN provider SET DEFAULT 'platform'::enum_database_provider;

-- Step 5: Drop old column
ALTER TABLE database_configs DROP COLUMN provider_old;

-- ============================================================================
-- COMMENTS
-- ============================================================================

COMMENT ON COLUMN database_configs.provider IS 'Database provider: platform (shared multi-tenant) or postgres (external connection)';
