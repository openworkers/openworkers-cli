--
-- OpenWorkers Database Schema - Replace endpoints table with view
--
-- Replaces the endpoints table with a view that combines workers and projects.
-- Uses triggers to enforce global name uniqueness across workers and projects.
--

BEGIN;

-- ============================================================================
-- Drop existing endpoints table
-- ============================================================================

DROP TABLE IF EXISTS endpoints CASCADE;

-- ============================================================================
-- Add columns to support workers in projects (FIRST, before constraints)
-- ============================================================================

-- Projects can have an environment (shared by all workers in the project)
ALTER TABLE projects ADD COLUMN IF NOT EXISTS environment_id uuid REFERENCES environments(id) ON UPDATE CASCADE ON DELETE SET NULL;

-- Workers can belong to a project (deferrable for circular dependency with projects.id)
ALTER TABLE workers ADD COLUMN IF NOT EXISTS project_id uuid;
DO $$ BEGIN
    ALTER TABLE workers ADD CONSTRAINT workers_project_id_fkey
    FOREIGN KEY (project_id) REFERENCES projects(id)
    ON UPDATE CASCADE ON DELETE CASCADE
    DEFERRABLE INITIALLY DEFERRED;
EXCEPTION
    WHEN duplicate_object THEN NULL;
END $$;

-- Projects.id must reference a worker (the main worker has same ID as project)
DO $$ BEGIN
    ALTER TABLE projects ADD CONSTRAINT projects_id_fkey
    FOREIGN KEY (id) REFERENCES workers(id)
    ON UPDATE CASCADE ON DELETE CASCADE
    DEFERRABLE INITIALLY DEFERRED;
EXCEPTION
    WHEN duplicate_object THEN NULL;
END $$;

