-- ============================================================================
-- Procedure to link an environment to a worker.
--
-- If the worker belongs to a project, cascades the environment to the project
-- and all sibling workers in a single transaction.
-- Replaces the trigger-based constraint with an explicit procedure.
-- ============================================================================

-- Drop the old triggers that blocked the operation
DROP TRIGGER IF EXISTS check_workers_project_environment ON workers;
DROP TRIGGER IF EXISTS check_projects_environment ON projects;
DROP FUNCTION IF EXISTS check_worker_project_environment();

CREATE OR REPLACE FUNCTION link_worker_environment(
    p_worker_id uuid,
    p_environment_id uuid
)
RETURNS void AS $$
DECLARE
    v_project_id uuid;
    v_worker_user_id uuid;
    v_env_user_id uuid;
BEGIN
    SELECT project_id, user_id INTO v_project_id, v_worker_user_id
    FROM workers
    WHERE id = p_worker_id;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'Worker not found';
    END IF;

    SELECT user_id INTO v_env_user_id
    FROM environments
    WHERE id = p_environment_id;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'Environment not found';
    END IF;

    IF v_worker_user_id IS DISTINCT FROM v_env_user_id THEN
        RAISE EXCEPTION 'Worker and environment must belong to the same user';
    END IF;

    IF v_project_id IS NOT NULL THEN
        -- Worker is in a project: cascade to project + all its workers
        UPDATE projects
        SET environment_id = p_environment_id, updated_at = now()
        WHERE id = v_project_id;

        UPDATE workers
        SET environment_id = p_environment_id, updated_at = now()
        WHERE project_id = v_project_id;
    ELSE
        -- Standalone worker
        UPDATE workers
        SET environment_id = p_environment_id, updated_at = now()
        WHERE id = p_worker_id;
    END IF;
END;
$$ LANGUAGE plpgsql;
