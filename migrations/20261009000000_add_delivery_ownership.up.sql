-- Existing rows become unowned; envelope and historical counters remain intact.
ALTER TABLE relay.outbox_events
    ADD COLUMN ownership_token UUID,
    ADD COLUMN acquired_at TIMESTAMPTZ,
    ADD COLUMN expires_at TIMESTAMPTZ,
    ADD CONSTRAINT ck_outbox_events_ownership CHECK (
        (ownership_token IS NULL AND acquired_at IS NULL AND expires_at IS NULL)
        OR
        (ownership_token IS NOT NULL AND acquired_at IS NOT NULL AND expires_at IS NOT NULL
         AND ownership_token <> '00000000-0000-0000-0000-000000000000'::uuid
         AND status = 'pending' AND attempt_count > 0
         AND available_at <= acquired_at
         AND isfinite(acquired_at) AND isfinite(expires_at)
         AND expires_at > acquired_at)
    );
-- Keep pending availability index for due-row scan; expiry is a residual predicate.
