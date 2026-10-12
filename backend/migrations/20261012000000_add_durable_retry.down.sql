-- Isolated rollback only: loses cycle, revision, diagnostics and terminal audit.
-- Retains envelope, lifetime acquisitions, status, last_error and availability.
ALTER TABLE relay.outbox_events
 DROP CONSTRAINT ck_outbox_events_terminal,
 DROP CONSTRAINT ck_outbox_events_diagnostic,
 DROP CONSTRAINT ck_outbox_events_retry_history,
 DROP COLUMN retry_cycle,
 DROP COLUMN cycle_baseline,
 DROP COLUMN cycle_maximum,
 DROP COLUMN state_revision,
 DROP COLUMN diagnostic_code,
 DROP COLUMN diagnostic_at,
 DROP COLUMN failure_evidence,
 DROP COLUMN terminal_at,
 DROP COLUMN terminal_reason;
