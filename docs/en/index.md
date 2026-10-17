[Milestone 6 preparation — reference comparison and proposed live scope (not approved)](operations/milestone-6-preparation.md).

**Current Milestone 5 closure (2026-10-17):** [5.3 protocol, measurements, acceptance matrix, rollout and decision list](operations/milestone-5-3.md) | [Bounded result data](testing/milestone-5-3-results.json). Closed with explicit nonblocking limitations; Milestone 6 undefined. Earlier dated entries remain historical.

# Documentation catalog: rust-event-relay

[Project introduction](../../README.md) | [Other language](../pt-BR/index.md)

Reading path: purpose/setup → architecture/contracts → security/failures → testing/evidence → operations. For the library, use catalog/dossier → project interview → foundations → failure follow-ups. Historical milestone/refactor/handoff records describe their original dates; current review/checklist explains present scope.

- [Milestone 5.2 — readiness, freshness and validated 5.1 rollout](operations/milestone-5-2.md)

## Current resource and observability contract

- [Milestone 5.1 — resource boundaries and process-local metrics](operations/milestone-5-1.md)

## Current local rollout

- [Milestone 4.5 — controlled rollout, verified backup, retained demonstration and recovery](operations/milestone-4-5-rollout.md)

## Current Milestone 4 closure

- [Milestone 4.4 — verification, acceptance matrix and rollout](operations/milestone-4-4-review.md)
- [Environment inventory and effective configuration](docker/environment-inventory.md)


## architecture

- [Milestone 2.6 — Integrated recovery](architecture/milestone-2-6.md)

- [Milestone 2.5 — Worker](architecture/milestone-2-5.md)

- [Milestone 2.4 — Delivery use case](architecture/milestone-2-4.md)

Components, flows and implemented boundaries.

- [Failure and transaction semantics - Milestone 1.8](architecture/failure-and-transactions.md)
- [Milestone 2.1 — RabbitMQ publisher and process management](architecture/milestone-2-1.md)
- [Milestone 2.2 — Delivery state and event ownership](architecture/milestone-2-2.md)
- [Milestone 2.3 — PostgreSQL ownership operations](architecture/milestone-2-3.md)
- [Architecture](architecture/overview.md)
- [Persistence abstraction — Milestone 1.5](architecture/persistence-contracts.md)

## adr

Recorded decisions and consequences; identifiers/history preserved.

- [ADR 001 — Use Rust for the relay](adr/001-use-rust-for-the-relay.md)
- [ADR 002 — Use PostgreSQL for durable event storage](adr/002-use-postgresql-for-durable-event-storage.md)
- [ADR 003 — Use Versioned SQL Migrations for PostgreSQL Schema Evolution](adr/003-use-versioned-sql-migrations.md)
- [ADR 004 — Use a PostgreSQL Transactional Outbox Table for Durable Event Handoff](adr/004-use-postgresql-transactional-outbox-schema.md)
- [ADR 005 — Separate Relay Persistence Contracts from PostgreSQL Infrastructure](adr/005-separate-persistence-contracts-from-postgresql.md)
- [ADR 006 — Confirmed RabbitMQ publisher and local Supervisor](adr/006-rabbitmq-publisher-and-supervisor.md)
- [ADR 007 — Durable delivery ownership with short transactions](adr/007-durable-delivery-ownership.md)

## guides

- [Frontend/backend preparation](guides/frontend-backend.md)

Setup, prerequisites and reading procedures.

- [Development dependencies and recovery](guides/dependencies.md)
- [Project guide](guides/project-guide.md)

## testing

Test strategy, inventories and dated evidence.

- [Testing](testing/strategy.md)
- [Validation results](testing/validation-results.md)

## docker

Local images, services, mounts and configuration.

- [Docker and configuration](docker/configuration.md)

## database

Models, constraints, migrations and consistency.

- [Database migrations — Milestone 1.3](database/migrations.md)
- [Outbox schema — Milestone 1.4](database/outbox-schema.md)
- [PostgreSQL repository — Milestone 1.6](database/postgres-repository.md)
- [PostgreSQL decision and local infrastructure](database/postgresql.md)

## integrations

Verified boundaries and explicit non-integrations.

- [Verified publisher integration](integrations/rabbitmq-contract.md)

## operations

- [Milestone 2 acceptance review](operations/milestone-2-review.md)

Diagnosis, recovery, review checklists and historical records.

- [rust-event-relay: Repository completion checklist](operations/completion-checklist.md)
- [Deployment, observability and evidence gaps](operations/deployment-and-evidence.md)
- [Milestone 1 handoff](operations/handoff-marco-1.md)
- [Milestone 1 closure review](operations/milestone-1-review.md)

## security

Authentication, authorization, data and resource boundaries.

- [Authority, diagnostics and transport](security/diagnostic-and-authority-boundaries.md)

Category applicability and omissions are justified in the [completion checklist](operations/completion-checklist.md). No empty category is created.

- [Milestone 3.1 — durable failure policy and roadmap](architecture/milestone-3-1.md)
- [ADR 008 — durable failure policy](adr/008-durable-failure-policy.md)

- [Milestone 3.2 — current persistence scope](architecture/milestone-3-2.md)

[Milestone 3.3 — current integration](architecture/milestone-3-3.md).

[Local relay rollout and automatic startup](operations/local-relay-rollout.md).


## Milestone 3.4 — current discovery/isolation

Bounded discovery and fresh-authority isolation are implemented; replay remains 3.5 and integrated closure 3.6. Current local rollout is preserved; this change has no migration or shared restart. [Milestone 3.4 — current discovery/isolation](architecture/milestone-3-4.md).

[Milestone 3.5 — replay](architecture/milestone-3-5.md) | [ADR 009](adr/009-authorized-idempotent-replay.md) | [Operator replay](operations/replay.md)


## Current Milestone 3.6 acceptance — 2026-10-13

[Full-project audit, traceable acceptance matrix, actual validation counts, operational gaps and study path](operations/milestone-3-review.md). Milestone 3 closed with explicit nonblocking limitations; shared replay rollout, consumer idempotency, live frontend integration and production readiness remain separate. Earlier sections retain their dated historical scope.


## Milestone 4.1 — contracts implemented; runtime planned

[Inventory, deduplication, quarantine and 4.2–4.4](architecture/milestone-4-1.md) | [ADR 010](adr/010-atomic-consumer-and-quarantine.md).

[Milestone 4.2 — PostgreSQL](architecture/milestone-4-2.md).

[Milestone 4.3 — consumer runtime and confirmed quarantine](architecture/milestone-4-3.md).

- [Milestone 6.1 — persistent real delivery](operations/milestone-6-1.md)
- [ADR 011 — persistent demo runs](adr/011-persistent-demo-runs.md)

- [Milestone 6.2 — durable duplicates](operations/milestone-6-2.md)
