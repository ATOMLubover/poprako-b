CREATE TABLE agent_checkpoints (
    id UUID PRIMARY KEY,
    session_id UUID NOT NULL REFERENCES agent_sessions(id) ON DELETE CASCADE,
    run_id UUID,
    kind TEXT NOT NULL,
    model TEXT NOT NULL,
    messages JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT agent_checkpoints_kind_check CHECK (kind IN ('before_run', 'after_run', 'fork'))
);

CREATE INDEX agent_checkpoints_session_created_idx
    ON agent_checkpoints (session_id, created_at, id);

CREATE INDEX agent_checkpoints_run_idx
    ON agent_checkpoints (run_id)
    WHERE run_id IS NOT NULL;

ALTER TABLE agent_sessions
    ADD CONSTRAINT agent_sessions_parent_checkpoint_id_fkey
    FOREIGN KEY (parent_checkpoint_id)
    REFERENCES agent_checkpoints(id);
