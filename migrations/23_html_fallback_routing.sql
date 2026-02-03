--
-- OpenWorkers Database Schema - Add HTML fallback routing
--
-- Allow requests with .html extension to match prerendered routes
-- without the extension. For example, /about.html matches route /about.
--

BEGIN;

-- ============================================================================
-- Update resolve_worker_from_request to handle .html fallback
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
        v_result.worker_id := p_worker_id;
        v_result.project_id := NULL;
        v_result.backend_type := 'worker'::enum_backend_type;
        v_result.assets_storage_id := NULL;
        v_result.priority := NULL;
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
        SELECT backend_type, worker_id, priority
        INTO v_route_record
        FROM project_routes
        WHERE project_id = v_project_id_temp
          AND (
            pattern = p_path
            OR (pattern LIKE '%/*' AND p_path LIKE REPLACE(pattern, '/*', '') || '%')
            -- Allow /about.html to match route /about (prerendered fallback)
            OR (p_path LIKE '%.html' AND pattern = REPLACE(p_path, '.html', ''))
          )
        ORDER BY priority DESC
        LIMIT 1;

        IF NOT FOUND THEN
            RETURN NULL;  -- No route matches
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

COMMENT ON FUNCTION resolve_worker_from_request(varchar, uuid, varchar, varchar) IS 'Resolves endpoint and route with .html fallback support for prerendered routes';

COMMIT;
