BEGIN;

CREATE TABLE session_effects (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    effect_type TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    status VARCHAR(20) NOT NULL,
    source_hook_id TEXT NOT NULL,
    source_turn_id TEXT NOT NULL,
    result_ref TEXT,
    error_text TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT session_effects_unique_effect UNIQUE (session_id, effect_type, idempotency_key)
);

CREATE INDEX idx_session_effects_session_created_at
    ON session_effects (session_id, created_at);

CREATE INDEX idx_session_effects_lookup
    ON session_effects (session_id, effect_type, idempotency_key);

COMMIT;
