[Português brasileiro](../pt-BR/outbox-schema.md) | [README](../../README.md)


## Current extension — Milestone 2.2

[Delivery state and ownership](milestone-2-2.md) and [ADR 007](adr/007-durable-delivery-ownership.md) now define durable lease recovery and separate acquisition/completion/release contracts. New migration `20261009000000_add_delivery_ownership` adds nullable token/acquired_at/expires_at with coherent pending-only leases. Earlier milestone sections below describe their original scope; earlier claims that ownership/attempt semantics are undecided are superseded by ADR 007. Production mutation adapters remain deferred to 2.3; reader SELECT and publisher behavior remain unchanged.
# Outbox schema — Milestone 1.4

## Purpose and ownership

`relay.outbox_events` is the initial durable relational representation of EventEnvelope plus minimal relay metadata. This milestone defines schema only: the Rust binary neither inserts nor processes events. Producers own event identity and should eventually write business state and the outbox event in the **same local PostgreSQL transaction**. The relay will later read and deliver durable rows. No database-generated ID or occurrence timestamp silently replaces the producer's values.

The classic dual write can lose an event if business state commits and the producer crashes before publishing; publishing first can expose an event whose business transaction later fails. Recording both durable changes locally in one transaction removes that handoff gap. Delivery still has publication/acknowledgement crash windows and possible duplicates; future at-least-once processing requires consumer idempotency. The schema alone implements no such transaction, claim or guarantee.

```text
BEGIN
  update business state
  insert relay.outbox_events (...)
COMMIT
```

## Columns and envelope mapping

| Column | PostgreSQL type | Nullable | Default | Meaning |
| --- | --- | --- | --- | --- |
| id | UUID | No | None | Producer-generated event identity; primary key |
| event_type | TEXT | No | None | Application-specific logical event name |
| aggregate_type | TEXT | No | None | Generic originating aggregate type |
| aggregate_id | TEXT | No | None | Generic identifier; UUID, integer, ULID, external or legacy string |
| schema_version | BIGINT | No | None | Full NonZeroU32 range, 1..4294967295 |
| occurred_at | TIMESTAMPTZ | No | None | Business event occurrence instant |
| correlation_id | UUID | Yes | NULL | Optional correlation metadata |
| causation_id | UUID | Yes | NULL | Optional causal event/command identity |
| payload | JSONB | No | None | Event-specific JSON value, including JSON null |
| created_at | TIMESTAMPTZ | No | now() | Outbox insertion transaction timestamp |
| status | TEXT | No | pending | Relay lifecycle state |
| attempt_count | INTEGER | No | 0 | Non-negative infrastructure attempt counter |
| available_at | TIMESTAMPTZ | No | now() | Earliest eligibility time for future processing |
| processed_at | TIMESTAMPTZ | Yes | NULL | Successful relay completion time |
| last_error | TEXT | Yes | NULL | Future sanitized operational diagnostic |

The first nine columns are the canonical event. The remaining six are infrastructure metadata. Mapping: Uuid → UUID; EventType/String → TEXT; NonZeroU32 → BIGINT with bounds; DateTime<Utc> → TIMESTAMPTZ; Value → JSONB; Option<Uuid> → nullable UUID. The Rust envelope remains unchanged.

