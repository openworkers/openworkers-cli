--
-- Migration 02: Auto timestamps and UUIDs
--
-- This migration adds:
-- 1. Auto-generated UUIDs for all id columns (gen_random_uuid())
-- 2. Auto-generated created_at timestamps (CURRENT_TIMESTAMP)
-- 3. Auto-updated updated_at timestamps via trigger function
--

BEGIN;

-- ============================================================================
-- FUNCTION: Auto update updated_at
-- ============================================================================

CREATE OR REPLACE FUNCTION auto_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- ALTER TABLES: Add defaults for id and created_at
-- ============================================================================

-- Users
ALTER TABLE users
    ALTER COLUMN id SET DEFAULT gen_random_uuid(),
    ALTER COLUMN created_at SET DEFAULT CURRENT_TIMESTAMP,
    ALTER COLUMN updated_at SET DEFAULT CURRENT_TIMESTAMP;

-- External Users
ALTER TABLE external_users
    ALTER COLUMN created_at SET DEFAULT CURRENT_TIMESTAMP,
    ALTER COLUMN updated_at SET DEFAULT CURRENT_TIMESTAMP;

-- Environments
ALTER TABLE environments
    ALTER COLUMN id SET DEFAULT gen_random_uuid(),
    ALTER COLUMN created_at SET DEFAULT CURRENT_TIMESTAMP,
    ALTER COLUMN updated_at SET DEFAULT CURRENT_TIMESTAMP;

-- Environment Values
ALTER TABLE environment_values
    ALTER COLUMN id SET DEFAULT gen_random_uuid(),
    ALTER COLUMN created_at SET DEFAULT CURRENT_TIMESTAMP,
    ALTER COLUMN updated_at SET DEFAULT CURRENT_TIMESTAMP;

-- Workers
ALTER TABLE workers
    ALTER COLUMN id SET DEFAULT gen_random_uuid(),
    ALTER COLUMN created_at SET DEFAULT CURRENT_TIMESTAMP,
    ALTER COLUMN updated_at SET DEFAULT CURRENT_TIMESTAMP;

-- Domains
ALTER TABLE domains
    ALTER COLUMN created_at SET DEFAULT CURRENT_TIMESTAMP,
    ALTER COLUMN updated_at SET DEFAULT CURRENT_TIMESTAMP;

-- Crons
ALTER TABLE crons
    ALTER COLUMN id SET DEFAULT gen_random_uuid(),
    ALTER COLUMN created_at SET DEFAULT CURRENT_TIMESTAMP,
    ALTER COLUMN updated_at SET DEFAULT CURRENT_TIMESTAMP;

-- Scheduled Events
ALTER TABLE scheduled_events
    ALTER COLUMN id SET DEFAULT gen_random_uuid();

-- ============================================================================
-- TRIGGERS: Auto-update updated_at on modification
-- ============================================================================

CREATE TRIGGER update_users_updated_at_trigger
    BEFORE UPDATE ON users
    FOR EACH ROW
    EXECUTE FUNCTION auto_updated_at();

CREATE TRIGGER update_external_users_updated_at_trigger
    BEFORE UPDATE ON external_users
    FOR EACH ROW
    EXECUTE FUNCTION auto_updated_at();

CREATE TRIGGER update_environments_updated_at_trigger
    BEFORE UPDATE ON environments
    FOR EACH ROW
    EXECUTE FUNCTION auto_updated_at();

CREATE TRIGGER update_environment_values_updated_at_trigger
    BEFORE UPDATE ON environment_values
    FOR EACH ROW
    EXECUTE FUNCTION auto_updated_at();

CREATE TRIGGER update_workers_updated_at_trigger
    BEFORE UPDATE ON workers
    FOR EACH ROW
    EXECUTE FUNCTION auto_updated_at();

CREATE TRIGGER update_domains_updated_at_trigger
    BEFORE UPDATE ON domains
    FOR EACH ROW
    EXECUTE FUNCTION auto_updated_at();

CREATE TRIGGER update_crons_updated_at_trigger
    BEFORE UPDATE ON crons
    FOR EACH ROW
    EXECUTE FUNCTION auto_updated_at();

COMMIT;
