--
-- OpenWorkers Database Schema - Durable Object snapshots and purge queue
--
-- The runner copies the SQLite file of each changed object to object
-- storage, as it does for D1 databases (33_d1_archives), and records each
-- verified copy here. A row is accepted only from the latest runner epoch.
--
-- durable_object_purge queues what the runner must remove when rows go
-- away: the files of a namespace (object_id and key NULL) or one snapshot
-- object (key set). The runner deletes a queue row once the removal
-- succeeded, so a failure is retried.
--

BEGIN;

CREATE TABLE durable_object_archives (
    namespace_id uuid NOT NULL REFERENCES durable_object_namespaces(id) ON DELETE CASCADE,
    object_id character(64) NOT NULL CHECK (object_id ~ '^[0-9a-f]{64}$'),
    seq bigint NOT NULL,
    epoch bigint NOT NULL REFERENCES runner_epochs(epoch),
    key text NOT NULL,
    checksum character(64) NOT NULL,
    size bigint NOT NULL,
    created_at timestamp with time zone NOT NULL DEFAULT now(),

    PRIMARY KEY (namespace_id, object_id, seq)
);

COMMENT ON TABLE durable_object_archives IS 'Verified snapshots of Durable Object storage in object storage; checksum is the SHA-256 of the stored object';

CREATE TABLE durable_object_purge (
    id bigserial PRIMARY KEY,
    namespace_id uuid NOT NULL,
    object_id character(64),
    key text,
    created_at timestamp with time zone NOT NULL DEFAULT now()
);

CREATE FUNCTION durable_object_purge_namespace() RETURNS trigger AS $$
BEGIN
    INSERT INTO durable_object_purge (namespace_id) VALUES (OLD.id);
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE FUNCTION durable_object_purge_snapshot() RETURNS trigger AS $$
BEGIN
    INSERT INTO durable_object_purge (namespace_id, object_id, key)
    VALUES (OLD.namespace_id, OLD.object_id, OLD.key);
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER durable_object_purge_namespace
    AFTER DELETE ON durable_object_namespaces
    FOR EACH ROW EXECUTE FUNCTION durable_object_purge_namespace();

CREATE TRIGGER durable_object_purge_snapshot
    AFTER DELETE ON durable_object_archives
    FOR EACH ROW EXECUTE FUNCTION durable_object_purge_snapshot();

COMMIT;
