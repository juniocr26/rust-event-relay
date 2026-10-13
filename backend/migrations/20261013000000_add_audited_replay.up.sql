-- Durable global request identity; complete intent and prior terminal evidence.
CREATE TABLE relay.replay_requests (
 request_id UUID PRIMARY KEY CHECK (request_id <> '00000000-0000-0000-0000-000000000000'),
 actor TEXT NOT NULL CHECK (length(actor) BETWEEN 1 AND 64),
 scope TEXT NOT NULL CHECK (scope = 'outbox:replay'),
 event_id UUID NOT NULL REFERENCES relay.outbox_events(id),
 intent JSONB NOT NULL CHECK (jsonb_typeof(intent) = 'object' AND octet_length(intent::text) <= 2048),
 receipt JSONB NOT NULL CHECK (jsonb_typeof(receipt) = 'object'),
 prior_terminal JSONB NOT NULL CHECK (jsonb_typeof(prior_terminal) = 'object'),
 recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp() CHECK (isfinite(recorded_at))
);
-- Application-immutable, not tamper-proof against owner/superuser/DDL privileges.
CREATE FUNCTION relay.reject_replay_audit_change() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'replay audit is immutable' USING ERRCODE = '42501'; END;
$$;
CREATE TRIGGER replay_audit_immutable BEFORE UPDATE OR DELETE OR TRUNCATE ON relay.replay_requests
 FOR EACH STATEMENT EXECUTE FUNCTION relay.reject_replay_audit_change();
REVOKE ALL ON relay.replay_requests FROM PUBLIC;
