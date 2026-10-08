[English](README.md) | [Português brasileiro](README.pt-BR.md)

# Reliable Event Relay

A Rust service exploring reliable event delivery, retries, idempotency, backpressure, dead-letter handling, and failure recovery in distributed systems.

This open-source engineering portfolio and study project investigates distributed event delivery under failure. Rust is the implementation tool; the engineering problem is the reason for the repository. English is the canonical documentation language; the Portuguese documentation covers the same scope.

## Current status and scope

**Implemented today:** environment and optional `.env` configuration, structured JSON tracing, an Axum HTTP server, `GET /health` returning `200` and `ok`, SIGINT/SIGTERM shutdown, configuration and lifecycle tests, a validated canonical event envelope with UUID v7, UTC timestamps and JSON round-trip tests, Docker development with local PostgreSQL and versioned SQL migration infrastructure and the initial durable outbox schema and application-level persistence contracts and a read-only PostgreSQL repository (no application writes), isolated opt-in PostgreSQL repository integration tests, a confirmed RabbitMQ publisher adapter, local broker management and Supervisor process control, CI checks and bilingual documentation.

**Planned / future exploration:** outbox writes and processing, delivery-state updates, retry orchestration, idempotency, dead-letter isolation, worker pools, bounded concurrency and backpressure, HTTP webhooks, Redis Streams, readiness, Prometheus metrics and failure experiments. The HTTP binary does not run an outbox worker; the publisher is callable separately. The provisional roadmap is in [architecture](docs/en/architecture.md).

## Architecture

A thin `main.rs` loads configuration, configures tracing, binds a socket and runs the application. `application.rs` owns HTTP lifecycle; `config.rs` owns parsing; `telemetry.rs` owns logging. `domain/event.rs` defines the canonical event envelope; the application publisher contract is implemented by the Lapin 4.12.0 RabbitMQ infrastructure adapter. SQLx 0.8.6 backs PostgreSQL reads. Compose provides PostgreSQL and RabbitMQ separately; the HTTP binary connects to neither service. The persistence module exposes bounded pending snapshots and classified errors for future generic callers; `infrastructure/postgres` implements OutboxReader with an injected PgPool.

## Development

Prerequisites: Docker Engine/Desktop with Compose v2. Native development also needs stable Rust with rustfmt and Clippy; Docker and CI use Rust 1.95.0 for a reproducible baseline. Bash and Cargo are included in the development image.

```bash
cp .env.example .env
# Linux: edit LOCAL_UID and LOCAL_GID in .env to match id -u and id -g.
# Set RabbitMQ credentials in .env before startup.
docker compose build app
docker compose run --rm --no-deps app cargo build --locked
python3 scripts/start-local.py
```

In another terminal: `curl --fail http://localhost:8080/health`. Supervisor manages the `http` binary. Open **http://localhost:15672/** and log in with `RABBITMQ_DEFAULT_USER` / `RABBITMQ_DEFAULT_PASS` from ignored `.env`. See [Milestone 2.1](docs/en/milestone-2-1.md) for exact collective/named/interactive commands, build/start workflow and existing-volume credential handling. Source changes require stop, build and start; there is no automatic reload.

```bash
docker compose exec app bash
docker compose exec app cargo test --locked
docker compose exec app cargo fmt --check
docker compose exec app cargo clippy --locked --all-targets --all-features -- -D warnings
```

Native equivalents: `cargo run --locked`, `cargo test --locked`, `cargo fmt --check`, and `cargo clippy --locked --all-targets --all-features -- -D warnings`.

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

Host clients use `127.0.0.1:5433`; containers use `postgres:5432`. DBeaver uses POSTGRES_DB/USER/PASSWORD from your local configuration, matching persisted cluster credentials. Changing initialization variables does not update an existing cluster. `.dockerized-postgres/` persists through Compose down. SQLx CLI 0.8.6 provides explicit migrations creating the relay namespace and relay.outbox_events; application writes and outbox worker delivery remain future work. See [PostgreSQL and destructive reset](docs/en/postgresql.md) and [migration commands](docs/en/database-migrations.md).

## RabbitMQ and Supervisor

```bash
docker compose exec app bash
supervisorctl status
supervisorctl stop all
supervisorctl start all
supervisorctl stop http
supervisorctl start http
supervisorctl restart http
```

The same arguments work with `supervisor`; run either command without arguments for its interactive console. `all` controls app programs only, currently `http`. Browser login and initialization behavior are documented in [Milestone 2.1](docs/en/milestone-2-1.md).

## Documentation

- [Architecture, trade-offs and roadmap](docs/en/architecture.md)
- [PostgreSQL decision and trade-offs](docs/en/postgresql.md)
- [Database migrations](docs/en/database-migrations.md)
- [Outbox schema](docs/en/outbox-schema.md)
- [Persistence abstraction](docs/en/persistence-abstraction.md)
- [Dependency recovery](docs/en/development-dependencies.md)
- [Docker and configuration](docs/en/docker-and-configuration.md)
- [Project guide and dependency choices](docs/en/project-guide.md)
- [Testing](docs/en/testing.md)
- [Validation results](docs/en/validation-results.md)
- [ADR 001: Rust](docs/en/adr/001-use-rust-for-the-relay.md)
- [ADR 002: PostgreSQL](docs/en/adr/002-use-postgresql-for-durable-event-storage.md)
- [ADR 003: Versioned SQL migrations](docs/en/adr/003-use-versioned-sql-migrations.md)
- [ADR 004: Transactional outbox schema](docs/en/adr/004-use-postgresql-transactional-outbox-schema.md)
- [ADR 005: Persistence boundary](docs/en/adr/005-separate-persistence-contracts-from-postgresql.md)

## Limits and philosophy

`/health` proves HTTP liveness only, with no infrastructure readiness or delivery guarantee. The binary has graceful HTTP shutdown; Supervisor bounds child shutdown to 30 seconds. There is no HTTP application authentication, producer writes, relay processing, performance measurement or production deployment image. Future at-least-once delivery requires consumer idempotency; no exactly-once guarantee is claimed. Decisions will evolve through tests and documented failure experiments. Favor clear failure semantics, resource control and recovery over complexity or unmeasured claims.

MIT licensed; see [LICENSE](LICENSE).

[Milestone 1 is closed](docs/en/milestone-1-review.md): envelope, PostgreSQL, migrations, schema, contracts, repository, integration and [failure/transaction semantics](docs/en/failure-and-transaction-semantics.md). [Milestone 2.1](docs/en/milestone-2-1.md) implements the publisher; delivery state and at-least-once orchestration remain deferred. [Portuguese handoff PDF](HANDOFF_MARCO_1.pdf) | [Markdown source](docs/pt-BR/handoff-marco-1.md).
