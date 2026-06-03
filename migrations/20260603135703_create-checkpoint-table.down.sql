ALTER TABLE agent_sessions
    DROP CONSTRAINT agent_sessions_parent_checkpoint_id_fkey;

DROP INDEX agent_checkpoints_run_idx;

DROP INDEX agent_checkpoints_session_created_idx;

DROP TABLE agent_checkpoints;
