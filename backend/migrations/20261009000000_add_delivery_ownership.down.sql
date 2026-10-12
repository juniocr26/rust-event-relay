-- Removes durable authority: never roll back with live workers or leases.
ALTER TABLE relay.outbox_events
    DROP CONSTRAINT ck_outbox_events_ownership,
    DROP COLUMN ownership_token,
    DROP COLUMN acquired_at,
    DROP COLUMN expires_at;
