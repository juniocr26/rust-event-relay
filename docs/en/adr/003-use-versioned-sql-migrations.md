[Português brasileiro](../../pt-BR/adr/003-use-versioned-sql-migrations.md) | [README](../../../README.md)

# ADR 003 — Use Versioned SQL Migrations for PostgreSQL Schema Evolution

## Status

Accepted — 2026-10-07, Milestone 1.3.

## Context

Local PostgreSQL now persists a cluster across container recreation. Reconstructing intended schema requires reviewed source history, not manual GUI changes. Application persistence and the outbox belong to later milestones. Persisted roles also survive changes to initialization environment variables; schema rollback cannot repair this mismatch.

## Decision

Use SQLx CLI 0.8.6 as pinned Docker development tooling, installed with its packaged lockfile and only rustls/PostgreSQL features. Keep reversible timestamped SQL files under `migrations/`. No Rust runtime dependency or automatic startup migration is introduced. Compose gates app startup on PostgreSQL health; migration commands are explicit.

Create an empty `relay` namespace as the initial infrastructure migration (Option B), reserving an explicit boundary for future qualified objects. Its down migration uses RESTRICT. SQLx's metadata table is infrastructure bookkeeping. No application tables are created. A Python standard-library wrapper safely percent-encodes the internal connection URL on every CLI invocation.

## Alternatives considered

- Manual SQL/DBeaver changes: easy experimentation, but weak reproducibility and no canonical reviewable history.
- ORM-generated schema synchronization: adds model/tool coupling and can obscure exact SQL; unnecessary without persistence models.
- Application-managed creation: couples lifecycle and schema privileges, complicates startup failures and concurrent execution; premature here.
- Versioned SQL migrations: explicit PostgreSQL semantics, inspectable SQL, checksums and reversible development workflow. SQLx CLI fits the Rust/Docker toolchain without linking SQLx into the application.

## Consequences

Migration files own schema evolution and are committed with code. Shared applied history is immutable by default. DBeaver is for inspection, queries and debugging. Generated physical data, Cargo cache and build artifacts remain ignored. New clusters require explicit migration application. Initialization credentials are separate cluster state; resetting them deletes all local state only by deliberate developer action.

## Trade-offs

Docker builds take longer and include Python for reliable URL encoding. SQL must be written and reviewed, including rollback. Each PostgreSQL migration is transactional by default, with exceptions requiring explicit nontransactional handling. Down scripts cannot restore all lost data and are not a production recovery guarantee. The empty namespace is an architectural boundary, not outbox persistence. Version and lockfile pinning do not make upstream base-image/package rebuilds immutable.

See [workflow and limitations](../database/migrations.md).