-- Make workers.name nullable (functions don't have public names)
ALTER TABLE workers ALTER COLUMN name DROP NOT NULL;

-- Workers must have same environment as their project
-- This is enforced by a trigger (check_worker_project_environment) below

-- ============================================================================
-- Unique constraints for endpoint names
-- ============================================================================

-- Drop existing simple UNIQUE constraint on workers.name (if exists)
ALTER TABLE workers DROP CONSTRAINT IF EXISTS workers_name_key;

-- Workers: name is UNIQUE when NOT NULL (public workers, excludes functions)
DROP INDEX IF EXISTS workers_name_unique_when_standalone;
DROP INDEX IF EXISTS workers_name_unique_when_public;
CREATE UNIQUE INDEX workers_name_unique_when_public
ON workers (name)
WHERE name IS NOT NULL;

-- Projects: name is always UNIQUE
DROP INDEX IF EXISTS projects_name_unique;
CREATE UNIQUE INDEX projects_name_unique
ON projects (name);

-- ============================================================================
-- Function to check endpoint name uniqueness across workers and projects
-- ============================================================================

CREATE OR REPLACE FUNCTION check_endpoint_name_unique()
RETURNS TRIGGER AS $$
BEGIN
  -- For workers table: check name doesn't exist in projects (only if standalone)
  IF TG_TABLE_NAME = 'workers' THEN
    -- Only check if this worker is standalone (will be in endpoints)
    IF NEW.project_id IS NULL THEN
      IF EXISTS (
        SELECT 1 FROM projects
        WHERE name = NEW.name
      ) THEN
        RAISE EXCEPTION 'Endpoint name "%" already exists as a project', NEW.name;
      END IF;
    END IF;
  END IF;

  -- For projects table: check name doesn't exist in standalone workers
  -- Exception: allow same name if project.id = worker.id (main worker case)
  IF TG_TABLE_NAME = 'projects' THEN
    IF EXISTS (
      SELECT 1 FROM workers
      WHERE name = NEW.name AND project_id IS NULL AND id != NEW.id
    ) THEN
      RAISE EXCEPTION 'Endpoint name "%" already exists as a worker', NEW.name;
    END IF;
  END IF;

  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- Triggers to enforce global endpoint name uniqueness
-- ============================================================================

CREATE TRIGGER check_workers_endpoint_name
  BEFORE INSERT OR UPDATE OF name ON workers
  FOR EACH ROW
  EXECUTE FUNCTION check_endpoint_name_unique();

CREATE TRIGGER check_projects_endpoint_name
  BEFORE INSERT OR UPDATE OF name ON projects
  FOR EACH ROW
  EXECUTE FUNCTION check_endpoint_name_unique();

-- ============================================================================
-- Function to ensure workers have same environment as their project
-- ============================================================================

CREATE OR REPLACE FUNCTION check_worker_project_environment()
RETURNS TRIGGER AS $$
DECLARE
    v_project_env_id uuid;
BEGIN
  -- For workers table: if in a project, must have same environment_id as project
  IF TG_TABLE_NAME = 'workers' THEN
    IF NEW.project_id IS NOT NULL THEN
      SELECT environment_id INTO v_project_env_id
      FROM projects
      WHERE id = NEW.project_id;

      IF NOT FOUND THEN
        RAISE EXCEPTION 'Project % not found', NEW.project_id;
      END IF;

      IF NEW.environment_id IS DISTINCT FROM v_project_env_id THEN
        RAISE EXCEPTION 'Worker environment_id must match project environment_id';
      END IF;
    END IF;
  END IF;

  -- For projects table: all workers in project must have same environment_id
  IF TG_TABLE_NAME = 'projects' THEN
    IF EXISTS (
      SELECT 1 FROM workers
      WHERE project_id = NEW.id
        AND environment_id IS DISTINCT FROM NEW.environment_id
    ) THEN
      RAISE EXCEPTION 'All workers in project must have same environment_id as project';
    END IF;
  END IF;

  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER check_workers_project_environment
  BEFORE INSERT OR UPDATE OF project_id, environment_id ON workers
  FOR EACH ROW
  EXECUTE FUNCTION check_worker_project_environment();

CREATE TRIGGER check_projects_environment
  BEFORE INSERT OR UPDATE OF environment_id ON projects
  FOR EACH ROW
  EXECUTE FUNCTION check_worker_project_environment();

-- ============================================================================
-- Function to validate ownership consistency
-- ============================================================================

CREATE OR REPLACE FUNCTION check_ownership()
RETURNS TRIGGER AS $$
BEGIN
  -- Projects: user_id must match environment.user_id
  IF TG_TABLE_NAME = 'projects' THEN
    IF NEW.environment_id IS NOT NULL AND NEW.user_id != (SELECT user_id FROM environments WHERE id = NEW.environment_id) THEN
      RAISE EXCEPTION 'Project and environment must belong to same user';
    END IF;
  END IF;

  -- Workers: user_id must match project.user_id and environment.user_id
  IF TG_TABLE_NAME = 'workers' THEN
    IF NEW.project_id IS NOT NULL AND NEW.user_id != (SELECT user_id FROM projects WHERE id = NEW.project_id) THEN
      RAISE EXCEPTION 'Worker and project must belong to same user';
    END IF;
    IF NEW.environment_id IS NOT NULL AND NEW.user_id != (SELECT user_id FROM environments WHERE id = NEW.environment_id) THEN
      RAISE EXCEPTION 'Worker and environment must belong to same user';
    END IF;
  END IF;

  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER check_projects_ownership
  BEFORE INSERT OR UPDATE OF user_id, environment_id ON projects
  FOR EACH ROW
  EXECUTE FUNCTION check_ownership();

CREATE TRIGGER check_workers_ownership
  BEFORE INSERT OR UPDATE OF user_id, project_id ON workers
  FOR EACH ROW
  EXECUTE FUNCTION check_ownership();

-- ============================================================================
-- Create endpoints view
-- ============================================================================

CREATE VIEW endpoints AS
  -- Workers with public names (standalone or main workers)
  -- Excludes functions (name IS NULL)
  SELECT
    name,
    'worker'::enum_endpoint_type AS type,
    user_id,
    id AS worker_id,
    NULL::uuid AS project_id,
    created_at,
    updated_at
  FROM workers
  WHERE name IS NOT NULL

  UNION ALL

  -- Projects as endpoints
  SELECT
    name,
    'project'::enum_endpoint_type AS type,
    user_id,
    NULL::uuid AS worker_id,
    id AS project_id,
    created_at,
    updated_at
  FROM projects;

-- ============================================================================
-- Update domains table to support projects
-- ============================================================================

-- Add project_id column
ALTER TABLE domains ADD COLUMN project_id uuid REFERENCES projects(id) ON UPDATE CASCADE ON DELETE CASCADE;

-- Add constraint: domain must point to either worker OR project (not both, not neither)
ALTER TABLE domains ADD CONSTRAINT check_domain_target CHECK (
  (worker_id IS NOT NULL AND project_id IS NULL) OR
  (project_id IS NOT NULL AND worker_id IS NULL)
);

-- ============================================================================
-- Update project_routes with proper types and FKs
-- ============================================================================

-- Create enum for backend types
CREATE TYPE enum_backend_type AS ENUM ('worker', 'storage');

-- Change backend_type from VARCHAR to ENUM
ALTER TABLE project_routes
  ALTER COLUMN backend_type TYPE enum_backend_type
  USING backend_type::enum_backend_type;

-- Add worker_id FK column for worker backend
ALTER TABLE project_routes ADD COLUMN worker_id uuid REFERENCES workers(id) ON UPDATE CASCADE ON DELETE CASCADE;

-- Drop deprecated columns (replaced by worker_id for worker backend, ASSETS binding for storage backend)
ALTER TABLE project_routes DROP COLUMN IF EXISTS backend_target;
ALTER TABLE project_routes DROP COLUMN IF EXISTS storage_config_id;

-- Change primary key from id to (project_id, pattern) to avoid conflicts
ALTER TABLE project_routes DROP CONSTRAINT IF EXISTS project_routes_pkey;
ALTER TABLE project_routes DROP CONSTRAINT IF EXISTS project_routes_project_id_pattern_key;
ALTER TABLE project_routes DROP COLUMN IF EXISTS id;
ALTER TABLE project_routes ADD PRIMARY KEY (project_id, pattern);

-- Add constraint: worker backend needs worker_id, storage backend uses ASSETS binding from environment
ALTER TABLE project_routes ADD CONSTRAINT check_backend_target CHECK (
    (backend_type = 'worker' AND worker_id IS NOT NULL) OR
    (backend_type = 'storage')
);

-- ============================================================================
-- Helper functions
-- ============================================================================

-- Assign a worker to a project
CREATE OR REPLACE FUNCTION assign_worker_to_project(
    p_worker_id uuid,
    p_project_id uuid
)
RETURNS void AS $$
DECLARE
    v_project_env_id uuid;
    v_worker_user_id uuid;
    v_project_user_id uuid;
BEGIN
    -- Get worker and project info
    SELECT user_id INTO v_worker_user_id FROM workers WHERE id = p_worker_id;
    SELECT user_id, environment_id INTO v_project_user_id, v_project_env_id
    FROM projects WHERE id = p_project_id;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'Worker or project not found';
    END IF;

    -- Verify same owner
    IF v_worker_user_id != v_project_user_id THEN
        RAISE EXCEPTION 'Worker and project must belong to same user';
    END IF;

    -- Assign worker to project and sync environment
    UPDATE workers
    SET project_id = p_project_id,
        environment_id = v_project_env_id
    WHERE id = p_worker_id;
END;
$$ LANGUAGE plpgsql;

-- Create a function worker in a project (name IS NULL)
CREATE OR REPLACE FUNCTION create_project_function(
    p_project_id uuid,
    p_code bytea,
    p_code_type enum_code_type
)
RETURNS uuid AS $$
DECLARE
    v_worker_id uuid;
    v_project_user_id uuid;
    v_project_env_id uuid;
BEGIN
    -- Get project info
    SELECT user_id, environment_id INTO v_project_user_id, v_project_env_id
    FROM projects WHERE id = p_project_id;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'Project % not found', p_project_id;
    END IF;

    -- Create worker with name = NULL (function worker)
    INSERT INTO workers (name, user_id, project_id, environment_id)
    VALUES (NULL, v_project_user_id, p_project_id, v_project_env_id)
    RETURNING id INTO v_worker_id;

    -- Create initial deployment
    INSERT INTO worker_deployments (worker_id, version, code, code_type)
    VALUES (v_worker_id, 1, p_code, p_code_type);

    -- Set current version
    UPDATE workers SET current_version = 1 WHERE id = v_worker_id;

    RETURN v_worker_id;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- Function to update project environment (cascades to all workers)
-- ============================================================================

CREATE OR REPLACE FUNCTION update_project_environment(
    p_project_id uuid,
    p_environment_id uuid
)
RETURNS void AS $$
BEGIN
    -- Update project environment
    UPDATE projects
    SET environment_id = p_environment_id
    WHERE id = p_project_id;

    -- Cascade to all workers in the project
    UPDATE workers
    SET environment_id = p_environment_id
    WHERE project_id = p_project_id;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- Function to upgrade a worker to a project
-- ============================================================================

CREATE OR REPLACE FUNCTION upgrade_worker_to_project(p_worker_id uuid)
RETURNS uuid AS $$
DECLARE
    v_worker_name varchar(255);
    v_environment_id uuid;
    v_user_id uuid;
BEGIN
    -- 1. Get worker info
    SELECT name, environment_id, user_id
    INTO v_worker_name, v_environment_id, v_user_id
    FROM workers
    WHERE id = p_worker_id AND project_id IS NULL;  -- Only upgrade standalone workers

    IF NOT FOUND THEN
        RAISE EXCEPTION 'Worker % not found or already in a project', p_worker_id;
    END IF;

    -- 2. Create project with SAME ID and SAME name as worker
    INSERT INTO projects (id, name, user_id, environment_id)
    VALUES (p_worker_id, v_worker_name, v_user_id, v_environment_id);

    -- 3. Link worker to project (environment_id stays the same)
    UPDATE workers
    SET project_id = p_worker_id
    WHERE id = p_worker_id;

    -- 4. Create catch-all route for the main worker (priority 0)
    INSERT INTO project_routes (project_id, pattern, priority, backend_type, worker_id)
    VALUES (p_worker_id, '/*', 0, 'worker'::enum_backend_type, p_worker_id);

    -- Note: No need to migrate domains - worker and project have same ID
    -- Domains pointing to worker_id will work for both worker and project queries

    RETURN p_worker_id;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- Function to resolve worker from request parameters
-- ============================================================================

-- Return type for resolve_worker_from_request
CREATE TYPE request_resolution AS (
    worker_id uuid,
    project_id uuid,
    backend_type enum_backend_type,
    assets_storage_id uuid
);

CREATE OR REPLACE FUNCTION resolve_worker_from_request(
    p_domain varchar DEFAULT NULL,
    p_worker_id uuid DEFAULT NULL,
    p_worker_name varchar DEFAULT NULL,
    p_path varchar DEFAULT '/'
)
RETURNS request_resolution AS $$
DECLARE
    v_result request_resolution;
    v_worker_id_temp uuid;
    v_project_id_temp uuid;
    v_route_record RECORD;
BEGIN
    -- Priority 1: Direct worker_id (used for service bindings, worker-to-worker calls)
    IF p_worker_id IS NOT NULL THEN
        v_result.worker_id := p_worker_id;
        v_result.project_id := NULL;
        v_result.backend_type := 'worker'::enum_backend_type;
        v_result.assets_storage_id := NULL;
        RETURN v_result;
    END IF;

    -- Priority 2: Resolve project_id/worker_id from worker_name (subdomain) or domain (custom domain)
    IF p_worker_name IS NOT NULL THEN
        -- Subdomain case: name.workers.rocks → lookup in endpoints
        -- Projects have priority over standalone workers for routing
        SELECT project_id, worker_id
        INTO v_project_id_temp, v_worker_id_temp
        FROM endpoints
        WHERE name = p_worker_name
        ORDER BY type DESC  -- 'project' > 'worker' alphabetically
        LIMIT 1;

        IF NOT FOUND THEN
            RETURN NULL;  -- Endpoint not found
        END IF;
    ELSIF p_domain IS NOT NULL THEN
        -- Custom domain case: example.com → lookup in domains
        SELECT project_id, worker_id
        INTO v_project_id_temp, v_worker_id_temp
        FROM domains
        WHERE name = p_domain;

        IF NOT FOUND THEN
            RETURN NULL;  -- Domain not found
        END IF;
    ELSE
        -- No identifier provided
        RETURN NULL;
    END IF;

    -- If we have a project, match route pattern
    IF v_project_id_temp IS NOT NULL THEN
        SELECT backend_type, worker_id
        INTO v_route_record
        FROM project_routes
        WHERE project_id = v_project_id_temp
          AND (
            pattern = p_path
            OR (pattern LIKE '%/*' AND p_path LIKE REPLACE(pattern, '/*', '') || '%')
          )
        ORDER BY priority DESC
        LIMIT 1;

        IF NOT FOUND THEN
            RETURN NULL;  -- No route matches
        END IF;

        v_result.project_id := v_project_id_temp;
        v_result.worker_id := v_route_record.worker_id;
        v_result.backend_type := v_route_record.backend_type;

        -- If storage backend, get ASSETS storage_config_id from main worker's environment
        IF v_route_record.backend_type = 'storage' THEN
            SELECT value::uuid
            INTO v_result.assets_storage_id
            FROM environment_values ev
            JOIN workers w ON w.environment_id = ev.environment_id
            WHERE w.id = v_project_id_temp  -- main worker has same id as project
              AND ev.key = 'ASSETS'
              AND ev.type = 'assets';

            IF v_result.assets_storage_id IS NULL THEN
                RETURN NULL;  -- ASSETS binding not found
            END IF;
        ELSE
            v_result.assets_storage_id := NULL;
        END IF;

        RETURN v_result;
    END IF;

    -- If we have a standalone worker (not in a project)
    IF v_worker_id_temp IS NOT NULL THEN
        v_result.worker_id := v_worker_id_temp;
        v_result.project_id := NULL;
        v_result.backend_type := 'worker'::enum_backend_type;
        v_result.assets_storage_id := NULL;
        RETURN v_result;
    END IF;

    -- Nothing matched
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- Function to sync main worker name to project
-- ============================================================================

CREATE OR REPLACE FUNCTION sync_main_worker_name()
RETURNS TRIGGER AS $$
BEGIN
  -- If this worker is a main worker (id = project_id), sync name to project
  IF NEW.project_id IS NOT NULL AND NEW.id = NEW.project_id THEN
    UPDATE projects
    SET name = NEW.name
    WHERE id = NEW.id;
  END IF;

  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- Trigger to sync names on worker update
-- ============================================================================

CREATE TRIGGER sync_main_worker_name_trigger
  AFTER UPDATE OF name ON workers
  FOR EACH ROW
  WHEN (OLD.name IS DISTINCT FROM NEW.name)
  EXECUTE FUNCTION sync_main_worker_name();

-- ============================================================================
-- Comments
-- ============================================================================

COMMENT ON VIEW endpoints IS 'Virtual table combining workers and projects into a shared namespace';
COMMENT ON FUNCTION check_endpoint_name_unique() IS 'Ensures endpoint names are globally unique across workers and projects';
COMMENT ON FUNCTION resolve_worker_from_request(varchar, uuid, varchar, varchar) IS 'Resolves endpoint and route in a single query to determine which worker or storage to use';
COMMENT ON FUNCTION sync_main_worker_name() IS 'Automatically syncs main worker name changes to the corresponding project';
COMMENT ON COLUMN domains.worker_id IS 'Worker target (mutually exclusive with project_id)';
COMMENT ON COLUMN domains.project_id IS 'Project target (mutually exclusive with worker_id)';

COMMIT;
