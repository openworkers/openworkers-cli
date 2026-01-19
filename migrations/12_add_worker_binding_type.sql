--
-- OpenWorkers Database Schema - Add 'worker' binding type
--
-- Must be separate from 13_worker_bindings because PostgreSQL cannot use
-- a newly added enum value in the same transaction.
--
-- NOTE: No BEGIN/COMMIT - ALTER TYPE ADD VALUE cannot run inside a transaction block.
--

ALTER TYPE enum_binding_type ADD VALUE 'worker';
