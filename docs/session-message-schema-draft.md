# Session Message Schema Draft

This file translates `docs/session-message-model-spec.md` into a from-scratch PostgreSQL schema draft.

Assumptions:

- no backward compatibility
- no transition tables
- no legacy field preservation
- preserve existing directory layout and migration style preferences

## Migration Style To Preserve

Observed from `crates/santi-db/src/db/migrations/0001_init.sql`:

- one numbered SQL migration file under `crates/santi-db/src/db/migrations/`
- wrap the migration in `BEGIN; ... COMMIT;`
- use explicit named constraints
- prefer `TEXT` ids over database-generated surrogate ids
- use `TIMESTAMPTZ NOT NULL DEFAULT NOW()` for timestamps
- add explicit indexes after table creation
- keep foreign-key ownership in the business layer rather than the database

## Target Replacement

Replace the current init migration with a new clean-slate init migration, for example:

- `crates/santi-db/src/db/migrations/0001_init.sql`

No compatibility layer is needed.

## Schema Outline

### Public Ledger

- `accounts`
- `souls`
- `sessions`
- `messages`
- `r_session_messages`
- `message_events`

### Soul Runtime

- `soul_sessions`
- `turns`
- `tool_calls`
- `tool_results`
- `compacts`
- `r_soul_session_messages`

## Draft SQL

```sql
BEGIN;

CREATE TABLE accounts (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE souls (
    id TEXT PRIMARY KEY,
    memory TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    actor_type TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    content JSONB NOT NULL,
    state TEXT NOT NULL,
    version BIGINT NOT NULL DEFAULT 1,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT messages_actor_type_check CHECK (actor_type IN ('account', 'soul', 'system')),
    CONSTRAINT messages_state_check CHECK (state IN ('pending', 'fixed')),
    CONSTRAINT messages_version_positive CHECK (version > 0)
);

CREATE TABLE r_session_messages (
    session_id TEXT NOT NULL,
    message_id TEXT NOT NULL,
    session_seq BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT r_session_messages_pk PRIMARY KEY (session_id, message_id),
    CONSTRAINT r_session_messages_session_seq_positive CHECK (session_seq > 0),
    CONSTRAINT r_session_messages_session_seq_unique UNIQUE (session_id, session_seq)
);

CREATE TABLE message_events (
    id TEXT PRIMARY KEY,
    message_id TEXT NOT NULL,
    action TEXT NOT NULL,
    actor_type TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    base_version BIGINT NOT NULL,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT message_events_action_check CHECK (action IN ('patch', 'insert', 'remove', 'fix', 'delete')),
    CONSTRAINT message_events_actor_type_check CHECK (actor_type IN ('account', 'soul', 'system')),
    CONSTRAINT message_events_base_version_positive CHECK (base_version > 0)
);

CREATE TABLE soul_sessions (
    id TEXT PRIMARY KEY,
    soul_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    session_memory TEXT NOT NULL DEFAULT '',
    provider_state JSONB,
    next_seq BIGINT NOT NULL DEFAULT 1,
    last_seen_session_seq BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT soul_sessions_next_seq_positive CHECK (next_seq > 0),
    CONSTRAINT soul_sessions_last_seen_session_seq_non_negative CHECK (last_seen_session_seq >= 0),
    CONSTRAINT soul_sessions_soul_session_unique UNIQUE (soul_id, session_id)
);

CREATE TABLE turns (
    id TEXT PRIMARY KEY,
    soul_session_id TEXT NOT NULL,
    trigger_type TEXT NOT NULL,
    trigger_ref TEXT,
    input_through_session_seq BIGINT NOT NULL,
    base_soul_session_seq BIGINT NOT NULL,
    end_soul_session_seq BIGINT,
    status TEXT NOT NULL,
    error_text TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at TIMESTAMPTZ,
    CONSTRAINT turns_trigger_type_check CHECK (trigger_type IN ('session_send', 'system')),
    CONSTRAINT turns_input_through_session_seq_non_negative CHECK (input_through_session_seq >= 0),
    CONSTRAINT turns_base_soul_session_seq_non_negative CHECK (base_soul_session_seq >= 0),
    CONSTRAINT turns_end_soul_session_seq_non_negative CHECK (end_soul_session_seq IS NULL OR end_soul_session_seq >= 0),
    CONSTRAINT turns_status_check CHECK (status IN ('running', 'completed', 'failed'))
);

CREATE TABLE tool_calls (
    id TEXT PRIMARY KEY,
    turn_id TEXT NOT NULL,
    tool_name TEXT NOT NULL,
    arguments JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE tool_results (
    id TEXT PRIMARY KEY,
    tool_call_id TEXT NOT NULL,
    output JSONB,
    error_text TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT tool_results_tool_call_unique UNIQUE (tool_call_id),
    CONSTRAINT tool_results_terminal_shape_check CHECK (
        (output IS NOT NULL AND error_text IS NULL) OR
        (output IS NULL AND error_text IS NOT NULL)
    )
);

CREATE TABLE compacts (
    id TEXT PRIMARY KEY,
    turn_id TEXT NOT NULL,
    summary TEXT NOT NULL,
    start_session_seq BIGINT NOT NULL,
    end_session_seq BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT compacts_start_session_seq_positive CHECK (start_session_seq > 0),
    CONSTRAINT compacts_end_session_seq_positive CHECK (end_session_seq > 0),
    CONSTRAINT compacts_interval_check CHECK (start_session_seq <= end_session_seq)
);

CREATE TABLE r_soul_session_messages (
    soul_session_id TEXT NOT NULL,
    target_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    soul_session_seq BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT r_soul_session_messages_pk PRIMARY KEY (soul_session_id, target_type, target_id),
    CONSTRAINT r_soul_session_messages_soul_session_seq_unique UNIQUE (soul_session_id, soul_session_seq),
    CONSTRAINT r_soul_session_messages_target_type_check CHECK (
        target_type IN ('message', 'compact', 'tool_call', 'tool_result')
    ),
    CONSTRAINT r_soul_session_messages_soul_session_seq_positive CHECK (soul_session_seq > 0),
    CONSTRAINT r_soul_session_messages_target_id_non_empty CHECK (char_length(target_id) > 0)
);

CREATE INDEX idx_messages_actor_created_at ON messages (actor_type, actor_id, created_at);
CREATE INDEX idx_messages_state_created_at ON messages (state, created_at);
CREATE INDEX idx_r_session_messages_message_id ON r_session_messages (message_id);
CREATE INDEX idx_r_session_messages_session_seq ON r_session_messages (session_id, session_seq);
CREATE INDEX idx_message_events_message_id_created_at ON message_events (message_id, created_at);
CREATE INDEX idx_soul_sessions_session_id ON soul_sessions (session_id);
CREATE INDEX idx_soul_sessions_soul_id ON soul_sessions (soul_id);
CREATE INDEX idx_turns_soul_session_created_at ON turns (soul_session_id, created_at);
CREATE INDEX idx_turns_soul_session_status_created_at ON turns (soul_session_id, status, created_at);
CREATE INDEX idx_tool_calls_turn_id_created_at ON tool_calls (turn_id, created_at);
CREATE INDEX idx_tool_results_tool_call_id ON tool_results (tool_call_id);
CREATE INDEX idx_compacts_turn_id_created_at ON compacts (turn_id, created_at);
CREATE INDEX idx_r_soul_session_messages_target_lookup ON r_soul_session_messages (target_type, target_id);
CREATE INDEX idx_r_soul_session_messages_seq ON r_soul_session_messages (soul_session_id, soul_session_seq);

COMMIT;
```

