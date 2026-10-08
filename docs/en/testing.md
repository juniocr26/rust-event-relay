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

`cargo test` also works without `--locked`; use the flag for reproducible validation. Run `cargo fmt` to apply formatting. CI uses the same checks on Rust 1.95.0. Default tests need no database or broker; PostgreSQL tests are opt-in.

Current unit tests verify defaults and rejection of invalid addresses/empty environment without modifying process environment. Integration tests start a real listener on an ephemeral port, check HTTP 200 and body, request graceful shutdown and verify the listener closes. A Unix subprocess test loads an isolated `.env`, checks the startup environment log and separately sends SIGTERM/SIGINT, expecting successful exit and a final shutdown log. It requires the OS `kill` command (included in the Docker image). Its temporary files live outside the source tree. Windows skips only that Unix test. Server/process waits have deadlines; there are no external services or fixed test ports.

These tests establish bootstrap correctness, not event reliability. Repository integration is implemented in Milestone 1.7. Future milestones will add broker integration, publication/acknowledgement crash windows, retries, idempotency, poison messages, concurrency bounds and backpressure. Infrastructure tests should isolate state and inject failures; benchmarks must publish workload, hardware, methodology and measured limitations. See [validation results](validation-results.md) for commands actually executed; CI configuration is not proof of a completed GitHub run.

## Canonical envelope tests

`tests/event_envelope.rs` verifies generated UUID v7 identity and UTC time, validation on construction/restoration, exact JSON shape, metadata round-trips and timestamp offset normalization. These tests are independent of PostgreSQL.

## PostgreSQL infrastructure validation (Milestone 1.2)

These are explicit local infrastructure checks, not application persistence integration tests. Start with the documented local defaults; use your configured user/database if different:

```bash
docker compose up -d --build --wait --wait-timeout 120
docker compose ps
docker compose exec -T postgres sh -c 'pg_isready -h 127.0.0.1 -p 5432 -U "$POSTGRES_USER" -d "$POSTGRES_DB"'
docker compose exec -T postgres sh -c 'PGPASSWORD="$POSTGRES_PASSWORD" psql -h postgres -p 5432 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT 1;"'
docker compose port postgres 5432
# Host psql, if installed; enter the local password when prompted:
psql -h 127.0.0.1 -p 5433 -U change_me -d reliable_event_relay -W -c "SELECT 1;"
docker compose exec -T app cargo fmt --check
docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings
docker compose exec -T app cargo test --locked
docker compose exec -T app cargo build --locked
```

Confirm PostgreSQL is healthy, host publication is `127.0.0.1:5433`, authentication succeeds and files exist under `.dockerized-postgres/18/docker/`. A TCP host probe proves reachability only, not SQL authentication. `/health` and lifecycle tests continue to verify application behavior without a database connection.

### Container recreation survival

Only on your local development database: use a uniquely named disposable regular table (a SQL TEMP table cannot survive a session), insert a marker, recreate the PostgreSQL container without deleting host data, verify the marker and then drop the table. If the chosen name already exists, stop and choose another; never drop an unrelated object. The validation object is infrastructure-only and must not remain afterward:

```bash
docker compose exec -T postgres sh -c 'psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "CREATE TABLE milestone12_validation_20261007 (marker text); INSERT INTO milestone12_validation_20261007 VALUES ('\''survives-recreation'\'');"'
docker compose up -d --force-recreate --no-deps --wait --wait-timeout 120 postgres
docker compose exec -T postgres sh -c 'psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT marker FROM milestone12_validation_20261007;"'
docker compose exec -T postgres sh -c 'psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "DROP TABLE milestone12_validation_20261007;"'
```

Confirm the container ID changed and the mount still points at the same physical directory. Do not reset/delete the data directory as part of survival validation. This test says nothing about application transactions, crash recovery or delivery guarantees. See [actual results](validation-results.md) and [destructive reset instructions](docker-and-configuration.md).

## Migration validation — Milestone 1.3

These are tooling/infrastructure checks, not outbox integration tests. Rust tests still need no database. On a disposable local cluster, confirm PostgreSQL health, TCP authentication and migration status; apply, inspect, revert and re-apply. Revert only after reviewing the down SQL and confirming this is an appropriate local database.

