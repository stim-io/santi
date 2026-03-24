BEGIN;

CREATE TABLE souls (
    id TEXT PRIMARY KEY,
    memory TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT souls_id_non_empty CHECK (char_length(id) > 0)
);

CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    soul_id TEXT NOT NULL,
    memory TEXT NOT NULL DEFAULT '',
    next_session_seq BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT sessions_id_non_empty CHECK (char_length(id) > 0),
    CONSTRAINT sessions_next_session_seq_positive CHECK (next_session_seq > 0),
    CONSTRAINT sessions_soul_fk FOREIGN KEY (soul_id) REFERENCES souls(id) ON DELETE RESTRICT
);

CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    type TEXT NOT NULL,
    role TEXT,
    content TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT messages_id_non_empty CHECK (char_length(id) > 0),
    CONSTRAINT messages_type_check CHECK (
        type IN ('user', 'assistant', 'tool_call', 'tool_result', 'compact')
    ),
    CONSTRAINT messages_role_check CHECK (
        role IS NULL OR role IN ('user', 'assistant')
    ),
    CONSTRAINT messages_type_role_matrix_check CHECK (
        (type = 'user' AND role = 'user') OR
        (type = 'assistant' AND role = 'assistant') OR
        (type IN ('tool_call', 'tool_result', 'compact') AND role IS NULL)
    )
);

CREATE TABLE r_session_messages (
    session_id TEXT NOT NULL,
    message_id TEXT NOT NULL,
    session_seq BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT r_session_messages_pk PRIMARY KEY (session_id, message_id),
    CONSTRAINT r_session_messages_session_seq_positive CHECK (session_seq > 0),
    CONSTRAINT r_session_messages_session_seq_unique UNIQUE (session_id, session_seq),
    CONSTRAINT r_session_messages_session_fk FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
    CONSTRAINT r_session_messages_message_fk FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE RESTRICT
);

CREATE INDEX idx_sessions_soul_id ON sessions (soul_id);
CREATE INDEX idx_messages_created_at ON messages (created_at);
CREATE INDEX idx_messages_type_created_at ON messages (type, created_at);
CREATE INDEX idx_r_session_messages_message_id ON r_session_messages (message_id);
CREATE INDEX idx_r_session_messages_session_seq ON r_session_messages (session_id, session_seq);

COMMIT;
