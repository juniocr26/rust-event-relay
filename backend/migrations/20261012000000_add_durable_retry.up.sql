-- Initial cycle includes historical acquisitions; no implicit budget renewal.
ALTER TABLE relay.outbox_events
 ADD COLUMN retry_cycle INTEGER NOT NULL DEFAULT 1,
 ADD COLUMN cycle_baseline INTEGER NOT NULL DEFAULT 0,
 ADD COLUMN cycle_maximum INTEGER NOT NULL DEFAULT 10,
 ADD COLUMN state_revision BIGINT NOT NULL DEFAULT 0,
 ADD COLUMN diagnostic_code TEXT,
 ADD COLUMN diagnostic_at TIMESTAMPTZ,
 ADD COLUMN failure_evidence JSONB,
 ADD COLUMN terminal_at TIMESTAMPTZ,
 ADD COLUMN terminal_reason TEXT;

-- Preserve legacy terminal status and last_error; do not invent a failure cause.
UPDATE relay.outbox_events SET terminal_at=statement_timestamp(), terminal_reason='legacy_dead_letter'
 WHERE status='dead_letter';

ALTER TABLE relay.outbox_events
 ADD CONSTRAINT ck_outbox_events_retry_history CHECK (
  retry_cycle > 0 AND cycle_baseline >= 0 AND cycle_baseline <= attempt_count
  AND cycle_maximum > 0 AND state_revision >= 0),
 ADD CONSTRAINT ck_outbox_events_diagnostic CHECK ((
  (diagnostic_code IS NULL AND diagnostic_at IS NULL AND failure_evidence IS NULL)
  OR (diagnostic_code IS NOT NULL AND diagnostic_at IS NOT NULL AND failure_evidence IS NOT NULL
   AND isfinite(diagnostic_at)
   AND diagnostic_code IN ('unroutable','rejected','serialization_retryable','serialization_permanent',
     'invalid_stored_envelope','acquisition_budget_exhausted','storage_counter_exhausted')
   AND jsonb_typeof(failure_evidence)='object'
   AND octet_length(failure_evidence::text) <= 2048
   AND failure_evidence ?& ARRAY['policy_version','phase']
   AND (failure_evidence - ARRAY['policy_version','phase','validator_version','evidence_reference'])='{}'::jsonb
   AND failure_evidence->>'policy_version'='adr008-v1'
   AND failure_evidence->>'phase' IN ('before_send','broker_response','maintenance')
   AND (NOT failure_evidence ? 'validator_version' OR failure_evidence->>'validator_version'='envelope-v1')
   AND (NOT failure_evidence ? 'evidence_reference' OR failure_evidence->>'evidence_reference' ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$' AND failure_evidence->>'evidence_reference' <> '00000000-0000-0000-0000-000000000000'))) IS TRUE),
 ADD CONSTRAINT ck_outbox_events_terminal CHECK (
  (status <> 'dead_letter' AND terminal_at IS NULL AND terminal_reason IS NULL)
  OR (status='dead_letter' AND terminal_at IS NOT NULL AND isfinite(terminal_at)
   AND processed_at IS NULL AND ownership_token IS NULL AND terminal_reason IS NOT NULL
   AND (terminal_reason='legacy_dead_letter'
     OR (terminal_reason IN ('serialization_permanent','invalid_stored_envelope',
       'acquisition_budget_exhausted','storage_counter_exhausted')
       AND diagnostic_code IS NOT NULL AND terminal_reason=diagnostic_code AND terminal_at=diagnostic_at
       AND (terminal_reason <> 'acquisition_budget_exhausted' OR attempt_count-cycle_baseline >= cycle_maximum)
       AND (terminal_reason <> 'storage_counter_exhausted' OR attempt_count=2147483647)))));
-- Existing pending-availability index supports due exhausted maintenance as well.