```bash
docker compose up -d --build --wait --wait-timeout 120
docker compose ps
docker compose exec -T app sqlx --version
docker compose exec -T app sqlx migrate info
docker compose exec -T app sqlx migrate run
docker compose exec -T app sqlx migrate info
docker compose exec -T postgres sh -c 'psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -c "SELECT nspname FROM pg_namespace;"'
docker compose exec -T app sqlx migrate revert
docker compose exec -T app sqlx migrate info
docker compose exec -T app sqlx migrate run
docker compose exec -T app sqlx migrate run
docker compose exec -T app sqlx migrate info
docker compose exec -T postgres sh -c 'PGPASSWORD="$POSTGRES_PASSWORD" psql -h postgres -p 5432 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT 1;"'
docker compose port postgres 5432
docker compose exec -T app cargo fmt --check
docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings
docker compose exec -T app cargo test --locked
docker compose exec -T app cargo build --locked
```

With current history, expect installed/pending/installed for the latest outbox migration and outbox table present/absent/present, preserving the relay schema and earlier history. The initial Milestone 1.3 validation of namespace rollback is historical; do not expect current revert to drop relay. A repeated run applies nothing. SQLx metadata remains after revert. To test migration generation without adding project history, run `docker compose exec -T app sqlx migrate add -r --source /tmp/milestone13-generated create_relay_schema` and inspect the two generated files.

### Isolated validation fixture

When local credentials differ from persisted state or data must be preserved, use a separate Compose project and data directory. This fixture uses example-only credentials, including URI-reserved characters to test encoding. Choose an unused directory/project/port; never reuse unknown cluster data. A GUI is not required.

```bash
mkdir -p /tmp/relay-milestone13-validation
cat > /tmp/relay-milestone13-validation/test.env <<'EOF'
POSTGRES_USER=validation_user
POSTGRES_PASSWORD='validation:p@ss/%?#'
POSTGRES_DB=milestone13_validation
POSTGRES_HOST=postgres
POSTGRES_PORT=5432
EOF
cat > /tmp/relay-milestone13-validation/compose.yaml <<'EOF'
services:
  app:
    image: rust-event-relay-app
    pull_policy: never
    ports: !reset []
  postgres:
    volumes:
      - /tmp/relay-milestone13-validation/data:/var/lib/postgresql
    ports: !override
      - "127.0.0.1:15433:5432"
EOF
dc() {
  docker compose -p relay-milestone13-validation --env-file /tmp/relay-milestone13-validation/test.env -f compose.yaml -f /tmp/relay-milestone13-validation/compose.yaml "$@"
}
dc up -d --wait --wait-timeout 120
# Substitute dc for docker compose in the migration checks above.
# Docker Desktop: test authentication through the host publication:
dc exec -T postgres sh -c 'PGPASSWORD="$POSTGRES_PASSWORD" psql -h host.docker.internal -p 15433 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT 1;"'
# Stop/remove only the fixture containers/network; preserve its data directory:
dc down
```

The fixture requires Compose supporting `!override` (2.24.4+) and `!reset`. The custom image must already be built. host.docker.internal is Docker Desktop-specific; on Linux use an authenticated host psql query instead.

For the initialization-variable reproduction, change only the fixture postgres environment through an additional override, recreate that container with health-gated `up --force-recreate --no-deps`, and verify the changed credentials fail while original app credentials still connect. Restore the original container environment and confirm the applied schema survives. Never reset the developer cluster to perform this experiment. For unavailability, stop only fixture postgres, expect `dc exec -T app sqlx migrate info --connect-timeout 2` to fail, then restart with `dc up -d --wait --wait-timeout 120 postgres` and confirm installed status. See [actual results](validation-results.md).

## Read-only authentication diagnostics

```bash
./scripts/check-postgres.sh
docker compose exec -T app sqlx migrate info
```

[check-postgres.sh](../../scripts/check-postgres.sh) checks health, IPv4 loopback publication, current Compose/.env versus running app/postgres settings, authenticated identity and existing migration history. It prints neither the configured username nor password. It exits nonzero on configuration drift or authentication failure, and never changes roles, creates metadata, applies migrations or resets data. It uses Python inside the running app container; the host TCP probe uses nc or Python 3, explicitly reporting skipped if neither is available. SQLx connectivity is checked separately.

Compose supplies POSTGRES_HOST/PORT to postgres for these client diagnostics; this does not change server listening settings. Host/DBeaver uses 127.0.0.1:5433 and current initialized POSTGRES_DB/USER/PASSWORD; containers use postgres:5432. DBeaver GUI was not tested.

