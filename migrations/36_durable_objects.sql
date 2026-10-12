--
-- OpenWorkers Database Schema - Durable Object namespaces
--
-- A namespace is the set of Durable Objects of one class that a worker
-- exports. The runner keeps the storage of each object in a SQLite file,
-- D1_DATA_DIR/objects/<namespace id>/<object id>.sqlite. The ID of a
-- namespace never changes and is never reused: a deleted namespace keeps
-- its row, with deleted_at set, so a binding cannot reach new objects
-- through an old ID.
--
-- key makes and checks the object IDs of the namespace (HMAC-SHA-256); it
-- never leaves the runner. A worker that goes away leaves worker_id NULL:
-- the objects stop, and their storage stays until the namespace is deleted.
--

BEGIN;

-- ============================================================================
-- TABLE: durable_object_namespaces
-- ============================================================================

CREATE TABLE durable_object_namespaces (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id uuid NOT NULL REFERENCES users(id) ON UPDATE CASCADE ON DELETE CASCADE,
    worker_id uuid REFERENCES workers(id) ON UPDATE CASCADE ON DELETE SET NULL,
    class_name character varying(100) NOT NULL
        CHECK (class_name ~ '^[A-Za-z_$][A-Za-z0-9_$]*$'),
    name character varying(100) NOT NULL,
    "desc" character varying(255),
    key bytea NOT NULL
        DEFAULT decode(replace(gen_random_uuid()::text || gen_random_uuid()::text, '-', ''), 'hex')
        CHECK (length(key) = 32),
    created_at timestamp with time zone NOT NULL DEFAULT now(),
    updated_at timestamp with time zone NOT NULL DEFAULT now(),
    deleted_at timestamp with time zone
);

-- A name is unique among the namespaces of a user that are not deleted
CREATE UNIQUE INDEX idx_durable_object_namespaces_user_name
    ON durable_object_namespaces(user_id, name)
    WHERE deleted_at IS NULL;

-- One namespace per class of a worker
CREATE UNIQUE INDEX idx_durable_object_namespaces_worker_class
    ON durable_object_namespaces(worker_id, class_name)
    WHERE deleted_at IS NULL;

CREATE TRIGGER update_durable_object_namespaces_updated_at_trigger
    BEFORE UPDATE ON durable_object_namespaces
    FOR EACH ROW
    EXECUTE FUNCTION auto_updated_at();

-- The runner resolves Durable Object bindings through this table
CREATE TRIGGER worker_update_notify
    AFTER INSERT OR UPDATE OR DELETE OR TRUNCATE ON durable_object_namespaces
    FOR EACH STATEMENT EXECUTE FUNCTION worker_update();

COMMENT ON COLUMN durable_object_namespaces.key IS 'Key of the object IDs of the namespace; read by the runner only';

-- ============================================================================
-- UPDATE CONSTRAINT: Add 'durable_object' to environment_values check
-- ============================================================================

ALTER TABLE environment_values DROP CONSTRAINT check_binding_value;

ALTER TABLE environment_values
ADD CONSTRAINT check_binding_value CHECK (
    (type IN ('var', 'secret') AND value IS NOT NULL) OR
    (type IN ('assets', 'storage', 'kv', 'database', 'worker', 'd1', 'durable_object') AND value ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$')
);

COMMENT ON TYPE enum_binding_type IS 'Binding types: var, secret, assets, storage, kv, database, worker, d1, durable_object';

-- ============================================================================
-- Durable Objects are off by default
-- ============================================================================

ALTER TABLE users ADD COLUMN limit_durable_objects integer NOT NULL DEFAULT 0;

COMMENT ON COLUMN users.limit_durable_objects IS 'Max number of Durable Object namespaces this user can create; 0 turns Durable Objects off';

COMMIT;
