[Português brasileiro](../pt-BR/testing.md) | [README](../../README.md)

# Testing

```bash
cargo test --locked
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
```

```bash
docker compose exec app cargo test --locked
docker compose exec app cargo fmt --check
docker compose exec app cargo clippy --locked --all-targets --all-features -- -D warnings
```

`cargo test` also works without `--locked`; use the flag for reproducible validation. Run `cargo fmt` to apply formatting. CI uses the same checks on Rust 1.95.0. No tests need databases or brokers.

Current unit tests verify defaults and rejection of invalid addresses/empty environment without modifying process environment. Integration tests start a real listener on an ephemeral port, check HTTP 200 and body, request graceful shutdown and verify the listener closes. A Unix subprocess test loads an isolated `.env`, checks the startup environment log and separately sends SIGTERM/SIGINT, expecting successful exit and a final shutdown log. It requires the OS `kill` command (included in the Docker image). Its temporary files live outside the source tree. Windows skips only that Unix test. Server/process waits have deadlines; there are no external services or fixed test ports.

These tests establish bootstrap correctness, not event reliability. Future milestones will add persistence/broker integration, publication/acknowledgement crash windows, retries, idempotency, poison messages, concurrency bounds and backpressure. Infrastructure tests should isolate state and inject failures; benchmarks must publish workload, hardware, methodology and measured limitations. See [validation results](validation-results.md) for commands actually executed; CI configuration is not proof of a completed GitHub run.

## Canonical envelope tests

`tests/event_envelope.rs` verifies generated UUID v7 identity and UTC time, validation on construction/restoration, exact JSON shape, metadata round-trips and timestamp offset normalization. These tests are independent of PostgreSQL.

## PostgreSQL infrastructure validation (Milestone 1.2)

These are explicit local infrastructure checks, not application persistence integration tests. Start with the documented local defaults; use your configured user/database if different:

```bash
docker compose up -d --build --wait --wait-timeout 120
docker compose ps
docker compose exec -T postgres pg_isready -h 127.0.0.1 -p 5432 -U relay -d reliable_event_relay
docker compose exec -T postgres sh -c 'PGPASSWORD="$POSTGRES_PASSWORD" psql -h postgres -p 5432 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT 1;"'
docker compose port postgres 5432
# Host psql, if installed; enter the local password when prompted:
psql -h localhost -p 5433 -U relay -d reliable_event_relay -W -c "SELECT 1;"
docker compose exec -T app cargo fmt --check
docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings
docker compose exec -T app cargo test --locked
docker compose exec -T app cargo build --locked
```

Confirm PostgreSQL is healthy, host publication is `127.0.0.1:5433`, authentication succeeds and files exist under `.dockerized-postgres/18/docker/`. A TCP host probe proves reachability only, not SQL authentication. `/health` and lifecycle tests continue to verify application behavior without a database connection.

### Container recreation survival

Only on your local development database: use a uniquely named disposable regular table (a SQL TEMP table cannot survive a session), insert a marker, recreate the PostgreSQL container without deleting host data, verify the marker and then drop the table. If the chosen name already exists, stop and choose another; never drop an unrelated object. The validation object is infrastructure-only and must not remain afterward:

```bash
docker compose exec -T postgres psql -U relay -d reliable_event_relay -v ON_ERROR_STOP=1 -c "CREATE TABLE milestone12_validation_20261007 (marker text); INSERT INTO milestone12_validation_20261007 VALUES ('survives-recreation');"
docker compose up -d --force-recreate --no-deps --wait --wait-timeout 120 postgres
docker compose exec -T postgres psql -U relay -d reliable_event_relay -v ON_ERROR_STOP=1 -c "SELECT marker FROM milestone12_validation_20261007;"
docker compose exec -T postgres psql -U relay -d reliable_event_relay -v ON_ERROR_STOP=1 -c "DROP TABLE milestone12_validation_20261007;"
```

Confirm the container ID changed and the mount still points at the same physical directory. Do not reset/delete the data directory as part of survival validation. This test says nothing about application transactions, crash recovery or delivery guarantees. See [actual results](validation-results.md) and [destructive reset instructions](docker-and-configuration.md).
