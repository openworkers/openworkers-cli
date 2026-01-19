--
-- OpenWorkers Database Schema - Add databases to resource_limits
--

BEGIN;

-- Update default for resource_limits column
ALTER TABLE users
ALTER COLUMN resource_limits SET DEFAULT '{"workers": 5, "environments": 5, "databases": 3}'::jsonb;

-- Add databases limit to existing users who don't have it
UPDATE users
SET resource_limits = resource_limits || '{"databases": 3}'::jsonb
WHERE NOT resource_limits ? 'databases';

COMMIT;