Changing `.env` does not update an initialized cluster. A missing role can produce generic TCP password-authentication failure. Inspect server details/roles to distinguish it from an existing role with a wrong password. The real-cluster repair inspected for data before the explicitly authorized destructive reset, then ran existing migrations. Normal Compose startup/shutdown remains non-destructive. Reset deletes all cluster state; migration rollback does not repair credentials.

Raw Compose config, environment dumps and SQLx help can reveal secrets; inspect through a parser reporting only non-sensitive fields/equality checks and redact identifying credentials before sharing logs. See [PostgreSQL repair](postgresql.md#authentication-remediation-on-the-real-local-cluster) and [actual remediation results](validation-results.md#local-postgresql-authentication-remediation).

## Outbox schema validation — Milestone 1.4

These are database schema fixtures, not Rust repository integration tests. The source-controlled [SQL fixture](../../tests/sql/outbox_schema.sql) uses a transaction and final ROLLBACK: ON_ERROR_STOP=1 also causes a failed connection/session to roll back, preserving preexisting rows. It uses reserved synthetic UUIDs; a collision fails safely, so use an appropriate local database. No seed data is introduced.

It asserts 25 cases: valid envelope/defaults, duplicate identity, all 11 NOT NULL fields, empty required strings, versions 0/-1/above u32, negative attempts, invalid status, completion timestamp consistency, full u32 maximum/optional UUIDs/JSON null/time offsets, and representable pending retry/terminal failure. Payload type is inspected as JSONB. No broker, retry loop or worker claim executes.

```bash
docker compose exec -T app sqlx migrate info
docker compose exec -T app sqlx migrate run
docker compose exec -T app sqlx migrate info
docker compose exec -T postgres sh -c 'psql -X -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1' < tests/sql/outbox_schema.sql
docker compose exec -T postgres sh -c 'psql -X -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT count(*) AS outbox_rows FROM relay.outbox_events;"'
```

Use catalog queries for columns/types/nullability/defaults, pg_constraint for the primary key/checks, pg_indexes for index definitions and _sqlx_migrations for version/success. Exact executed queries and results are in [validation results](validation-results.md).

**Destructive rollback:** only after confirming the outbox has no important data and the last migration is create_outbox_events, run migrate revert, inspect outbox absence and relay/history preservation, then migrate run and info. Do not leave the local schema reverted. Rollback loses all table rows after real data exists; it is not a production recovery guarantee. Run the four Docker Cargo checks documented above. This milestone's final developer table had zero rows after fixtures and after reapplication.

The reusable [read-only catalog inspection](../../tests/sql/inspect_outbox_schema.sql) checks all columns, constraints, indexes, history and row count:

```bash
docker compose exec -T postgres sh -c 'psql -X -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1' < tests/sql/inspect_outbox_schema.sql
```

## Persistence contract tests — Milestone 1.5

[tests/persistence_contract.rs](../../tests/persistence_contract.rs) contains five database-independent Rust tests:

- Positive batch limits, zero rejection and no arbitrary maximum; explicit UTC cutoff survives request construction.
- Canonical envelope identity/time/payload/UUID metadata remains intact, while attempt/availability metadata stays outside the serialized nine-field event.
- All error categories, optional downcastable sources and source redaction in both Display and Debug.
- Generic OutboxReader usage with a one-response scripted fake, captured request and Tokio-spawned Send future.
- Successful empty snapshots and typed unavailable failure propagation through a generic caller.

The fake holds only one prepared result; it is not an in-memory engine and does not simulate eligibility filtering, locks, durability or claims. The tests do not query PostgreSQL, instantiate pools, read .env or execute SQL. Milestone 1.6 adds adapter tests and a focused smoke check; the integration suite is implemented in 1.7. Pending metadata uses unsigned u32; checked signed-width decoding is implemented in the adapter.

```bash
docker compose exec -T app cargo test --locked --test persistence_contract
docker compose exec -T app cargo fmt --check
docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings
docker compose exec -T app cargo test --locked
docker compose exec -T app cargo build --locked
docker compose exec -T -e RUSTDOCFLAGS='-D warnings' app cargo doc --locked --no-deps
```

Distinguish Milestone 1.4 SQL schema fixtures above, these Milestone 1.5 contract/model tests, and Milestone 1.7 PostgreSQL repository integration tests. The existing schema fixtures are unchanged and were not rerun for this Rust-only milestone. See [contract requirements](persistence-abstraction.md) and [actual validation](validation-results.md).

## PostgreSQL repository integration — Milestone 1.7

`cargo test --locked` remains database-independent: 15 PostgreSQL cases are ignored; the closed-pool/oversized-limit case needs no server. Opt in explicitly:

```bash
docker compose exec -T app cargo test --locked --test postgres_repository -- --ignored --test-threads=1
docker compose exec -T app cargo test --locked --test postgres_repository -- --ignored --test-threads=4
# Native/CI equivalent, with POSTGRES variables explicitly exported:
cargo test --locked --test postgres_repository -- --ignored --test-threads=4
```

Required variables: POSTGRES_USER, POSTGRES_PASSWORD and POSTGRES_DB (existing database for the administrative connection). POSTGRES_HOST defaults to postgres; POSTGRES_PORT defaults to 5432. Host execution uses 127.0.0.1 and the published port (default 5433); CI uses 127.0.0.1:5432. The harness does not load .env itself; Compose supplies it. Missing configuration or unavailable PostgreSQL fails explicitly. The role needs CREATEDB and ownership of its generated databases. CI uses PostgreSQL 18.6 with disposable test credentials in a dedicated job; its initialization role can create databases. Existing database-independent checks remain separate.

Shared support creates one `relay_it_<UUID hex>` database per case, safely quotes the controlled name, and applies the two committed up migrations in chronological order. Fixtures use explicit IDs/timestamps and SQL outside the production API. It never seeds the administrative/application database. Spawned cases preserve assertion diagnostics; returned driver errors are redacted with operation/SQLSTATE diagnostics for fixture operations. Connection/acquisition waits are 10 seconds, server statements 5 seconds, locks 3 seconds, cases 45 seconds, and pool closure/drop waits 10 seconds. Cases use a barrier for independent-reader synchronization, with no fixed sleeps.

After success, panic or returned error, support closes the test pool before dropping exactly its database and verifies catalog absence. Two dedicated cases exercise assertion-failure and returned-error cleanup. Abrupt process termination, server unavailability during cleanup or deadline failure may leave an isolated database; logs identify its exact generated name. Inspect ownership and remove only that exact fixture manually after closing its connections; never reset data or broadly delete databases.

Coverage: empty tables and empty reads with future/terminal rows; inclusive cutoff; pending/processed/dead-letter filtering; bounded backlogs and available_at/created_at/id tie-breakers as adapter behavior. Restoration checks every envelope field and pending metadata, versions 1/u32::MAX, attempts 0/i32::MAX, optional IDs, preserved whitespace, historical timestamps, offsets and microseconds, nested/null/array/scalar JSON and exact large numbers including 1e1000. JSON comparisons use stored semantic values, allowing JSONB formatting/key normalization; timestamps compare UTC instants, not offset spelling or nanoseconds.

Repeated reads and two readers observe identical unchanged snapshots; complete stored rows match before/after. Selected blank required fields, infinite timestamps, negative-infinite availability and a finite date outside Chrono fail the entire batch with sources retained; correcting fixtures restores valid reads. Malformed future/terminal rows are excluded safely. Closed-pool and missing-table failures preserve typed SQLx sources and sanitized formatting. An oversized representable usize limit fails before closed-pool access. Constraints remain unchanged; unreachable negative counters, invalid versions and malformed JSONB bytes stay in unit/schema tests.

These cases do not establish safety of concurrent delivery, business ordering, or snapshot behavior under every concurrent write schedule. Milestone 1.8 documents [failure/recovery and transaction semantics](failure-and-transaction-semantics.md); crash injection remains unvalidated. Producer writes, claims, leases, delivery, retries and lifecycle transitions remain outside this suite.

## Milestone 1 closure checks

The ignored `committed_schema_fixture_validates_constraints_and_rolls_back` case executes the existing 25-case SQL fixture only in its isolated database and verifies zero rows after rollback. It does not add production writes or modify shared application state. The nested JSON fixture is compared to explicit expected content, normalizing only the large exponent spelling. The suite now has 15 opt-in cases plus one database-independent case.

Run `./scripts/check-postgres.sh` and `docker compose exec -T app sqlx migrate info` before the Cargo checks above. The catalog script `tests/sql/inspect_outbox_schema.sql` is read-only; fixture writes belong to the isolated Rust case. See [review](milestone-1-review.md), [semantics](failure-and-transaction-semantics.md) and [actual closing results](validation-results.md).
