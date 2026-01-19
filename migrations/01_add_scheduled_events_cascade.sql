--
-- Migration 01: Add ON DELETE CASCADE to scheduled_events.worker_id
--
-- Previously, deleting a worker would fail if scheduled_events existed.
-- This migration adds CASCADE behavior to automatically clean up scheduled_events
-- when their parent worker is deleted.
--

BEGIN;

-- Drop the existing constraint
ALTER TABLE scheduled_events
    DROP CONSTRAINT IF EXISTS scheduled_events_worker_id_fkey;

-- Re-add the constraint with ON DELETE CASCADE
ALTER TABLE scheduled_events
    ADD CONSTRAINT scheduled_events_worker_id_fkey
    FOREIGN KEY (worker_id)
    REFERENCES workers(id)
    ON UPDATE CASCADE
    ON DELETE CASCADE;

COMMIT;
