-- Normalize platform database UUID from aaaa... to nil UUID (00000000-...)
-- The aaaa UUID was arbitrary and fails strict RFC 4122 validation.
-- database_tokens.database_id has ON UPDATE CASCADE so it propagates automatically.

-- 1. Update environment_values that reference this DB (stored as text, no FK)
UPDATE environment_values
SET value = '00000000-0000-0000-0000-000000000000'
WHERE value = 'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa'
  AND type = 'database';

-- 2. Update the database config itself (cascades to database_tokens)
UPDATE database_configs
SET id = '00000000-0000-0000-0000-000000000000'
WHERE id = 'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa';

