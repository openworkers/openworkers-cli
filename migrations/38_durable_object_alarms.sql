--
-- OpenWorkers Database Schema - Durable Object alarms
--
-- The alarm of an object lives in its SQLite file. This table is the index
-- of due alarms that the runner reads; scheduled_at is never later than the
-- alarm in the file, so a crash between the two writes makes an alarm early,
-- never late or lost. The runner checks the file when the time comes.
--
-- retry_count counts the failed runs of the alarm. delivering is set while
-- the handler runs, and cleared when a runner starts.
--

BEGIN;

CREATE TABLE durable_object_alarms (
    namespace_id uuid NOT NULL REFERENCES durable_object_namespaces(id) ON DELETE CASCADE,
    object_id character(64) NOT NULL CHECK (object_id ~ '^[0-9a-f]{64}$'),
    scheduled_at timestamp with time zone NOT NULL,
    retry_count integer NOT NULL DEFAULT 0,
    delivering boolean NOT NULL DEFAULT false,

    PRIMARY KEY (namespace_id, object_id)
);

CREATE INDEX durable_object_alarms_due ON durable_object_alarms (scheduled_at) WHERE NOT delivering;

COMMENT ON TABLE durable_object_alarms IS 'Due index of Durable Object alarms; the alarm itself is in the SQLite file of the object';

COMMIT;
