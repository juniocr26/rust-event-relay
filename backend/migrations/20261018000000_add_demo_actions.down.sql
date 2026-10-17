DROP TRIGGER demo_retry_evidence ON relay.outbox_events;
DROP FUNCTION demo.capture_retry();
DROP TABLE demo.faults;
DROP TABLE demo.publications;
DROP TABLE demo.consumer_observations;
DROP TABLE demo.actions;
-- Do not erase newer run evidence merely to restore an earlier CHECK.
ALTER TABLE demo.runs DROP CONSTRAINT runs_scenario_check;
ALTER TABLE demo.runs ADD CONSTRAINT runs_scenario_check CHECK(scenario IN ('normal','dedupe','concurrent','recovery'));
