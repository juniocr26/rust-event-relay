-- Schema-only fixtures: every inserted row is rolled back, including on failure.
-- Run with psql -X -v ON_ERROR_STOP=1 (see testing.md).
BEGIN;
SET LOCAL TIME ZONE 'UTC';

DO $$
DECLARE
    fixture relay.outbox_events%ROWTYPE;
    candidate JSONB;
    field TEXT;
    violated_constraint TEXT;
    tests_run INTEGER := 0;
BEGIN
    INSERT INTO relay.outbox_events (
        id, event_type, aggregate_type, aggregate_id, schema_version, occurred_at, payload
    ) VALUES (
        '0199ba40-0000-7000-8000-000000000001', 'order.created', 'order',
        'external-order-42', 1, '2026-10-07T12:00:00Z', '{"example":true}'
    ) RETURNING * INTO fixture;

    IF fixture.status <> 'pending' OR fixture.attempt_count <> 0
        OR fixture.created_at <> now() OR fixture.available_at <> fixture.created_at
        OR fixture.processed_at IS NOT NULL OR fixture.last_error IS NOT NULL
        OR fixture.correlation_id IS NOT NULL OR fixture.causation_id IS NOT NULL
        OR fixture.occurred_at <> '2026-10-07T12:00:00Z'::timestamptz
        OR pg_typeof(fixture.payload) <> 'jsonb'::regtype
        OR fixture.payload <> '{"example":true}'::jsonb THEN
        RAISE EXCEPTION 'Envelope storage or lifecycle defaults failed';
    END IF;
    tests_run := tests_run + 1;

    -- Native UUID primary key retains identity; duplicates must fail.
    BEGIN
        INSERT INTO relay.outbox_events SELECT fixture.*;
        RAISE EXCEPTION 'Duplicate ID was accepted';
    EXCEPTION WHEN unique_violation THEN
        GET STACKED DIAGNOSTICS violated_constraint = CONSTRAINT_NAME;
        IF violated_constraint <> 'pk_outbox_events' THEN RAISE; END IF;
    END;
    tests_run := tests_run + 1;

    -- Required fields, including metadata with defaults, must reject explicit NULL.
    FOREACH field IN ARRAY ARRAY[
        'id', 'event_type', 'aggregate_type', 'aggregate_id', 'schema_version',
        'occurred_at', 'payload', 'created_at', 'status', 'attempt_count', 'available_at'
    ] LOOP
        candidate := jsonb_set(to_jsonb(fixture), '{id}', '"0199ba40-0000-7000-8000-000000000002"');
        candidate := jsonb_set(candidate, ARRAY[field], 'null');
        BEGIN
            INSERT INTO relay.outbox_events
                SELECT * FROM jsonb_populate_record(NULL::relay.outbox_events, candidate);
            RAISE EXCEPTION 'NULL accepted for %', field;
        EXCEPTION WHEN not_null_violation THEN
            NULL;
        END;
        tests_run := tests_run + 1;
    END LOOP;

    -- Invalid constrained values must fail the intended named invariant.
    FOR field, candidate, violated_constraint IN
        SELECT * FROM (VALUES
            ('event_type', '""'::jsonb, 'ck_outbox_events_event_type_non_empty'),
            ('aggregate_type', '""'::jsonb, 'ck_outbox_events_aggregate_type_non_empty'),
            ('aggregate_id', '""'::jsonb, 'ck_outbox_events_aggregate_id_non_empty'),
            ('schema_version', '0'::jsonb, 'ck_outbox_events_schema_version_positive'),
            ('schema_version', '-1'::jsonb, 'ck_outbox_events_schema_version_positive'),
            ('schema_version', '4294967296'::jsonb, 'ck_outbox_events_schema_version_positive'),
            ('attempt_count', '-1'::jsonb, 'ck_outbox_events_attempt_count_non_negative'),
            ('status', '"processing"'::jsonb, 'ck_outbox_events_status'),
            ('status', '"processed"'::jsonb, 'ck_outbox_events_processed_at'),
            ('processed_at', '"2026-10-07T12:01:00Z"'::jsonb, 'ck_outbox_events_processed_at')
        ) AS invalid(field, value, expected_constraint)
    LOOP
        DECLARE
            actual_constraint TEXT;
        BEGIN
            candidate := jsonb_set(to_jsonb(fixture), ARRAY[field], candidate);
            candidate := jsonb_set(candidate, '{id}', '"0199ba40-0000-7000-8000-000000000002"');
            BEGIN
                INSERT INTO relay.outbox_events
                    SELECT * FROM jsonb_populate_record(NULL::relay.outbox_events, candidate);
                RAISE EXCEPTION 'Invalid value accepted for %', field;
            EXCEPTION WHEN check_violation THEN
                GET STACKED DIAGNOSTICS actual_constraint = CONSTRAINT_NAME;
                IF actual_constraint <> violated_constraint THEN RAISE; END IF;
            END;
        END;
        tests_run := tests_run + 1;
    END LOOP;

    -- Full u32 range, optional UUID metadata, JSON null (not SQL NULL),
    -- and PostgreSQL timestamp offset normalization remain representable.
    INSERT INTO relay.outbox_events (
        id, event_type, aggregate_type, aggregate_id, schema_version, occurred_at,
        correlation_id, causation_id, payload, status, processed_at
    ) VALUES (
        '0199ba40-0000-7000-8000-000000000002', 'payment.completed', 'payment',
        '42', 4294967295, '2026-10-07T09:00:00-03:00',
        '0199ba40-0000-7000-8000-000000000003',
        '0199ba40-0000-7000-8000-000000000004', 'null'::jsonb, 'processed', now()
    ) RETURNING * INTO fixture;
    IF fixture.schema_version <> 4294967295 OR fixture.payload <> 'null'::jsonb
        OR fixture.occurred_at <> '2026-10-07T12:00:00Z'::timestamptz
        OR fixture.correlation_id IS NULL OR fixture.causation_id IS NULL
        OR fixture.processed_at IS NULL THEN
        RAISE EXCEPTION 'Extended envelope representation failed';
    END IF;
    tests_run := tests_run + 1;

    -- Retry and terminal failure are representable; no worker/retry behavior runs.
    INSERT INTO relay.outbox_events (
        id, event_type, aggregate_type, aggregate_id, schema_version, occurred_at,
        payload, status, attempt_count, available_at, last_error
    ) VALUES (
        '0199ba40-0000-7000-8000-000000000005', 'inventory.reserved', 'inventory',
        'legacy-key', 1, now(), '[]'::jsonb, 'pending', 2,
        now() + interval '1 minute', 'synthetic validation error'
    );
    UPDATE relay.outbox_events SET status = 'dead_letter'
        WHERE id = '0199ba40-0000-7000-8000-000000000005';
    tests_run := tests_run + 1;

    RAISE NOTICE 'Outbox schema assertions passed: % cases; fixture transaction will roll back', tests_run;
END;
$$;

ROLLBACK;
