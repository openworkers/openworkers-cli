--
-- OpenWorkers Database Schema - D1 purge queue
--
-- The files and snapshots of a D1 database live outside PostgreSQL. When a
-- row goes away, by a DELETE or by the cascade of a deleted user, these
-- triggers queue what the runner must remove: the local files of a database
-- (key NULL) or one snapshot object (key set). The runner deletes a queue
-- row once the removal succeeded, so a failure is retried.
--
-- A soft delete (deleted_at) keeps the rows; the runner removes the files
-- and snapshots of such a database itself.
--

BEGIN;

CREATE TABLE d1_purge (
    id bigserial PRIMARY KEY,
    database_id uuid NOT NULL,
    key text,
    created_at timestamp with time zone NOT NULL DEFAULT now()
);

CREATE FUNCTION d1_purge_database() RETURNS trigger AS $$
BEGIN
    INSERT INTO d1_purge (database_id) VALUES (OLD.id);
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE FUNCTION d1_purge_snapshot() RETURNS trigger AS $$
BEGIN
    INSERT INTO d1_purge (database_id, key) VALUES (OLD.database_id, OLD.key);
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER d1_purge_database
    AFTER DELETE ON d1_databases
    FOR EACH ROW EXECUTE FUNCTION d1_purge_database();

CREATE TRIGGER d1_purge_snapshot
    AFTER DELETE ON d1_archives
    FOR EACH ROW EXECUTE FUNCTION d1_purge_snapshot();

COMMIT;
