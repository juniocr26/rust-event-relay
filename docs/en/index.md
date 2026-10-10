# Documentation catalog: rust-event-relay

[Project introduction](../../README.md) | [Other language](../pt-BR/index.md)

Reading path: purpose/setup → architecture/contracts → security/failures → testing/evidence → operations. For the library, use catalog/dossier → project interview → foundations → failure follow-ups. Historical milestone/refactor/handoff records describe their original dates; current review/checklist explains present scope.

## architecture

Components, flows and implemented boundaries.

- [Failure and transaction semantics - Milestone 1.8](architecture/failure-and-transactions.md)
- [Milestone 2.1 — RabbitMQ publisher and process management](architecture/milestone-2-1.md)
- [Milestone 2.2 — Delivery state and event ownership](architecture/milestone-2-2.md)
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

Diagnosis, recovery, review checklists and historical records.

- [rust-event-relay: Repository completion checklist](operations/completion-checklist.md)
- [Deployment, observability and evidence gaps](operations/deployment-and-evidence.md)
- [Milestone 1 handoff](operations/handoff-marco-1.md)
- [Milestone 1 closure review](operations/milestone-1-review.md)

## security

Authentication, authorization, data and resource boundaries.

- [Authority, diagnostics and transport](security/diagnostic-and-authority-boundaries.md)

Category applicability and omissions are justified in the [completion checklist](operations/completion-checklist.md). No empty category is created.
