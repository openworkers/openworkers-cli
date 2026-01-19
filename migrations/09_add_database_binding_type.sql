--
-- OpenWorkers Database Schema - Add 'database' binding type
--
-- Must be separate from 09b because PostgreSQL cannot use
-- a newly added enum value in the same transaction.
--

ALTER TYPE enum_binding_type ADD VALUE 'database';
