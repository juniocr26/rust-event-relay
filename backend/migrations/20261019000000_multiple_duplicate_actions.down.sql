-- Refuse rollback when multiple actions exist; never discard evidence to force it.
ALTER TABLE demo.actions ADD CONSTRAINT actions_run_id_key UNIQUE(run_id);
DROP INDEX demo.demo_actions_run;
