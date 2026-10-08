[English](README.md) | [Português brasileiro](README.pt-BR.md)

# Reliable Event Relay

A Rust service exploring reliable event delivery, retries, idempotency, backpressure, dead-letter handling, and failure recovery in distributed systems.

This open-source engineering portfolio and study project investigates distributed event delivery under failure. Rust is the implementation tool; the engineering problem is the reason for the repository. English is the canonical documentation language; the Portuguese documentation covers the same scope.

## Current status and scope

**Implemented today:** environment and optional `.env` configuration, structured JSON tracing, an Axum HTTP server, `GET /health` returning `200` and `ok`, SIGINT/SIGTERM shutdown, configuration and lifecycle tests, a validated canonical event envelope with UUID v7, UTC timestamps and JSON round-trip tests, Docker development with local PostgreSQL and versioned SQL migration infrastructure and the initial durable outbox schema and application-level persistence contracts and a read-only PostgreSQL repository (no application writes), isolated opt-in PostgreSQL repository integration tests, CI checks and bilingual documentation.

**Planned / future exploration:** outbox writes and processing, RabbitMQ delivery, retries, idempotency, dead-letter isolation, worker pools, bounded concurrency and backpressure, HTTP webhooks, Redis Streams, readiness, Prometheus metrics and failure experiments. The Rust application does not persist or deliver events today. The provisional roadmap is in [architecture](docs/en/architecture.md).

## Architecture

A thin `main.rs` loads configuration, configures tracing, binds a socket and runs the application. `application.rs` owns HTTP lifecycle; `config.rs` owns parsing; `telemetry.rs` owns logging. `domain/event.rs` defines the canonical event envelope; delivery modules remain planned. SQLx 0.8.6 backs PostgreSQL reads; broker dependencies remain deferred. Compose provides local PostgreSQL; the binary does not connect to it. The persistence module exposes bounded pending snapshots and classified errors for future generic callers; `infrastructure/postgres` implements OutboxReader with an injected PgPool.

## Development

Prerequisites: Docker Engine/Desktop with Compose v2. Native development also needs stable Rust with rustfmt and Clippy; Docker and CI use Rust 1.95.0 for a reproducible baseline. Bash and Cargo are included in the development image.

```bash
cp .env.example .env
# Linux: edit LOCAL_UID and LOCAL_GID in .env to match id -u and id -g.
docker compose up -d --build
docker compose exec app cargo fetch --locked
docker compose exec app cargo build --locked
docker compose exec app cargo run --locked
```

In another terminal: `curl --fail http://localhost:8080/health`. Stop the foreground service with Ctrl-C. The development container itself remains available until `docker compose down`. Source code is bind-mounted; changes require restarting `cargo run` (no automatic reload).

```bash
docker compose exec app bash
docker compose exec app cargo test --locked
docker compose exec app cargo fmt --check
docker compose exec app cargo clippy --locked --all-targets --all-features -- -D warnings
```

Native equivalents: `cargo run --locked`, `cargo test --locked`, `cargo fmt --check`, and `cargo clippy --locked --all-targets --all-features -- -D warnings`.

## Configuration and recovery

`.env` is ignored. Existing process variables override `.env`; absence of `.env` is supported, malformed files fail startup. Defaults: `APP_ENV=development`, `RUST_LOG=info`, `HTTP_ADDR=0.0.0.0:8080`. Compose publishes only on host loopback; `HOST_HTTP_PORT` changes the host port. Keep container port 8080 in `HTTP_ADDR` unless you also change Compose mapping.

`CARGO_HOME=/app/.cargo-cache` stores registry/git dependencies; `CARGO_TARGET_DIR=/app/target` stores compiled artifacts. Both are physical host directories under the source bind mount, ignored by Git. They are created by the entrypoint, with no named volumes. Restore downloads with `cargo fetch --locked` and artifacts with `cargo build --locked` inside the container. Full recovery:

```bash
docker compose down
rm -rf .cargo-cache target
docker compose up -d
docker compose exec app cargo fetch --locked
docker compose exec app cargo build --locked
docker compose exec app cargo test --locked
```

## Local PostgreSQL

Host clients use `127.0.0.1:5433`; containers use `postgres:5432`. DBeaver uses POSTGRES_DB/USER/PASSWORD from your local configuration, matching persisted cluster credentials. Changing initialization variables does not update an existing cluster. `.dockerized-postgres/` persists through Compose down. SQLx CLI 0.8.6 provides explicit migrations creating the relay namespace and relay.outbox_events; application writes and delivery remain future work. See [PostgreSQL and destructive reset](docs/en/postgresql.md) and [migration commands](docs/en/database-migrations.md).

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

`/health` proves HTTP liveness only, with no infrastructure readiness or delivery guarantee. Graceful HTTP shutdown has no forced timeout yet; long-lived requests could delay termination. There is no authentication, producer writes, relay processing, performance measurement or production deployment image. Future at-least-once delivery requires consumer idempotency; no exactly-once guarantee is claimed. Decisions will evolve through tests and documented failure experiments. Favor clear failure semantics, resource control and recovery over complexity or unmeasured claims.

MIT licensed; see [LICENSE](LICENSE).

[PostgreSQL repository — Milestone 1.6](docs/en/postgres-repository.md). Milestone 1.7 repository integration tests are implemented and validated. Next: 1.8, broader failure/transaction analysis. See [opt-in testing](docs/en/testing.md).
