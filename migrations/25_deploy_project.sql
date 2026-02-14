-- ============================================================================
-- Function to deploy a project with routes and function workers in one call.
--
-- Replaces N individual INSERT round-trips with a single DB call.
-- Handles: main worker deployment, storage routes, function workers + routes.
-- ============================================================================

CREATE OR REPLACE FUNCTION deploy_project(
    p_worker_id uuid,
    p_user_id uuid,
    p_script bytea,
    p_hash varchar(64),
    p_language enum_code_type,
    p_storage_routes jsonb DEFAULT '[]'::jsonb,
    p_function_workers jsonb DEFAULT '[]'::jsonb
)
RETURNS TABLE(
    out_project_id uuid,
    out_next_version int,
    functions_created int
) AS $$
DECLARE
    v_project_id uuid;
    v_next_version int;
    v_environment_id uuid;
    v_func jsonb;
    v_func_worker_id uuid;
    v_func_hash varchar(64);
    v_functions_created int := 0;
BEGIN
    -- 1. Deploy main worker script
    SELECT COALESCE(MAX(version), 0) + 1
    INTO v_next_version
    FROM worker_deployments
    WHERE worker_id = p_worker_id;

    INSERT INTO worker_deployments (worker_id, version, hash, code_type, code, message)
    VALUES (p_worker_id, v_next_version, p_hash, p_language, p_script, 'Upload via CLI');

    UPDATE workers SET current_version = v_next_version WHERE id = p_worker_id;

    -- 2. Ensure worker is in a project
    SELECT w.project_id, w.environment_id
    INTO v_project_id, v_environment_id
    FROM workers w
    WHERE w.id = p_worker_id;

    IF v_project_id IS NULL THEN
        PERFORM upgrade_worker_to_project(p_worker_id);
        v_project_id := p_worker_id;
    END IF;

    -- 3. Clear old routes (keep catch-all at priority 0)
    DELETE FROM project_routes pr WHERE pr.project_id = v_project_id AND pr.priority > 0;

    -- 4. Delete old function workers (anonymous workers in this project, excluding main)
    DELETE FROM workers w
    WHERE w.project_id = v_project_id
      AND w.id != p_worker_id
      AND w.name IS NULL;

    -- 5. Insert storage routes in bulk
    INSERT INTO project_routes (project_id, pattern, priority, backend_type)
    SELECT v_project_id, r->>'pattern', (r->>'priority')::int, 'storage'::enum_backend_type
    FROM jsonb_array_elements(p_storage_routes) AS r
    ON CONFLICT (project_id, pattern) DO UPDATE
    SET priority = EXCLUDED.priority, backend_type = 'storage'::enum_backend_type;

    -- 6. Create function workers and their routes
    FOR v_func IN SELECT * FROM jsonb_array_elements(p_function_workers)
    LOOP
        -- Create anonymous worker
        INSERT INTO workers (name, user_id, environment_id, project_id, current_version)
        VALUES (NULL, p_user_id, v_environment_id, v_project_id, 1)
        RETURNING id INTO v_func_worker_id;

        -- Hash the script
        v_func_hash := encode(sha256(convert_to(v_func->>'script', 'UTF8')), 'hex');

        -- Create deployment
        INSERT INTO worker_deployments (worker_id, version, hash, code_type, code, message)
        VALUES (
            v_func_worker_id,
            1,
            v_func_hash,
            'javascript'::enum_code_type,
            convert_to(v_func->>'script', 'UTF8'),
            'Function worker'
        );

        -- Create route
        INSERT INTO project_routes (project_id, pattern, priority, backend_type, worker_id)
        VALUES (v_project_id, v_func->>'pattern', 10, 'worker'::enum_backend_type, v_func_worker_id)
        ON CONFLICT (project_id, pattern) DO UPDATE
        SET priority = 10, backend_type = 'worker'::enum_backend_type, worker_id = v_func_worker_id;

        v_functions_created := v_functions_created + 1;
    END LOOP;

    RETURN QUERY SELECT v_project_id, v_next_version, v_functions_created;
END;
$$ LANGUAGE plpgsql;
