CREATE TABLE agent_checkpoints (
    id UUID PRIMARY KEY,
    session_id UUID NOT NULL REFERENCES agent_sessions(id) ON DELETE CASCADE,
    solution_id UUID,
    kind TEXT NOT NULL,
    model TEXT NOT NULL,
    base_checkpoint_id UUID REFERENCES agent_checkpoints(id),
    created_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT agent_checkpoints_kind_check CHECK (kind IN ('before_solution', 'after_solution', 'fork'))
);

CREATE INDEX agent_checkpoints_session_created_idx
    ON agent_checkpoints (session_id, created_at, id);

ALTER TABLE agent_sessions
    ADD CONSTRAINT agent_sessions_forked_from_fkey
    FOREIGN KEY (forked_from_checkpoint_id)
    REFERENCES agent_checkpoints(id);
