CREATE TABLE agent_checkpoint_messages (
    checkpoint_id UUID NOT NULL REFERENCES agent_checkpoints(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    message_id UUID NOT NULL REFERENCES agent_messages(id),
    PRIMARY KEY (checkpoint_id, position)
);