**BIGINT is deliberate:** PostgreSQL INTEGER ends at 2147483647 and cannot store every valid u32. The positive/bounded check preserves 1..4294967295 instead of silently narrowing the model. Future repositories must convert the signed database value with bounds checking. See [integer ranges](https://www.postgresql.org/docs/18/datatype-numeric.html).

Event names remain flexible (order.created, payment.completed, inventory.reserved); no PostgreSQL enum or business catalog migration is needed. Aggregate identifiers have no UUID restriction or arbitrary length cap. Correlation/causation UUIDs match current IDs, but external systems may eventually require non-UUID tokens; that documented trade-off remains unchanged.

`occurred_at` is business time, `created_at` is database insertion time, and `processed_at` is successful relay completion. `now()` is transaction-start time: both created_at/available_at defaults match within the inserting transaction, not a per-row wall-clock sampling guarantee. TIMESTAMPTZ preserves instants rather than the original textual zone; output follows session timezone. PostgreSQL microsecond precision cannot retain arbitrary Chrono nanoseconds. Future persistence must make precision/finite-time handling explicit. See [date/time types](https://www.postgresql.org/docs/18/datatype-datetime.html).

JSONB validates JSON syntax and allows objects, arrays, scalars and JSON null; SQL NULL is rejected. It offers queries and future indexing, with weaker relational typing and a risk of infrastructure coupling to business payload layouts. It does not validate a business payload schema. JSONB normalizes formatting/key order and does not preserve duplicate keys; PostgreSQL text/JSONB restrictions such as embedded zero characters mean not every possible Rust string is storable unchanged. Future persistence must surface such errors. No speculative GIN/expression payload index exists. See [JSON types](https://www.postgresql.org/docs/18/datatype-json.html).

## Durable invariants

| Invariant | Rust | PostgreSQL |
| --- | --- | --- |
| Event/aggregate strings non-empty | Yes; also rejects Unicode whitespace-only | Named checks reject exactly empty strings |
| Positive schema version within u32 | NonZeroU32 | BIGINT range check |
| Required envelope fields | Typed construction/restoration | NOT NULL |
| Optional UUID metadata | Option<Uuid> | Nullable UUID |
| Unique persisted event identity | Generates UUID v7; no global uniqueness registry | UUID primary key |
| Non-negative attempts and valid lifecycle | No relay behavior yet | Named checks |
| Completion timestamp iff processed | No relay behavior yet | Named check |

Checks are `ck_outbox_events_event_type_non_empty`, `ck_outbox_events_aggregate_type_non_empty`, `ck_outbox_events_aggregate_id_non_empty`, `ck_outbox_events_schema_version_positive`, `ck_outbox_events_attempt_count_non_negative`, `ck_outbox_events_status` and `ck_outbox_events_processed_at`. The primary key is `pk_outbox_events`. SQL whitespace-only strings are intentionally left to producer/application validation; matching Rust's complete Unicode trim rules in SQL is unnecessary complexity. The database does not rewrite identifiers.

## Lifecycle and future claims

`pending` covers initial work and scheduled retries; `available_at <= now()` describes eligibility, not an implemented query. `processed` records successful completion and requires processed_at. `dead_letter` represents terminal failure without a separate DLQ table. Both pending and dead_letter require processed_at NULL. attempt_count and last_error support future retries and inspection; no code increments or populates them yet. Diagnostics should be sanitized, avoiding sensitive payload copies or unnecessary stack traces.

There is no separate failed state: retryable failure remains pending with future availability, and exhausted/non-retryable work may eventually become dead_letter. There is no processing state: a status flip alone cannot recover a crashed worker. Future workers may coordinate transactional row locks with FOR UPDATE SKIP LOCKED; a claim spanning remote I/O/transaction boundaries would need explicit ownership, expiry/recovery and possibly another migration. No claim SQL, lease columns, worker loop or state-transition enforcement is implemented. These checks validate row shape, not the legal sequence of transitions or attempt increments.

## Indexes and ordering

- `pk_outbox_events`: unique B-tree on id for identity lookup and duplicate prevention. Producer UUID v7 may improve locality relative to fully random IDs, but is not strict business ordering and cannot replace occurred_at.
- `ix_outbox_events_pending_available`: partial B-tree `(available_at, created_at, id) WHERE status = 'pending'`. It supports the intended pending eligibility cutoff and ordered, limited polling with deterministic ties; terminal rows are excluded. It does not itself claim work or guarantee scheduling fairness.

Both indexes add storage and write amplification; status changes maintain partial-index membership. No aggregate ordering, occurrence, correlation or payload indexes are added speculatively. Add aggregate/key ordering indexes only after worker semantics and measured access patterns justify them. No throughput or production-readiness claim is made.

## Migration and rollback

New immutable-history pair:

```text
20261007175358_create_outbox_events.up.sql
20261007175358_create_outbox_events.down.sql
```

Up creates the table, checks and one polling index in the existing relay namespace. Down drops only the table with RESTRICT; its own indexes/constraints disappear with it. It preserves relay and earlier SQLx history and refuses dependent external objects. SQLx runs PostgreSQL migrations transactionally by default. Rollback permanently loses stored events once data exists: inspect/back up first; forward correction may be safer. It is not a database reset.

See [schema validation and commands](testing.md#outbox-schema-validation--milestone-14), [actual results](validation-results.md), [migration ownership](database-migrations.md) and [ADR 004](adr/004-use-postgresql-transactional-outbox-schema.md). No routing/destination columns, repository, publisher, retry execution, DLQ table or worker implementation exists.
