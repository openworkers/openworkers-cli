--
-- OpenWorkers Database Schema - Databases table
-- Links openworkers users to postgate databases
--

BEGIN;

-- Databases (references to postgate)
CREATE TABLE databases (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    name character varying(100) NOT NULL,
    "desc" character varying(255),
    user_id uuid NOT NULL REFERENCES users(id) ON UPDATE CASCADE ON DELETE CASCADE,
    postgate_id uuid NOT NULL,  -- Reference to postgate database
    schema_name character varying(100),  -- Cached from postgate
    created_at timestamp with time zone DEFAULT NOW() NOT NULL,
    updated_at timestamp with time zone DEFAULT NOW() NOT NULL,
    UNIQUE (user_id, name)
);

CREATE INDEX idx_databases_user_id ON databases(user_id);

-- Trigger for updated_at
CREATE TRIGGER update_databases_updated_at_trigger
    BEFORE UPDATE ON databases
    FOR EACH ROW
    EXECUTE FUNCTION auto_updated_at();

COMMIT;
