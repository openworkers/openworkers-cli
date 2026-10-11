--
-- OpenWorkers Database Schema - D1 databases
--
-- A D1 database is a SQLite file that the runner keeps in D1_DATA_DIR,
-- named by d1_databases.id. The ID never changes and is never reused: a
-- deleted database keeps its row, with deleted_at set, so a binding cannot
-- reach a new database through an old ID.
--
-- d1_requests carries management SQL (ow d1 execute, migrations) to the
-- runner, which owns the files. A client inserts a request; the runner
-- writes the result into the same row. Both sides LISTEN, so neither polls.
--

BEGIN;

-- ============================================================================
-- TABLE: d1_databases
-- ============================================================================

CREATE TABLE d1_databases (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id uuid NOT NULL REFERENCES users(id) ON UPDATE CASCADE ON DELETE CASCADE,
    name character varying(100) NOT NULL,
    "desc" character varying(255),
    created_at timestamp with time zone NOT NULL DEFAULT now(),
    updated_at timestamp with time zone NOT NULL DEFAULT now(),
    deleted_at timestamp with time zone
);

-- A name is unique among the databases of a user that are not deleted
CREATE UNIQUE INDEX idx_d1_databases_user_name ON d1_databases(user_id, name)
    WHERE deleted_at IS NULL;

CREATE TRIGGER update_d1_databases_updated_at_trigger
    BEFORE UPDATE ON d1_databases
    FOR EACH ROW
    EXECUTE FUNCTION auto_updated_at();

-- The runner resolves D1 bindings through this table
CREATE TRIGGER worker_update_notify
    AFTER INSERT OR UPDATE OR DELETE OR TRUNCATE ON d1_databases
    FOR EACH STATEMENT EXECUTE FUNCTION worker_update();

-- ============================================================================
-- TABLE: d1_requests
-- ============================================================================

CREATE TABLE d1_requests (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    database_id uuid NOT NULL REFERENCES d1_databases(id) ON DELETE CASCADE,
    request jsonb NOT NULL,
    result jsonb,
    created_at timestamp with time zone NOT NULL DEFAULT now(),
    finished_at timestamp with time zone
);

CREATE INDEX idx_d1_requests_pending ON d1_requests(created_at)
    WHERE finished_at IS NULL;

CREATE FUNCTION d1_request_notify() RETURNS trigger AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        PERFORM pg_notify('d1_request', NEW.id::text);
    ELSIF NEW.finished_at IS NOT NULL AND OLD.finished_at IS NULL THEN
        PERFORM pg_notify('d1_result', NEW.id::text);
    END IF;

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER d1_request_notify
    AFTER INSERT OR UPDATE ON d1_requests
    FOR EACH ROW EXECUTE FUNCTION d1_request_notify();

-- ============================================================================
-- UPDATE CONSTRAINT: Add 'd1' to environment_values check
-- ============================================================================

ALTER TABLE environment_values DROP CONSTRAINT check_binding_value;

ALTER TABLE environment_values
ADD CONSTRAINT check_binding_value CHECK (
    (type IN ('var', 'secret') AND value IS NOT NULL) OR
    (type IN ('assets', 'storage', 'kv', 'database', 'worker', 'd1') AND value ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$')
);

COMMENT ON TYPE enum_binding_type IS 'Binding types: var, secret, assets, storage, kv, database, worker, d1';

COMMIT;
