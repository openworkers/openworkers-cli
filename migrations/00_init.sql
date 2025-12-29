--
-- OpenWorkers Database Schema - Initial Schema
--

SET client_encoding = 'UTF8';

-- ============================================================================
-- ENUMS
-- ============================================================================

CREATE TYPE enum_external_users_provider AS ENUM (
    'github'
);

CREATE TYPE enum_logs_level AS ENUM (
    'error',
    'warn',
    'info',
    'log',
    'debug',
    'trace'
);

CREATE TYPE enum_workers_language AS ENUM (
    'javascript',
    'typescript'
);

-- ============================================================================
-- FUNCTIONS
-- ============================================================================

CREATE FUNCTION cron_update() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
DECLARE
BEGIN
  PERFORM pg_notify('cron_update', TG_OP);
  RETURN NEW;
END;
$$;

-- ============================================================================
-- TABLES
-- ============================================================================

-- Users
CREATE TABLE users (
    id uuid PRIMARY KEY,
    username character varying(255) UNIQUE NOT NULL,
    resource_limits jsonb DEFAULT '{"workers": 5, "environments": 5, "databases": 3}'::jsonb NOT NULL,
    avatar_url character varying(255) DEFAULT NULL,
    created_at timestamp with time zone NOT NULL,
    updated_at timestamp with time zone NOT NULL
);

-- External Users (OAuth)
CREATE TABLE external_users (
    provider enum_external_users_provider NOT NULL,
    external_id character varying(255) NOT NULL,
    user_id uuid PRIMARY KEY REFERENCES users(id) ON UPDATE CASCADE ON DELETE CASCADE,
    created_at timestamp with time zone NOT NULL,
    updated_at timestamp with time zone NOT NULL,
    UNIQUE (provider, external_id)
);

-- Environments
CREATE TABLE environments (
    id uuid PRIMARY KEY,
    name character varying(255) NOT NULL,
    "desc" character varying(255),
    user_id uuid NOT NULL REFERENCES users(id),
    created_at timestamp with time zone NOT NULL,
    updated_at timestamp with time zone NOT NULL,
    UNIQUE (id, user_id)
);

-- Environment Values
CREATE TABLE environment_values (
    id uuid PRIMARY KEY,
    environment_id uuid NOT NULL,
    user_id uuid NOT NULL,
    key character varying(255) NOT NULL,
    value character varying(255) NOT NULL,
    secret boolean DEFAULT false,
    created_at timestamp with time zone NOT NULL,
    updated_at timestamp with time zone NOT NULL,
    UNIQUE (environment_id, user_id, key),
    FOREIGN KEY (environment_id) REFERENCES environments(id) ON UPDATE CASCADE ON DELETE CASCADE,
    FOREIGN KEY (environment_id, user_id) REFERENCES environments(id, user_id) ON UPDATE CASCADE ON DELETE CASCADE
);

-- Workers
CREATE TABLE workers (
    id uuid PRIMARY KEY,
    name character varying(255) UNIQUE NOT NULL,
    "desc" character varying(255),
    user_id uuid NOT NULL REFERENCES users(id) ON UPDATE CASCADE ON DELETE CASCADE,
    environment_id uuid REFERENCES environments(id) ON UPDATE CASCADE ON DELETE SET NULL,
    script text DEFAULT ''::text,
    language enum_workers_language NOT NULL,
    created_at timestamp with time zone NOT NULL,
    updated_at timestamp with time zone NOT NULL,
    UNIQUE (id, user_id),
    FOREIGN KEY (environment_id, user_id) REFERENCES environments(id, user_id) ON UPDATE CASCADE ON DELETE CASCADE,
    CHECK (name ~* '^[a-z0-9]([a-z0-9-]*[a-z0-9])?$')
);

-- Domains
CREATE TABLE domains (
    name character varying(255) PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users(id) ON UPDATE CASCADE ON DELETE CASCADE,
    worker_id uuid NOT NULL REFERENCES workers(id) ON UPDATE CASCADE ON DELETE CASCADE,
    created_at timestamp with time zone NOT NULL,
    updated_at timestamp with time zone NOT NULL
);

-- Crons
CREATE TABLE crons (
    id uuid PRIMARY KEY,
    value character varying(255) NOT NULL,
    worker_id uuid NOT NULL REFERENCES workers(id) ON UPDATE CASCADE ON DELETE CASCADE,
    last_run timestamp with time zone,
    next_run timestamp with time zone,
    created_at timestamp with time zone NOT NULL,
    updated_at timestamp with time zone NOT NULL,
    deleted_at timestamp with time zone
);

-- Scheduled Events
CREATE TABLE scheduled_events (
    id uuid PRIMARY KEY,
    cron_id uuid NOT NULL REFERENCES crons(id) ON UPDATE CASCADE ON DELETE CASCADE,
    worker_id uuid NOT NULL REFERENCES workers(id),
    replied_at timestamp with time zone,
    executed_at timestamp with time zone NOT NULL,
    scheduled_at timestamp with time zone NOT NULL
);

-- Logs
CREATE TABLE logs (
    date timestamp with time zone NOT NULL,
    worker_id uuid NOT NULL REFERENCES workers(id) ON UPDATE CASCADE ON DELETE CASCADE,
    message character varying(255) NOT NULL,
    level enum_logs_level NOT NULL
);

-- ============================================================================
-- TRIGGERS
-- ============================================================================

CREATE TRIGGER cron_update
    AFTER INSERT OR DELETE OR UPDATE ON crons
    FOR EACH ROW
    EXECUTE FUNCTION cron_update();
