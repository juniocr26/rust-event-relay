-- Producer-owned envelope identity; relay metadata has separate defaults.
CREATE TABLE relay.outbox_events (
    id UUID NOT NULL,
    event_type TEXT NOT NULL,
    aggregate_type TEXT NOT NULL,
    aggregate_id TEXT NOT NULL,
    -- BIGINT preserves the full Rust NonZeroU32 range.
    schema_version BIGINT NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL,
    correlation_id UUID,
    causation_id UUID,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    status TEXT NOT NULL DEFAULT 'pending',
    attempt_count INTEGER NOT NULL DEFAULT 0,
    available_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    processed_at TIMESTAMPTZ,
    last_error TEXT,

    CONSTRAINT pk_outbox_events PRIMARY KEY (id),
    CONSTRAINT ck_outbox_events_event_type_non_empty CHECK (event_type <> ''),
    CONSTRAINT ck_outbox_events_aggregate_type_non_empty CHECK (aggregate_type <> ''),
    CONSTRAINT ck_outbox_events_aggregate_id_non_empty CHECK (aggregate_id <> ''),
    CONSTRAINT ck_outbox_events_schema_version_positive
        CHECK (schema_version BETWEEN 1 AND 4294967295),
    CONSTRAINT ck_outbox_events_attempt_count_non_negative CHECK (attempt_count >= 0),
    CONSTRAINT ck_outbox_events_status
        CHECK (status IN ('pending', 'processed', 'dead_letter')),
    CONSTRAINT ck_outbox_events_processed_at
        CHECK ((status = 'processed') = (processed_at IS NOT NULL))
);

-- Eligibility cutoff plus stable polling order; terminal rows are excluded.
CREATE INDEX ix_outbox_events_pending_available
    ON relay.outbox_events (available_at, created_at, id)
    WHERE status = 'pending';
