-- Notify the runners when a table they resolve requests from changes.
--
-- A runner keeps the worker a request resolves to, and that worker's
-- bindings, in memory. On 'worker_update' it drops all of it. The payload
-- names the table, for logs only. Statement-level triggers send one
-- notification per statement, and Postgres sends one per transaction for
-- equal payloads. endpoints is a view over workers and projects, which
-- have their own triggers.

BEGIN;

CREATE OR REPLACE FUNCTION worker_update() RETURNS trigger AS $$
BEGIN
    PERFORM pg_notify('worker_update', TG_TABLE_NAME);
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DO $$
DECLARE
    t text;
BEGIN
    FOREACH t IN ARRAY ARRAY[
        'workers',
        'worker_deployments',
        'projects',
        'project_routes',
        'domains',
        'environments',
        'environment_values',
        'database_configs',
        'kv_configs',
        'storage_configs'
    ]
    LOOP
        EXECUTE format(
            'CREATE TRIGGER worker_update_notify
                AFTER INSERT OR UPDATE OR DELETE OR TRUNCATE ON %I
                FOR EACH STATEMENT EXECUTE FUNCTION worker_update()',
            t
        );
    END LOOP;
END;
$$;

COMMIT;
