--
-- OpenWorkers Database Schema - Add 'database' binding type
--
-- Must be separate from 10_database_configs because PostgreSQL cannot use
-- a newly added enum value in the same transaction.
--
-- NOTE: No BEGIN/COMMIT - ALTER TYPE ADD VALUE cannot run inside a transaction block.
--

ALTER TYPE enum_binding_type ADD VALUE 'database';
