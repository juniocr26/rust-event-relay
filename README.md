[English](README.md) | [Português brasileiro](README.pt-BR.md)

# Reliable Event Relay

[Milestone 3 acceptance, operational gaps and study path](docs/en/operations/milestone-3-review.md): closed with explicit nonblocking limitations. Replay API remains disabled; replay schema is installed. Presentation is local simulation.

[Frontend/backend preparation — current layout and commands](docs/en/guides/frontend-backend.md). Native Cargo commands run in `backend/`; frontend commands run in `frontend/`.


[Milestone 2.6 — Integrated recovery](docs/en/architecture/milestone-2-6.md): real crash, uncertainty, concurrency and shutdown experiments completed. [Milestone 2 closed under explicit assumptions](docs/en/operations/milestone-2-review.md). Local ownership/retry migrations installed; http and relay autostart. [Current rollout](docs/en/operations/local-relay-rollout.md).

A Rust service exploring reliable event delivery, retries, idempotency, backpressure, dead-letter handling, and failure recovery in distributed systems.

This open-source engineering portfolio and study project investigates distributed event delivery under failure. Rust is the implementation tool; the engineering problem is the reason for the repository. English is the canonical documentation language; the Portuguese documentation covers the same scope.

[Milestone 4.5 — local rollout complete](docs/en/operations/milestone-4-5-rollout.md): verified protected backup/isolated restore, six applied migrations, current HTTP/relay/consumer with autostart and retained real 10 → 15 → 15 demonstration. Replay API disabled; frontend remains simulation.

## Documentation catalog and reading path

- [Complete categorized catalog](docs/en/index.md)
- [Review checklist and evidence gaps](docs/en/operations/completion-checklist.md)

Read purpose/setup first, then architecture and contracts, security and failures, tests/evidence, and local deployment/recovery. This is an independent portfolio project; runtime procedures in the documentation were not executed in the 2026-10-10 review.

## Current status and scope

Milestone 4 is closed with explicit nonblocking limitations. [4.4 verification/closure](docs/en/operations/milestone-4-4-review.md) records real inventory/ACK/quarantine recovery, exact test counts, the then-pending shared migrations/Supervisor installation and [environment inventory](docs/en/docker/environment-inventory.md). Presentation frontend verified; live backend integration is not implemented.

**Implemented today:** canonical event envelope, versioned PostgreSQL outbox/ownership/retry/replay schema, focused persistence contracts and adapters, confirmed RabbitMQ publication, sequential relay with bounded maintenance/reconnection/shutdown, durable retry and terminal preservation, poison discovery/isolation, opt-in authorized idempotent replay with audit, atomic PostgreSQL inventory effect/deduplication with historical receipts, RabbitMQ consumer with confirmed quarantine and manual ACK (activated locally in 4.5), HTTP liveness, Docker/Supervisor operation and a Vue presentation frontend. The [current acceptance matrix](docs/en/operations/milestone-4-4-review.md) separates isolated verification from shared running state and exact guarantees.

**Planned / future exploration:** transactional producer writes, live frontend/backend integration, audited repair, worker pools/backpressure, webhooks, Redis Streams, readiness/metrics, retention, production TLS/HA and independent backup/recovery operations. Local backup/restore verification is complete in 4.5. The HTTP binary does not run the relay; replay-enabled HTTP connects to PostgreSQL. The frontend runs local illustrative scenarios only.

[Milestone 2.2](docs/en/architecture/milestone-2-2.md) implements the ownership decision, models, contracts and schema only. Leased events remain pending; expired owners cannot complete/release. Tokens protect database transitions but do not fence publication or establish end-to-end delivery.

[Milestone 2.3](docs/en/architecture/milestone-2-3.md) implements those PostgreSQL operations with short transactions and post-lock database time. The local ownership/retry migrations were explicitly installed on 2026-10-13. Continuity report (pt-BR) (2.5 report absent from this checkout); current handoff (historical handoff unavailable in this checkout). 2.6 validates an at-least-once foundation under explicit assumptions, without exactly-once consumer effects.

## Architecture

The separate `relay` composition constructs the database pool/repository and RabbitMQ lifecycle; application scheduling admits one delivery at a time with explicit pacing/reconnection and bounded shutdown. See [worker operation](docs/en/architecture/milestone-2-5.md).

