[Português brasileiro](../../pt-BR/adr/004-use-postgresql-transactional-outbox-schema.md) | [README](../../../README.md)

# ADR 004 — Use a PostgreSQL Transactional Outbox Table for Durable Event Handoff

## Status

Accepted — 2026-10-07, Milestone 1.4 (schema only).

## Context

EventEnvelope and migration infrastructure exist. Producers eventually need a durable event handoff that participates atomically in their local business-state transaction. A database commit and remote publish are independent operations with failure gaps. Relay delivery is future work.

## Decision

Introduce relay.outbox_events through a new reversible SQLx migration, preserving earlier files. Store producer identity/metadata with UUID, TEXT, TIMESTAMPTZ, JSONB and nullable UUIDs. Use bounded BIGINT for schema_version to preserve all NonZeroU32 values. Add pending/processed/dead_letter TEXT with checks, attempts, availability, completion and sanitized error metadata. No processing state or durable lease without a recovery protocol. Use the identity primary key and one pending-availability partial index. Down drops only the table with RESTRICT, preserving the namespace.

Producers own the future same-database transaction containing business change plus outbox insertion; the relay delivers durable rows later. The Rust application remains unchanged and does not write/read this table.

## Alternatives considered

- Publish after commit: a crash before publish loses the notification.
- Publish before commit: a failed database transaction leaves an event for nonexistent business state.
- Distributed transaction / 2PC: coordination/availability and participant support complicate operations; unnecessary for this study scope.
- CDC/logical replication: can observe committed changes but adds replication/operational infrastructure and event mapping contracts; may be explored later.
- Durable transactional outbox: explicit source-controlled handoff in one local transaction, at the cost of row lifecycle and later delivery/recovery work.

## Consequences

The schema is transport-neutral and producer-facing; repositories, destinations and workers remain future milestones. Immutable migrations own schema evolution. Minimal lifecycle fields make retries/terminal failures representable without executing them. Pending claims may later use PostgreSQL transactional locks; cross-transaction leases require explicit recovery design. Named constraints protect durable row invariants but do not enforce transition history.

## Trade-offs

JSONB weakens business-field relational typing and normalizes representation; time storage has precision/zone limits. UUID metadata cannot represent arbitrary external correlation tokens. BIGINT uses more space than INTEGER but preserves the Rust range. Indexes add storage/write cost; no speculative aggregate/payload indexes. Outbox reduces local dual-write inconsistency, not exactly-once delivery: remote publication may duplicate, requiring future idempotency and crash tests. Down is destructive once data exists; this is not production recovery sophistication or a performance claim.

See [full schema and limitations](../database/outbox-schema.md).
