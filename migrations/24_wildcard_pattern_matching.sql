--
-- OpenWorkers Database Schema - Wildcard Pattern Matching
--
-- Add support for wildcard patterns in routes:
--   * = single path segment (matches anything except /)
--  ** = multiple path segments (matches anything including /)
--
-- Examples:
--   /status/*/*        matches /status/200/OK
--   /range/*           matches /range/1024
--   /drip/**           matches /drip/10/2/0/200
--   /basic-auth/*/*    matches /basic-auth/user/pass
--

BEGIN;

-- ============================================================================
-- Helper function: Convert wildcard pattern to regex
-- ============================================================================

CREATE OR REPLACE FUNCTION pattern_to_regex(pattern TEXT)
RETURNS TEXT AS $$
BEGIN
  -- Convert wildcard pattern to PostgreSQL regex:
  -- ** → .* (match anything including /)
  -- *  → [^/]+ (match anything except /)

  -- Use placeholder for ** to avoid conflicts with single *
  RETURN '^' ||
    regexp_replace(
      regexp_replace(
        regexp_replace(pattern, '\*\*', '__DOUBLESTAR__', 'g'),
        '\*', '[^/]+', 'g'
      ),
      '__DOUBLESTAR__', '.*', 'g'
    ) || '$';
END;
$$ LANGUAGE plpgsql IMMUTABLE;

COMMENT ON FUNCTION pattern_to_regex(TEXT) IS 'Converts wildcard pattern (* and **) to PostgreSQL regex';

-- ============================================================================
-- Helper function: Match path against wildcard pattern
-- ============================================================================

CREATE OR REPLACE FUNCTION match_route_pattern(path TEXT, pattern TEXT)
RETURNS BOOLEAN AS $$
BEGIN
  -- Exact match (no wildcards)
  IF position('*' in pattern) = 0 THEN
    RETURN path = pattern;
  END IF;

  -- Single wildcard /* → prefix match (backward compatible for storage routes)
  IF pattern LIKE '%/*' AND position('**' in pattern) = 0 THEN
    RETURN path LIKE replace(pattern, '/*', '/%');
  END IF;

  -- Double wildcard /** → full regex match (for function routes with multiple segments)
  RETURN path ~ pattern_to_regex(pattern);
END;
$$ LANGUAGE plpgsql IMMUTABLE;

COMMENT ON FUNCTION match_route_pattern(TEXT, TEXT) IS 'Matches a request path against a route pattern with wildcard support';

-- ============================================================================
-- Helper function: Calculate route pattern specificity
-- ============================================================================

CREATE OR REPLACE FUNCTION route_pattern_specificity(pattern TEXT)
RETURNS INTEGER AS $$
BEGIN
  -- Higher score = more specific = higher priority
  -- Static routes get highest priority
  -- Each * reduces score by 1
  -- Each ** reduces score by 10

  IF position('*' in pattern) = 0 THEN
    -- Static route: use length as specificity
    RETURN length(pattern) + 1000;
  END IF;

  -- Count wildcards and reduce specificity
  RETURN
    length(pattern)
    - (length(pattern) - length(replace(pattern, '*', '')))  -- Penalty for each *
    - (length(pattern) - length(replace(pattern, '**', '')) -
       (length(pattern) - length(replace(pattern, '*', '')))) * 9;  -- Extra penalty for **
END;
$$ LANGUAGE plpgsql IMMUTABLE;

COMMENT ON FUNCTION route_pattern_specificity(TEXT) IS 'Calculates route specificity for ordering (higher = more specific)';

-- ============================================================================
-- Update resolve_worker_from_request to use wildcard matching
-- ============================================================================

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
        -- Verify worker exists before returning
        IF EXISTS (SELECT 1 FROM workers WHERE id = p_worker_id) THEN
            v_result.worker_id := p_worker_id;
            v_result.project_id := NULL;
            v_result.backend_type := 'worker'::enum_backend_type;
            v_result.assets_storage_id := NULL;
            v_result.priority := NULL;
            RETURN v_result;
        ELSE
            RETURN NULL;  -- Worker not found
        END IF;
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
        SELECT backend_type, worker_id, priority
        INTO v_route_record
        FROM project_routes
        WHERE project_id = v_project_id_temp
          AND (
            match_route_pattern(p_path, pattern)
            -- Allow /about.html to match route /about (prerendered fallback)
            OR (p_path LIKE '%.html' AND pattern = REPLACE(p_path, '.html', ''))
          )
        ORDER BY
          route_pattern_specificity(pattern) DESC,
          priority DESC
        LIMIT 1;

        IF NOT FOUND THEN
            -- No route matches, but project exists (should be 404)
            v_result.project_id := v_project_id_temp;
            v_result.worker_id := NULL;
            v_result.backend_type := NULL;
            v_result.assets_storage_id := NULL;
            v_result.priority := NULL;
            RETURN v_result;
        END IF;

        v_result.project_id := v_project_id_temp;
        v_result.worker_id := v_route_record.worker_id;
        v_result.backend_type := v_route_record.backend_type;
        v_result.priority := v_route_record.priority;

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
        v_result.priority := NULL;
        RETURN v_result;
    END IF;

    -- Nothing matched
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION resolve_worker_from_request(varchar, uuid, varchar, varchar) IS 'Resolves endpoint and route with wildcard pattern matching (* and **) and .html fallback';

-- ============================================================================
-- Prevent deletion of main workers
-- ============================================================================

CREATE OR REPLACE FUNCTION prevent_main_worker_deletion()
RETURNS TRIGGER AS $$
BEGIN
  -- Check if this worker is a main worker (id matches a project)
  IF EXISTS (SELECT 1 FROM projects WHERE id = OLD.id) THEN
    RAISE EXCEPTION 'Cannot delete main worker - delete the project instead';
  END IF;

  RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER prevent_main_worker_deletion_trigger
  BEFORE DELETE ON workers
  FOR EACH ROW
  EXECUTE FUNCTION prevent_main_worker_deletion();

COMMENT ON FUNCTION prevent_main_worker_deletion() IS 'Prevents direct deletion of main workers - projects must be deleted instead';

COMMIT;
