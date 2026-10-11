--
-- OpenWorkers Database Schema - Add 'd1' binding type
--
-- Separate from 31_d1_databases: PostgreSQL cannot use a new enum value in
-- the transaction that adds it.
--
-- NOTE: No BEGIN/COMMIT - ALTER TYPE ADD VALUE cannot run inside a transaction block.
--

ALTER TYPE enum_binding_type ADD VALUE 'd1';
