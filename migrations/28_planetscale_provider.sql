-- Add 'planetscale' to database provider enum (outside transaction — PG requirement)
ALTER TYPE enum_database_provider ADD VALUE 'planetscale';

BEGIN;

-- Update CHECK constraint to accept planetscale with connection_string
-- (constraint may have been dropped during migration 11's varchar→enum conversion)
ALTER TABLE database_configs DROP CONSTRAINT IF EXISTS check_database_config;
ALTER TABLE database_configs ADD CONSTRAINT check_database_config CHECK (
    (provider = 'platform' AND schema_name IS NOT NULL)
    OR (provider = 'postgres' AND connection_string IS NOT NULL)
    OR (provider = 'planetscale' AND connection_string IS NOT NULL)
);

-- Update postgate_databases view to map planetscale → dedicated
CREATE OR REPLACE VIEW postgate_databases AS
SELECT
    id,
    name,
    CASE provider::text
        WHEN 'platform' THEN 'schema'
        WHEN 'postgres' THEN 'dedicated'
        WHEN 'planetscale' THEN 'dedicated'
        ELSE 'schema'
    END::varchar(20) AS backend_type,
    schema_name,
    connection_string,
    max_rows,
    created_at
FROM database_configs;

-- Table for PlanetScale OAuth tokens (one per user)
CREATE TABLE planetscale_tokens (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    access_token text NOT NULL,
    refresh_token text NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    created_at timestamp with time zone NOT NULL DEFAULT now(),
    updated_at timestamp with time zone NOT NULL DEFAULT now(),
    UNIQUE(user_id)
);

COMMIT;