A thin `main.rs` loads configuration, configures tracing, binds a socket and runs the application. `application.rs` owns HTTP lifecycle; `config.rs` owns parsing; `telemetry.rs` owns logging. `domain/event.rs` defines the canonical event envelope; the application publisher contract is implemented by the Lapin 4.12.0 RabbitMQ infrastructure adapter. SQLx 0.8.6 backs PostgreSQL reads and ownership transactions. Compose provides PostgreSQL and RabbitMQ separately; liveness-only HTTP connects to neither service; opt-in replay HTTP connects to PostgreSQL after schema preflight. The persistence module exposes bounded pending snapshots and classified errors for future generic callers; `infrastructure/postgres` implements OutboxReader and the acquisition/completion/release ports with an injected PgPool.

## Development

Prerequisites: Docker Engine/Desktop with Compose v2. Native development also needs stable Rust with rustfmt and Clippy; Docker and CI use Rust 1.95.0 for a reproducible baseline. Bash and Cargo are included in the development image.

```bash
cp .env.example .env
# Linux: edit LOCAL_UID and LOCAL_GID in .env to match id -u and id -g.
# Set RabbitMQ credentials in .env before startup.
docker compose build app
docker compose run --rm --no-deps app cargo build --locked --bins
docker compose up -d --wait postgres rabbitmq
# Inspect existing migration SQL and database state before this explicit action:
docker compose run --rm --no-deps app sqlx migrate run
python3 scripts/start-local.py
```

In another terminal: `curl --fail http://localhost:8080/health`. Supervisor automatically starts `http` and `relay`; both remain manually controllable. Open **http://localhost:15672/** and log in with `RABBITMQ_DEFAULT_USER` / `RABBITMQ_DEFAULT_PASS` from ignored `.env`. See [Milestone 2.1](docs/en/architecture/milestone-2-1.md) for exact collective/named/interactive commands, build/start workflow and existing-volume credential handling. Source changes require stop, build and start; there is no automatic reload.

```bash
docker compose exec app bash
docker compose exec app cargo test --locked
docker compose exec app cargo fmt --check
docker compose exec app cargo clippy --locked --all-targets --all-features -- -D warnings
```

Native equivalents (from `backend/`): `cargo run --locked`, `cargo test --locked`, `cargo fmt --check`, and `cargo clippy --locked --all-targets --all-features -- -D warnings`.

## Configuration and recovery

`.env` is ignored. Existing process variables override `.env`; the HTTP binary supports absence of `.env`, while Compose requires broker credentials; malformed files fail binary startup. Defaults: `APP_ENV=development`, `RUST_LOG=info`, `HTTP_ADDR=0.0.0.0:8080`. Compose publishes only on host loopback; `HOST_HTTP_PORT` changes the host port. Keep container port 8080 in `HTTP_ADDR` unless you also change Compose mapping.

`CARGO_HOME=/app/.cargo-cache` stores registry/git dependencies; `CARGO_TARGET_DIR=/app/target` stores compiled artifacts. Both are physical host directories under the source bind mount, ignored by Git. They are created by the entrypoint, without named Cargo volumes; RabbitMQ uses a separate persistent named volume. Restore downloads with `cargo fetch --locked` and artifacts with `cargo build --locked` inside the container. Full recovery:

```bash
docker compose down
rm -rf .cargo-cache target
docker compose run --rm --no-deps app cargo build --locked
python3 scripts/start-local.py
docker compose exec app cargo test --locked
```

## Local PostgreSQL

Host clients use `127.0.0.1:5433`; containers use `postgres:5432`. DBeaver uses POSTGRES_DB/USER/PASSWORD from your local configuration, matching persisted cluster credentials. Changing initialization variables does not update an existing cluster. `.dockerized-postgres/` persists through Compose down. SQLx CLI 0.8.6 provides explicit migrations creating the relay namespace and relay.outbox_events; producer writes remain future work; relay delivery requires the ownership migration. See [PostgreSQL and destructive reset](docs/en/database/postgresql.md) and [migration commands](docs/en/database/migrations.md).

## RabbitMQ and Supervisor

```bash
docker compose exec app bash
supervisorctl status
supervisorctl stop all
# After authorized schema rollout:
supervisorctl stop http
supervisorctl start http
supervisorctl restart http
# After authorized schema rollout:
# supervisorctl start http relay
supervisorctl start relay
supervisorctl stop relay
supervisorctl restart relay
```

The same arguments work with `supervisor`; run either command without arguments for its interactive console. HTTP, relay and consumer now autostart after the authorized local rollout. Use named controls because consumer consumes real messages. Intentional stop remains stopped until explicitly started or the manager restarts; see [containment and recovery](docs/en/operations/milestone-4-5-rollout.md). Browser login and initialization behavior are documented in [Milestone 2.1](docs/en/architecture/milestone-2-1.md).

## Documentation

