CREATE TABLE agent_messages (
    id UUID PRIMARY KEY,
    payload_hash BYTEA NOT NULL,
    role TEXT NOT NULL,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT agent_messages_payload_hash_key UNIQUE (payload_hash),
    CONSTRAINT agent_messages_role_check CHECK (role IN ('system', 'user', 'assistant', 'tool'))
);
