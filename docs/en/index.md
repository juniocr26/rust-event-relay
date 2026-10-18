# Documentation catalog: Rust Event Relay

[README](../../README.md) | [English / Português brasileiro](../pt-BR/index.md)

Start with [installation](../../README.md), then [trace one event](guides/implementation-study.md), [current operation](operations/application-scenarios.md), [test strategy](testing/strategy.md) and the interview practice in the study guide. Final acceptance is [dated evidence](operations/final-audit.md), not a new execution. Milestone pages below are historical records; “current” in their titles refers to their recorded milestone.

## adr

- [ADR 001 — Use Rust for the relay](adr/001-use-rust-for-the-relay.md)
- [ADR 002 — Use PostgreSQL for durable event storage](adr/002-use-postgresql-for-durable-event-storage.md)
- [ADR 003 — Use Versioned SQL Migrations for PostgreSQL Schema Evolution](adr/003-use-versioned-sql-migrations.md)
- [ADR 004 — Use a PostgreSQL Transactional Outbox Table for Durable Event Handoff](adr/004-use-postgresql-transactional-outbox-schema.md)
- [ADR 005 — Separate Relay Persistence Contracts from PostgreSQL Infrastructure](adr/005-separate-persistence-contracts-from-postgresql.md)
- [ADR 006 — Confirmed RabbitMQ publisher and local Supervisor](adr/006-rabbitmq-publisher-and-supervisor.md)
- [ADR 007 — Durable delivery ownership with short transactions](adr/007-durable-delivery-ownership.md)
- [ADR 008 — Durable failure policy and recovery contracts](adr/008-durable-failure-policy.md)
- [ADR 009 — Authorized, idempotent and auditable replay](adr/009-authorized-idempotent-replay.md)
- [ADR 010 — Atomic inventory effect and application-managed quarantine](adr/010-atomic-consumer-and-quarantine.md)
- [ADR 011: persistent local successful-delivery runs](adr/011-persistent-demo-runs.md)
- [ADR 012 — one application environment and bounded real scenarios](adr/012-one-application-real-scenarios.md)

## architecture

- [Failure and transaction semantics - Milestone 1.8](architecture/failure-and-transactions.md)
- [Milestone 2.1 — RabbitMQ publisher and process management](architecture/milestone-2-1.md)
- [Milestone 2.2 — Delivery state and event ownership](architecture/milestone-2-2.md)
- [Milestone 2.3 — PostgreSQL delivery ownership operations](architecture/milestone-2-3.md)
- [Milestone 2.4 — Delivery use-case orchestration](architecture/milestone-2-4.md)
- [Milestone 2.5 — Worker composition and operation](architecture/milestone-2-5.md)
- [Milestone 2.6 — Integrated recovery experiment plan](architecture/milestone-2-6.md)
- [Milestone 3.1 — Durable failure policy and recovery contracts](architecture/milestone-3-1.md)
- [Milestone 3.2 — Durable retry state and PostgreSQL transitions](architecture/milestone-3-2.md)
- [Milestone 3.3 — Durable failure policy integration](architecture/milestone-3-3.md)
- [Milestone 3.4 — Bounded poison discovery and safe isolation](architecture/milestone-3-4.md)
- [Milestone 3.5 — Authorized, idempotent and auditable replay](architecture/milestone-3-5.md)
- [Milestone 4.1 — Inventory consumer contracts](architecture/milestone-4-1.md)
- [Milestone 4.2 — atomic PostgreSQL inventory persistence](architecture/milestone-4-2.md)
- [Milestone 4.3 — RabbitMQ consumer and confirmed quarantine](architecture/milestone-4-3.md)
- [Architecture](architecture/overview.md)
- [Persistence abstraction — Milestone 1.5](architecture/persistence-contracts.md)

## database

- [Database migrations — Milestone 1.3](database/migrations.md)
- [Outbox schema — Milestone 1.4](database/outbox-schema.md)
- [PostgreSQL repository — Milestone 1.6](database/postgres-repository.md)
- [PostgreSQL decision and local infrastructure](database/postgresql.md)

## docker

- [Docker and configuration](docker/configuration.md)
- [Environment inventory — Milestone 4.4](docker/environment-inventory.md)

## guides

- [Development dependencies and recovery](guides/dependencies.md)
- [Frontend/backend preparation — 2026-10-12](guides/frontend-backend.md)
- [Final local implementation: one event, five evidence boundaries](guides/implementation-study.md)
- [Project guide](guides/project-guide.md)

## integrations

- [rabbitmq-contract](integrations/rabbitmq-contract.md)

## operations

- [Startup and token verification](operations/application-scenarios.md)
- [Documentation maintenance — 2026-10-18](operations/completion-checklist.md)
- [Startup and token verification](operations/demo-token-connectivity.md)
- [deployment-and-evidence](operations/deployment-and-evidence.md)
- [Final implementation audit - 2026-10-17](operations/final-audit.md)
- [Milestone 1 handoff](operations/handoff-marco-1.md)
- [Local relay rollout and automatic startup](operations/local-relay-rollout.md)
- [Milestone 1 closure review](operations/milestone-1-review.md)
- [Milestone 2 acceptance review — 2026-10-12](operations/milestone-2-review.md)
- [Milestone 3.6 — Integrated acceptance and full project audit](operations/milestone-3-review.md)
- [Milestone 4.4 — full verification and Milestone 4 closure](operations/milestone-4-4-review.md)
- [Milestone 4.5 — controlled local rollout and operational verification](operations/milestone-4-5-rollout.md)
- [Milestone 5.1 — resource boundaries and observability foundations](operations/milestone-5-1.md)
- [Milestone 5.2 — readiness and operational freshness](operations/milestone-5-2.md)
- [Milestone 5.3 — reproducible measurements and acceptance](operations/milestone-5-3.md)
- [Milestone 6.1 — real delivery with persistent evidence](operations/milestone-6-1.md)
- [Milestone 6.2 — durable duplicate delivery](operations/milestone-6-2.md)
- [Milestone 6.3 — final acceptance](operations/milestone-6-3.md)
- [Milestone 6 preparation — reference comparison and live interface scope](operations/milestone-6-preparation.md)
- [Operator replay procedure](operations/replay.md)

## security

- [Authority, diagnostics and transport](security/diagnostic-and-authority-boundaries.md)

## testing

- [Routine verification](testing/strategy.md)
- [Validation results](testing/validation-results.md)
