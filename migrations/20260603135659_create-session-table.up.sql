CREATE TABLE agent_sessions (
    id UUID PRIMARY KEY,
    name TEXT,
    model TEXT NOT NULL,
    status TEXT NOT NULL,
    parent_session_id UUID REFERENCES agent_sessions(id),
    parent_checkpoint_id UUID,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT agent_sessions_status_check CHECK (status IN ('active', 'archived'))
);
