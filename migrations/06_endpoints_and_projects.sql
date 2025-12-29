--
-- OpenWorkers Database Schema - Endpoints and Projects
--
-- Introduces a shared namespace for workers and projects via the endpoints table.
-- An endpoint owns a subdomain (e.g., "mon-app" -> mon-app.workers.rocks)
-- and points to either a worker (direct dispatch) or a project (route matching).
--

-- ============================================================================
-- ENUMS
-- ============================================================================

CREATE TYPE enum_endpoint_type AS ENUM (
    'worker',
    'project'
);

-- ============================================================================
-- TABLES
-- ============================================================================

-- Projects (container for multiple workers + routes + assets)
CREATE TABLE projects (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    name character varying(255) NOT NULL,
    "desc" character varying(255),
    user_id uuid NOT NULL REFERENCES users(id) ON UPDATE CASCADE ON DELETE CASCADE,
    created_at timestamp with time zone NOT NULL DEFAULT now(),
    updated_at timestamp with time zone NOT NULL DEFAULT now(),
    UNIQUE (id, user_id),
    CHECK (name ~* '^[a-z0-9]([a-z0-9-]*[a-z0-9])?$')
);

-- Endpoints (shared namespace for subdomains)
CREATE TABLE endpoints (
    name character varying(255) PRIMARY KEY,
    type enum_endpoint_type NOT NULL,
    user_id uuid NOT NULL REFERENCES users(id) ON UPDATE CASCADE ON DELETE CASCADE,
    worker_id uuid REFERENCES workers(id) ON UPDATE CASCADE ON DELETE CASCADE,
    project_id uuid REFERENCES projects(id) ON UPDATE CASCADE ON DELETE CASCADE,
    created_at timestamp with time zone NOT NULL DEFAULT now(),
    updated_at timestamp with time zone NOT NULL DEFAULT now(),
    -- Ensure only one target is set based on type
    CHECK (
        (type = 'worker' AND worker_id IS NOT NULL AND project_id IS NULL) OR
        (type = 'project' AND project_id IS NOT NULL AND worker_id IS NULL)
    ),
    CHECK (name ~* '^[a-z0-9]([a-z0-9-]*[a-z0-9])?$')
);

-- Project routes (routing rules for projects)
CREATE TABLE project_routes (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id uuid NOT NULL REFERENCES projects(id) ON UPDATE CASCADE ON DELETE CASCADE,
    pattern character varying(255) NOT NULL,  -- e.g., "/api/users/*"
    priority integer NOT NULL DEFAULT 0,       -- higher = matched first
    backend_type character varying(50) NOT NULL,  -- 'worker', 's3', 'http'
    backend_target character varying(255) NOT NULL,  -- worker name, s3 bucket, or URL
    cache_ttl integer,  -- optional cache TTL in seconds
    created_at timestamp with time zone NOT NULL DEFAULT now(),
    updated_at timestamp with time zone NOT NULL DEFAULT now(),
    UNIQUE (project_id, pattern)
);

-- ============================================================================
-- MIGRATION: Existing workers -> endpoints
-- ============================================================================

INSERT INTO endpoints (name, type, user_id, worker_id, created_at, updated_at)
SELECT name, 'worker'::enum_endpoint_type, user_id, id, created_at, updated_at
FROM workers;

-- ============================================================================
-- CONSTRAINTS: Remove UNIQUE from workers.name (endpoints owns the namespace)
-- ============================================================================

ALTER TABLE workers DROP CONSTRAINT workers_name_key;

-- ============================================================================
-- INDEXES
-- ============================================================================

CREATE INDEX idx_endpoints_user_id ON endpoints(user_id);
CREATE INDEX idx_endpoints_type ON endpoints(type);
CREATE INDEX idx_projects_user_id ON projects(user_id);
CREATE INDEX idx_project_routes_project_id ON project_routes(project_id);
CREATE INDEX idx_project_routes_priority ON project_routes(project_id, priority DESC);
