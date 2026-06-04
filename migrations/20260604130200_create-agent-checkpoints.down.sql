ALTER TABLE agent_sessions
    DROP CONSTRAINT agent_sessions_forked_from_fkey;

DROP INDEX agent_checkpoints_session_created_idx;

DROP TABLE agent_checkpoints;