## Notes To Lock Before Implementation

- `messages.actor_id` stays `TEXT NOT NULL`; exact `system.actor_id` persistence remains a runtime/model decision
- `sessions` has no participant list and no `soul_id`
- `soul_sessions` is the only durable `(soul_id, session_id)` runtime container
- `tool_call`, `tool_result`, and `compact` ownership is derived through `turn_id`
- `r_soul_session_messages` stays as the only provider-assembly ordering table
- all cross-table ownership and existence checks live in runtime/repo logic, not SQL foreign keys or triggers
- target-specific same-`soul_session` validation for `r_soul_session_messages` lives in runtime/repo logic
- validation of `messages.content.parts[]` item shape should live in runtime/repo logic first rather than SQL
- turn lifecycle shape stays in runtime logic first rather than SQL state-shape checks

## Business-Layer Integrity Rules

- every referenced `_id` must be existence-checked before write
- `r_session_messages` writes must verify both `session_id` and `message_id`
- `message_events` writes must verify `message_id` plus actor legitimacy
- `soul_sessions` writes must verify both `soul_id` and `session_id`
- `turns`, `tool_calls`, `tool_results`, and `compacts` must verify their owner chain before write
- `r_soul_session_messages` must verify that targets resolve into the same `soul_session`
- `message` targets in `r_soul_session_messages` must already belong to the matching public `session`
- hard deletes, if ever allowed, must be ordered explicitly in the business layer; do not rely on cascade behavior

## Recommended Build Order

1. Replace `0001_init.sql` with the clean-slate schema above
2. Update `santi-core` models to match the canonical names exactly
3. Rebuild `santi-db` stores around the new table ownership rules
4. Rebuild `session/send` around `turns`, runtime artifacts, and `r_soul_session_messages`
5. Reconnect e2e specs only after the new schema is the single truth
