-- Migration: Convert KV values from TEXT to JSONB
-- This enables storing native JSON types (strings, numbers, booleans, objects, arrays)

BEGIN;

-- Convert existing text values to JSONB strings
-- to_jsonb(text) wraps the text as a JSON string: "hello" -> "\"hello\""
ALTER TABLE kv_data
ALTER COLUMN value TYPE jsonb USING to_jsonb(value);

-- Add a comment documenting the change
COMMENT ON COLUMN kv_data.value IS 'JSON value - supports strings, numbers, booleans, objects, arrays';

COMMIT;
