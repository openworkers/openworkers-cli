--
-- OpenWorkers Database Schema - Worker Bindings
--
-- Adds worker binding support for worker-to-worker communication
-- The binding value references workers.id directly (no config table needed)
--

BEGIN;

-- ============================================================================
-- UPDATE CONSTRAINT: Add 'worker' to environment_values check
-- ============================================================================

-- Drop old constraint
ALTER TABLE environment_values DROP CONSTRAINT check_binding_value;

-- Add new constraint including 'worker'
ALTER TABLE environment_values
ADD CONSTRAINT check_binding_value CHECK (
    (type IN ('var', 'secret') AND value IS NOT NULL) OR
    (type IN ('assets', 'storage', 'kv', 'database', 'worker') AND value ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$')
);

-- ============================================================================
-- COMMENTS
-- ============================================================================

COMMENT ON TYPE enum_binding_type IS 'Binding types: var, secret, assets, storage, kv, database, worker';

COMMIT;
