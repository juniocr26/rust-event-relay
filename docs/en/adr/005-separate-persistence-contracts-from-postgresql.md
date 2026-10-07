[Português brasileiro](../../pt-BR/adr/005-separate-persistence-contracts-from-postgresql.md) | [README](../../../README.md)

# ADR 005 — Separate Relay Persistence Contracts from PostgreSQL Infrastructure

## Status

Accepted — 2026-10-07, Milestone 1.5.

## Context

The envelope, durable outbox schema and migrations exist. Future application code needs explicit persistence semantics without SQLx/row types. The standalone relay does not own producers' business transactions. The schema has pending/processed/dead_letter but no durable processing/lease ownership, so retrieval cannot be advertised as a claim.

## Decision

Add a focused read-only OutboxReader contract with positive BatchSize, explicit UTC EligibleRead and PendingOutboxEvent snapshots. Keep event identity separate from unsigned attempt/UTC availability metadata. Reads are bounded, side-effect free and consistent snapshots, providing no exclusivity or business ordering. Defer mutation/claim contracts until authority, concurrency, atomicity and repetition semantics are resolved.

Do not expose a producer writer: standalone append cannot be atomic with another application's transaction. Do not expose unused terminal types or duplicate/conflict errors. Preserve diagnostic sources behind three classified errors with sanitized Display/Debug. Use standard-library native impl Future + Send and generic/static dispatch, avoiding async-trait, thiserror, driver and boxed-future dependencies. Future PostgreSQL adapters depend inward on this port. No new migration or runtime wiring.

## Alternatives considered

- Direct SQLx from application logic: fewer types, but mixes driver mapping, lifecycle and application concerns.
- Generic CRUD repository: superficially reusable but obscures eligibility, claims and producer transactional limits.
- PostgreSQL-specific API: exposes locking/transaction features, but ties caller contracts to driver types and does not resolve ownership semantics by itself.
- Full writer/claim/lifecycle port now: freezes unresolved transaction and recovery decisions, especially unsafe ID-only acknowledgement.
- Focused application-level contract: precise useful observation now, with deliberate extension after adapter evidence.

## Consequences

New tests can use a one-response scripted fake through generic async code without database access. This demonstrates substitution and Send compatibility, not durability/locking conformance. Runtime still does not use the reader. Producer duplicates, claim recovery and mutation idempotency remain open; source inspection requires sensitive-data redaction. Future limits must be configured by callers, and adapters must perform checked conversion and faithful restoration.

## Trade-offs

Abstraction adds interface/types; public Send/static dispatch choices affect future compatibility and rule out dyn without redesign. Hiding driver features risks hiding semantics, addressed through explicit guarantees and exclusions. The model is a pending view rather than every SQL column; future views/authority tokens can evolve when needed. Other adapters are possible but portability/performance are not goals or validated claims. Producer atomicity cannot be manufactured by a standalone relay API.

See [contract semantics, models, open questions and diagrams](../persistence-abstraction.md).
