--
-- Migration 14: Password Authentication
--
-- Adds support for email/password authentication alongside GitHub OAuth.
-- Flow: Register email -> Click link -> Set password (validates email implicitly)
--

-- ============================================================================
-- USER TABLE CHANGES
-- ============================================================================

-- Password hash for email-based authentication (NULL for OAuth-only users or pending registration)
ALTER TABLE users ADD COLUMN password_hash VARCHAR(255) DEFAULT NULL;

-- ============================================================================
-- AUTH TOKEN TYPE ENUM
-- ============================================================================

CREATE TYPE auth_token_type AS ENUM ('set_password', 'password_reset');

-- ============================================================================
-- AUTH TOKENS TABLE
-- ============================================================================

-- Tokens for setting password (registration) and password reset
CREATE TABLE auth_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token VARCHAR(64) NOT NULL UNIQUE,
    type auth_token_type NOT NULL,
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Index for token lookup (most common query)
CREATE INDEX idx_auth_tokens_token ON auth_tokens(token);

-- Index for finding user's tokens by type (for cleanup/invalidation)
CREATE INDEX idx_auth_tokens_user_type ON auth_tokens(user_id, type);

-- Auto-cleanup trigger for expired tokens
CREATE OR REPLACE FUNCTION cleanup_expired_auth_tokens()
RETURNS TRIGGER AS $$
BEGIN
    DELETE FROM auth_tokens WHERE expires_at < CURRENT_TIMESTAMP;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- Trigger runs periodically on INSERT to clean old tokens
CREATE TRIGGER trg_cleanup_expired_tokens
    AFTER INSERT ON auth_tokens
    FOR EACH STATEMENT
    EXECUTE FUNCTION cleanup_expired_auth_tokens();

-- ============================================================================
-- COMMENTS
-- ============================================================================

COMMENT ON COLUMN users.password_hash IS 'PBKDF2 hash for email-based authentication. NULL for OAuth-only users or pending registration.';
COMMENT ON TABLE auth_tokens IS 'Temporary tokens for set-password (registration) and password reset flows.';
COMMENT ON COLUMN auth_tokens.type IS 'Token type: set_password (24h expiry, registration) or password_reset (1h expiry).';
