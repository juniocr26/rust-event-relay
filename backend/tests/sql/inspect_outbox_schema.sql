-- Read-only catalog inspection for Milestone 1.4.
SELECT column_name, data_type, is_nullable, column_default
FROM information_schema.columns
WHERE table_schema = 'relay' AND table_name = 'outbox_events'
ORDER BY ordinal_position;

SELECT conname, pg_get_constraintdef(oid)
FROM pg_constraint
WHERE conrelid = 'relay.outbox_events'::regclass
ORDER BY conname;

SELECT indexname, indexdef
FROM pg_indexes
WHERE schemaname = 'relay' AND tablename = 'outbox_events'
ORDER BY indexname;

SELECT version, description, success
FROM public._sqlx_migrations
ORDER BY version;

SELECT count(*) AS outbox_rows FROM relay.outbox_events;
