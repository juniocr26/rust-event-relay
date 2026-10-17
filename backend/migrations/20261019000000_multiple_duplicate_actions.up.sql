-- Additive admission capability; preserve all original actions and observations.
ALTER TABLE demo.actions DROP CONSTRAINT actions_run_id_key;
CREATE INDEX demo_actions_run ON demo.actions(run_id,accepted_at,action_id);
