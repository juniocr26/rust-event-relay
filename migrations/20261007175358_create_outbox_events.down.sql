-- Destructive once real events exist. Keep the schema owned by the earlier migration.
-- Dropping the table also removes its own constraints and indexes; no CASCADE.
DROP TABLE relay.outbox_events RESTRICT;
