--
-- OpenWorkers Database Schema - D1 limit
--
-- D1 is off by default: an account gets D1 databases when an admin raises
-- its limit (UPDATE users SET limit_d1 = 3 WHERE ...).
--

BEGIN;

ALTER TABLE users ADD COLUMN limit_d1 integer NOT NULL DEFAULT 0;

COMMENT ON COLUMN users.limit_d1 IS 'Max number of D1 databases this user can create; 0 turns D1 off';

COMMIT;