- [Architecture, trade-offs and roadmap](docs/en/architecture/overview.md)
- [PostgreSQL decision and trade-offs](docs/en/database/postgresql.md)
- [Database migrations](docs/en/database/migrations.md)
- [Outbox schema](docs/en/database/outbox-schema.md)
- [Persistence abstraction](docs/en/architecture/persistence-contracts.md)
- [Dependency recovery](docs/en/guides/dependencies.md)
- [Docker and configuration](docs/en/docker/configuration.md)
- [Project guide and dependency choices](docs/en/guides/project-guide.md)
- [Testing](docs/en/testing/strategy.md)
- [Validation results](docs/en/testing/validation-results.md)
- [ADR 001: Rust](docs/en/adr/001-use-rust-for-the-relay.md)
- [ADR 002: PostgreSQL](docs/en/adr/002-use-postgresql-for-durable-event-storage.md)
- [ADR 003: Versioned SQL migrations](docs/en/adr/003-use-versioned-sql-migrations.md)
- [ADR 004: Transactional outbox schema](docs/en/adr/004-use-postgresql-transactional-outbox-schema.md)
- [ADR 005: Persistence boundary](docs/en/adr/005-separate-persistence-contracts-from-postgresql.md)
- [ADR 006: RabbitMQ publisher and Supervisor](docs/en/adr/006-rabbitmq-publisher-and-supervisor.md)
- [ADR 007: Durable delivery ownership](docs/en/adr/007-durable-delivery-ownership.md)
- [Milestone 2.2: Ownership types, contracts and schema](docs/en/architecture/milestone-2-2.md)

## Limits and philosophy

`/health` proves HTTP liveness only, with no infrastructure readiness or delivery guarantee. The binary has graceful HTTP shutdown; Supervisor bounds child shutdown to 30 seconds. The health route is unauthenticated; privileged replay routes use opt-in trusted bearer configuration and are disabled by default. Producer writes, performance measurement and a production deployment image remain absent. The consumer runtime and confirmed quarantine are active locally after 4.5; Milestone 4 acceptance is complete with explicit nonblocking limitations. At-least-once delivery relies on durable consumer deduplication; no exactly-once guarantee is claimed. Decisions will evolve through tests and documented failure experiments. Favor clear failure semantics, resource control and recovery over complexity or unmeasured claims.

MIT licensed; see [LICENSE](LICENSE).

[Milestone 1 is closed](docs/en/operations/milestone-1-review.md): envelope, PostgreSQL, migrations, schema, contracts, repository, integration and [failure/transaction semantics](docs/en/architecture/failure-and-transactions.md). [Milestone 2.1](docs/en/architecture/milestone-2-1.md) implements the publisher; [Milestone 2.2](docs/en/architecture/milestone-2-2.md) implements state/ownership contracts, with orchestration implemented in [2.4](docs/en/architecture/milestone-2-4.md). [Historical Portuguese handoff source](docs/pt-BR/operations/handoff-marco-1.md).

## Milestone 3.2 — current persistence scope

Durable retry schema, ownership/cycle/revision-protected scheduling and terminal preservation, acquisition budgets and exact-row exhausted maintenance are implemented. Application integration is implemented in 3.3; general poison scanning remains 3.4, replay 3.5 and integrated closure 3.6. Earlier sections retain their milestone-specific historical scope. [Implementation and rollout](docs/en/architecture/milestone-3-2.md).

[Milestone 3.3 — current integration](docs/en/architecture/milestone-3-3.md).

[Local startup, migration prerequisites and Supervisor controls](docs/en/operations/local-relay-rollout.md).


## Milestone 3.4 — current discovery/isolation

Bounded discovery and fresh-authority isolation are implemented; replay remains 3.5 and integrated closure 3.6. Current local rollout is preserved; this change has no migration or shared restart. [Milestone 3.4 — current discovery/isolation](docs/en/architecture/milestone-3-4.md).

[Milestone 3.5 — authorized replay](docs/en/architecture/milestone-3-5.md) | [Operator API](docs/en/operations/replay.md). Migration 5 is installed locally; privileged API remains disabled.

[Milestone 4.2 — atomic PostgreSQL inventory persistence](docs/en/architecture/milestone-4-2.md): implemented adapter/migration and isolated database evidence; RabbitMQ consumer/quarantine runtime is implemented in 4.3; shared local activation completed in 4.5.

[Milestone 4.3 — manual ACK, confirmed quarantine, configuration and explicit rollout](docs/en/architecture/milestone-4-3.md). Consumer now has autostart=true after the authorized [4.5 local rollout](docs/en/operations/milestone-4-5-rollout.md).
