[Português brasileiro](../../pt-BR/adr/002-use-postgresql-for-durable-event-storage.md) | [README](../../../README.md)

# ADR 002 — Use PostgreSQL for durable event storage

Status: accepted. Date: 2026-10-07.

## Context

The intended relay needs durable event records and a producer transaction boundary that avoids unsafe database/message dual writes. Today only the canonical envelope and application foundation exist; this milestone establishes local infrastructure.

## Decision

Select PostgreSQL for future durable event storage and the transactional outbox. Start local development with `postgres:18.6-bookworm`, loopback host access on 5433, internal port 5432 and a physical `.dockerized-postgres/` bind mount. Use a server healthcheck without adding application persistence. See the [detailed decision](../postgresql.md).

## Alternatives considered

MySQL/MariaDB also offer relational transactions; PostgreSQL's concurrency, JSONB and tooling fit the intended exploration. SQLite simplifies deployment but its single-writer model is less aligned with the intended concurrent networked workers. Document databases offer different transaction and modeling boundaries. Broker-only durability does not by itself make a producer database change atomic with publication. In-memory storage cannot survive process loss. No comparative benchmarks were performed and PostgreSQL is not universally superior.

## Consequences

The future producer must write business state and outbox in the same local transaction to obtain atomicity. Relay delivery remains asynchronous with duplicate risks. Local files survive container recreation and need deliberate reset; production requires backups, monitoring and a separate deployment design. Migrations, tables, repositories and worker claims are deferred.

## Trade-offs

Stateful operations, bounded vertical capacity, write/worker contention, growing outbox retention and vacuum costs must be managed. PostgreSQL-specific locking may reduce portability; portability is not a current goal. SKIP LOCKED may aid concurrent claims but cannot promise fairness or global order. These are future design considerations, not implemented behavior.
