-- Forward-only adoption into the main application; historical migrations stay immutable.
ALTER TABLE demo.runs DROP CONSTRAINT runs_scenario_check;
ALTER TABLE demo.runs ADD CONSTRAINT runs_scenario_check CHECK(scenario IN ('normal','dedupe','concurrent','recovery'));
CREATE TABLE demo.actions (
 action_id UUID PRIMARY KEY,
 request_id UUID NOT NULL UNIQUE,
 run_id UUID NOT NULL UNIQUE REFERENCES demo.runs,
 copies SMALLINT NOT NULL CHECK(copies IN (1,3)),
 state TEXT NOT NULL DEFAULT 'queued' CHECK(state IN ('queued','executing','confirmed','unknown','failed')),
 accepted_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 finished_at TIMESTAMPTZ,
 queue_name TEXT NOT NULL UNIQUE
);
CREATE TABLE demo.consumer_observations (
 observation_id UUID PRIMARY KEY,
 run_id UUID NOT NULL REFERENCES demo.runs,
 event_id UUID NOT NULL REFERENCES relay.outbox_events,
 action_id UUID REFERENCES demo.actions,
 context TEXT NOT NULL CHECK(context IN ('main','slot-1','slot-2')),
 outcome TEXT NOT NULL CHECK(outcome IN ('Applied','Duplicate')),
 observed_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX demo_observations_run ON demo.consumer_observations(run_id);
CREATE TABLE demo.publications (
 observation_id UUID PRIMARY KEY,
 run_id UUID NOT NULL REFERENCES demo.runs,
 action_id UUID REFERENCES demo.actions,
 ordinal SMALLINT NOT NULL CHECK(ordinal BETWEEN 0 AND 3),
 outcome TEXT NOT NULL CHECK(outcome IN ('confirmed','unknown','failed')),
 observed_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX demo_publications_run ON demo.publications(run_id);
CREATE TABLE demo.faults (
 run_id UUID PRIMARY KEY REFERENCES demo.runs,
 event_id UUID NOT NULL UNIQUE REFERENCES relay.outbox_events DEFERRABLE INITIALLY DEFERRED,
 condition TEXT NOT NULL CHECK(condition='reject_once_before_send'),
 injected_at TIMESTAMPTZ,
 retry_available_at TIMESTAMPTZ,
 retry_revision BIGINT
);
REVOKE ALL ON demo.actions,demo.consumer_observations,demo.publications,demo.faults FROM PUBLIC;
-- Preserve the actual ownership-protected retry schedule in the SAME commit.
CREATE FUNCTION demo.capture_retry() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 UPDATE demo.faults SET retry_available_at=NEW.available_at,retry_revision=NEW.state_revision
 WHERE event_id=NEW.id AND injected_at IS NOT NULL AND retry_available_at IS NULL
   AND NEW.diagnostic_code='rejected' AND NEW.failure_evidence->>'phase'='before_send'
   AND NEW.ownership_token IS NULL AND NEW.status='pending';
 RETURN NULL;
END $$;
CREATE TRIGGER demo_retry_evidence AFTER UPDATE OF available_at ON relay.outbox_events
 FOR EACH ROW EXECUTE FUNCTION demo.capture_retry();
REVOKE ALL ON FUNCTION demo.capture_retry() FROM PUBLIC;
