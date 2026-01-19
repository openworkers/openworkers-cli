--
-- Add token_id column to databases table
-- This stores the postgate token ID (not the secret) for reference
--

BEGIN;

ALTER TABLE databases ADD COLUMN token_id uuid;

-- Create index for token lookup
CREATE INDEX idx_databases_token_id ON databases(token_id);

COMMIT;
