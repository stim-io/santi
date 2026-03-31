BEGIN;

ALTER TABLE sessions
    ADD COLUMN IF NOT EXISTS parent_session_id TEXT,
    ADD COLUMN IF NOT EXISTS fork_point BIGINT;

ALTER TABLE soul_sessions
    ADD COLUMN IF NOT EXISTS parent_soul_session_id TEXT REFERENCES soul_sessions(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS fork_point BIGINT;

CREATE INDEX IF NOT EXISTS idx_sessions_lineage
    ON sessions (parent_session_id, fork_point)
    WHERE parent_session_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_soul_sessions_lineage
    ON soul_sessions (parent_soul_session_id, fork_point)
    WHERE parent_soul_session_id IS NOT NULL;

COMMIT;
