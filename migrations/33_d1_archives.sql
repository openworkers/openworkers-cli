--
-- OpenWorkers Database Schema - D1 snapshots
--
-- The runner copies each changed D1 database to object storage (R2) every
-- few minutes. d1_archives indexes the copies whose upload was verified; a
-- restore reads the latest one. runner_epochs gives each runner start a
-- number that only grows: an archive row is accepted only from the latest
-- epoch, so a runner that a newer one replaced cannot publish a snapshot.
--

BEGIN;

CREATE TABLE runner_epochs (
    epoch bigserial PRIMARY KEY,
    started_at timestamp with time zone NOT NULL DEFAULT now()
);

CREATE TABLE d1_archives (
    database_id uuid NOT NULL REFERENCES d1_databases(id) ON DELETE CASCADE,
    seq bigint NOT NULL,
    epoch bigint NOT NULL REFERENCES runner_epochs(epoch),
    key text NOT NULL,
    checksum character(64) NOT NULL,
    size bigint NOT NULL,
    created_at timestamp with time zone NOT NULL DEFAULT now(),

    PRIMARY KEY (database_id, seq)
);

COMMENT ON TABLE d1_archives IS 'Verified D1 snapshots in object storage; checksum is the SHA-256 of the stored object';

COMMIT;
