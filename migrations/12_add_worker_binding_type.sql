--
-- OpenWorkers Database Schema - Add 'worker' binding type
--
-- Must be separate from 11b because PostgreSQL cannot use
-- a newly added enum value in the same transaction.
--

ALTER TYPE enum_binding_type ADD VALUE 'worker';
