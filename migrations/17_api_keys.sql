--
-- Migration 15: API Keys
--
-- Adds support for API keys for programmatic access outside the dashboard.
-- Tokens are hashed (SHA-256) for security - only the prefix is stored in plain text.
--

-- ============================================================================
-- API KEYS TABLE
-- ============================================================================

CREATE TABLE api_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    "desc" TEXT,
    token_prefix VARCHAR(12) NOT NULL,
    token_hash VARCHAR(64) NOT NULL UNIQUE,
    last_used_at TIMESTAMP WITH TIME ZONE,
    expires_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Index for token lookup (most common query - auth)
CREATE INDEX idx_api_keys_token_hash ON api_keys(token_hash);

-- Index for user's keys listing
CREATE INDEX idx_api_keys_user_id ON api_keys(user_id);

-- ============================================================================
-- COMMENTS
-- ============================================================================

COMMENT ON TABLE api_keys IS 'API keys for programmatic access. Tokens are SHA-256 hashed.';
COMMENT ON COLUMN api_keys.token_prefix IS 'First 12 chars of token (e.g., "ow_a1b2c3d4") for identification in UI.';
COMMENT ON COLUMN api_keys.token_hash IS 'SHA-256 hash of the full token.';
COMMENT ON COLUMN api_keys.expires_at IS 'Optional expiration. NULL means never expires.';
