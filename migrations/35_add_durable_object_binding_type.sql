--
-- OpenWorkers Database Schema - Add 'durable_object' binding type
--
-- Separate from 36_durable_objects: PostgreSQL cannot use a new enum value
-- in the transaction that adds it.
--
-- NOTE: No BEGIN/COMMIT - ALTER TYPE ADD VALUE cannot run inside a transaction block.
--

ALTER TYPE enum_binding_type ADD VALUE 'durable_object';
