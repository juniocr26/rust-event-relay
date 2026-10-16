[Português brasileiro](../../pt-BR/testing/validation-results.md) | [README](../../../README.md)

# Validation results

## Milestone 1.5 — Persistence abstraction validation

Date: 2026-10-07. Docker development container, Rust 1.95.0. New Rust-level tests use no database, driver, environment configuration, pools or SQL. No migrations/schema changes or PostgreSQL calls were made for this milestone. Prior schema/credential validation remains preserved below.

| Check | Actual result |
| --- | --- |
| Initial targeted test and Clippy attempt | Failed: E0283/E0284, ambiguous Chrono parse timezone in one new assertion |
| Correction | Added explicit parse::<DateTime<Utc>>; no envelope or schema changes |
| Docker cargo fmt --check | Passed after formatting |
| Docker Clippy all targets/features, warnings denied | Passed |
| Docker cargo test --locked | Passed: 2 unit, 6 envelope, 2 lifecycle, 5 persistence tests (15 total) |
| Docker cargo build --locked | Passed |
| Rustdoc with -D warnings, --locked --no-deps | Passed; public persistence API documented |
| git diff --check / local links and anchors | Passed |
| Boundary/source review | No SQLx/PostgreSQL imports or types; no concrete adapter, mutations, worker or producer append |
| Scope verification | Cargo manifest/lock, EventEnvelope, existing migrations and configuration unchanged |

The five new tests cover positive bounded requests/zero rejection/explicit UTC, envelope preservation with separate unsigned attempts/UTC availability, all error categories and downcastable sources with Display/Debug redaction, generic contract use on a Tokio-spawned Send future, and empty-success/typed-failure handling. A single prepared-result fake demonstrates substitution only, not filtering, locking, durability, claim ownership or adapter conformance. Those guarantees cannot be validated without Milestones 1.6/1.7 and future worker semantics.

Exact commands executed (first targeted attempt failed before the explicit UTC correction):

```bash
# Initial attempt
docker compose exec -T app cargo fmt
docker compose exec -T app cargo test --locked --test persistence_contract
docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings

# After fixing the test annotation
docker compose exec -T app cargo fmt
docker compose exec -T app cargo fmt --check
docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings
docker compose exec -T app cargo test --locked
docker compose exec -T app cargo build --locked
docker compose exec -T -e RUSTDOCFLAGS='-D warnings' app cargo doc --locked --no-deps
git diff --check
```

The existing Python pathlib/re documentation scan resolved local file links and heading anchors across both READMEs and docs. Git diff/source review confirmed no edits to dependencies, migrations or canonical envelope and no storage-driver coupling in the new persistence module. No commit or push was created.

Scope decisions: one bounded read-only port instead of prematurely freezing claim/completion/reschedule/dead-letter APIs; no producer writer whose own transaction could misrepresent producer atomicity; no terminal enum without a returned terminal view. These are allowed smaller-boundary choices, not implementation of Milestone 1.6. Open questions include ownership/recovery, mutation atomicity/idempotency/conflicts, attempt update timing, producer duplicates, payload/operational limits and aggregate ordering. No production/throughput or concurrent-worker-safety claims. Configuration and the developer database remain unchanged.

## Milestone 1.4 — Outbox schema validation

Date: 2026-10-07. Real developer PostgreSQL 18.6 cluster, SQLx CLI 0.8.6, Docker Desktop/Linux ARM64, Rust 1.95.0. Existing authentication diagnostic passed before schema work. No cluster reset was performed. The preflight catalog showed no outbox table and only namespace migration 20261007000000 installed, so rollback of the new empty table was safe.

| Validation | Observed result |
| --- | --- |
| Reversible migration generation | CLI created 20261007175358_create_outbox_events up/down pair |
| Real info/run/info | Pending → applied → installed |
| Catalog inspection | 15 columns, correct native types/defaults/nullability, UUID primary key, seven named CHECKs, 11 NOT NULL constraints |
| Index inspection | Primary-key B-tree and pending partial B-tree (available_at, created_at, id); no payload/aggregate indexes |
| SQL fixture | 25 cases passed; valid envelope/defaults/JSONB, duplicate ID, required NULL failures, empty strings, invalid version bounds/attempts/status/completion and extended envelope/lifecycle representation |
| SQL fixture cleanup | ROLLBACK completed; count(*) = 0 |
| Real migrate revert | Outbox absent=true, relay preserved=true, history preserved=true; new migration pending |
| Real reapplication | Outbox recreated, both versions installed with success=true |
| Final read-only inspection | Table/indexes/constraints present; outbox_rows=0 |
| Docker fmt / Clippy / test / build | Passed; Clippy warnings denied; 2 unit + 6 envelope + 2 lifecycle tests (10) |
| git diff --check / local Markdown links and anchors | Passed |

The SQL fixture is schema-level validation, not Rust persistence integration. Constraint failures are caught/asserted in PL/pgSQL subtransactions, including intended check names; fixture rows remain inside an outer transaction that rolls back. No validation events or seed data remain. The final developer database keeps the applied migration and schema. Existing migration files and Rust code/dependencies are unchanged; no repository, publisher, worker, claim query or retry execution was introduced.

### Exact commands executed

```bash
./scripts/check-postgres.sh
docker compose exec -T app sqlx migrate add -r create_outbox_events
# Generated version: 20261007175358; edited SQL before applying.
docker compose exec -T app sqlx migrate info
docker compose exec -T app sqlx migrate run
docker compose exec -T app sqlx migrate info
docker compose exec -T postgres sh -c 'psql -X -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1' < tests/sql/outbox_schema.sql
docker compose exec -T postgres sh -c 'psql -X -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT count(*) AS outbox_rows FROM relay.outbox_events;"'
docker compose exec -T app sqlx migrate revert
docker compose exec -T app sqlx migrate info
docker compose exec -T postgres sh -c 'psql -X -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT to_regclass('\''relay.outbox_events'\'') IS NULL AS outbox_absent, to_regnamespace('\''relay'\'') IS NOT NULL AS relay_preserved, to_regclass('\''public._sqlx_migrations'\'') IS NOT NULL AS history_preserved;"'
docker compose exec -T app sqlx migrate run
docker compose exec -T app sqlx migrate info
docker compose exec -T postgres sh -c 'psql -X -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT count(*) AS outbox_rows FROM relay.outbox_events;"'
docker compose exec -T postgres sh -c 'psql -X -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1' < tests/sql/inspect_outbox_schema.sql
docker compose exec -T app cargo fmt --check
docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings
docker compose exec -T app cargo test --locked
docker compose exec -T app cargo build --locked
git diff --check
```

A preflight SELECT to_regclass('relay.outbox_events') and SELECT version, description, success FROM public._sqlx_migrations confirmed absence/prior history. Before rollback, catalog queries matching the source-controlled inspection file were also executed inline through psql for columns, pg_constraint, pg_indexes and SQLx history. The final inspection file repeated those read-only checks after reapplication, with explicit row count. The repository's Python pathlib/re method resolved local Markdown links and heading anchors.

Design deviation from the suggested type: schema_version uses BIGINT CHECK BETWEEN 1 AND 4294967295 because signed INTEGER cannot represent all Rust NonZeroU32 values. No envelope change was needed. No production benchmarks, concurrent claims, delivery guarantees or repository integration were validated; DBeaver GUI was not tested. Rollback was validated only while the outbox was empty. No commit or push was made. Earlier validation/repair results below are preserved as history.

## Local PostgreSQL authentication remediation

Date: 2026-10-07. This later remediation preserves the original Milestone 1.3 failure and isolated validation below.

**Root cause:** the current configured role did not exist in the initialized cluster. TCP returned password-authentication failure; server log details and authorized local socket inspection confirmed role absence. This was initialization-state drift, not a verified stale password on an existing configured role. No password hashes were queried; real identifying credentials are omitted.

Before reset: PostgreSQL 18.6, data directory `/var/lib/postgresql/18/docker`, one legacy login superuser plus built-in roles. The postgres maintenance database and configured database each had only public, zero user relations/routines and no migration history or application data. That inspection satisfied the explicit reset authorization. After Compose stopped, only `.dockerized-postgres/` was deleted. Current `.env`, Cargo cache and target were preserved; no automatic deletion was added.

| Validation | Observed result |
| --- | --- |
| Real PostgreSQL after reinitialization | Healthy; both containers match current Compose/.env |
| Current role/password/database over internal TCP | Authenticated SELECT current_user, current_database matched configured values (identifiers redacted) |
| Real SQLx info/run/info | Pending → applied → installed; version 20261007000000, success true |
| Schema inspection | public and empty relay; public._sqlx_migrations is the only user table |
| macOS loopback port | nc connected to 127.0.0.1:5433 |
| Host-published SQL authentication | Passed via host.docker.internal:5433 with current credentials |
| Read-only diagnostic | Passed: health, settings equality, authentication and existing migration history |
| Synthetic configuration drift | Nonzero failure; no container recreation or .env changes |
| Synthetic incorrect password | Diagnostic authentication branch failed with generic redacted error |
| Docker fmt, Clippy, test, build | Passed; warnings denied, all 10 Rust tests passed |
| Shell syntax, git diff --check, relative links/anchors | Passed |
| DBeaver GUI | Not directly tested |

DBeaver: PostgreSQL, 127.0.0.1:5433, database from POSTGRES_DB and username from POSTGRES_USER, using unchanged current POSTGRES_PASSWORD. No credential guessing/replacement is required. The real developer cluster accepts these values. No Milestone 1.4 work, application tables, repository, workers or brokers were added. No commit or push.

### Exact remediation commands

Temporary Python wrappers captured and redacted `docker compose ps`, `docker compose config --format json`, `docker compose logs --tail=100 postgres`, port publication and container POSTGRES_USER/DB/HOST/PORT inspection. Raw config was not printed because it contains secrets. Authorized socket SQL read version/data_directory, pg_roles without hashes, pg_database and every non-template database's user schemas/relations/routines. Actual configured and legacy login identifiers were omitted from output.

```bash
python3 /tmp/relay-postgres-diagnose.py
python3 /tmp/relay-postgres-inventory.py
docker compose down
```

Deletion used the following guarded command instead of unguarded shell rm:

```bash
python3 - <<'PY'
from pathlib import Path
import json
import shutil
inventory=json.loads(Path('/tmp/relay-postgres-inventory-result.json').read_text())
assert len(inventory['inventory']) == 2
for database in inventory['inventory']:
    state=database['inventory']
    assert state['schemas'] == ['public']
    assert not state['relations']
    assert state['routines'] == 0
path=Path.cwd()/'.dockerized-postgres'
assert path.is_dir() and not path.is_symlink()
assert path.resolve() == path
shutil.rmtree(path)
print('Explicitly authorized empty-cluster reset completed; .env and Cargo directories preserved')
PY
docker compose up -d --build --wait --wait-timeout 120
docker compose exec -T app sqlx migrate info
docker compose exec -T app sqlx migrate run
docker compose exec -T app sqlx migrate info
./scripts/check-postgres.sh
sh -n scripts/check-postgres.sh
if POSTGRES_PASSWORD=deliberately-invalid-test ./scripts/check-postgres.sh; then exit 1; else echo 'Diagnostic rejects configuration drift as expected'; fi
```

The diagnostic authenticates over TCP and compares SELECT current_user, current_database to configuration without printing identity. Additional authenticated host-publication and schema inspection:

```bash
docker compose exec -T postgres sh -s <<'CONTAINER'
set -eu
export PGPASSWORD="$POSTGRES_PASSWORD"
result=$(psql -X -h host.docker.internal -p 5433 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -At -F '|' -c 'SELECT current_user, current_database();')
[ "$result" = "$POSTGRES_USER|$POSTGRES_DB" ]
echo 'Authenticated query through host publication: current_user and current_database match configuration (identifiers redacted).'
psql -X -h postgres -p "$POSTGRES_PORT" -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT nspname FROM pg_namespace WHERE nspname NOT LIKE 'pg_%' AND nspname <> 'information_schema'; SELECT schemaname, tablename FROM pg_tables WHERE schemaname NOT LIKE 'pg_%' AND schemaname <> 'information_schema'; SELECT version, success FROM public._sqlx_migrations;"
CONTAINER
```

A temporary Python subprocess test extracted the diagnostic's container branch, ran it with `docker compose exec -T -e POSTGRES_PASSWORD=deliberately-invalid-test postgres sh -s`, and asserted nonzero exit plus the generic authentication failure message. Stored credentials were not changed. Quality commands:

```bash
docker compose exec -T app cargo fmt --check
docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings
docker compose exec -T app cargo test --locked
docker compose exec -T app cargo build --locked
git diff --check
git check-ignore .env
```

Documentation links/anchors used the same Python pathlib/re scan as Milestone 1.3. Limits: DBeaver GUI not manually tested; host-publication authentication used Docker Desktop's gateway rather than host psql. Linux portability and production recovery were not tested. Deviations: guarded Python deletion replaced raw rm; diagnostic output was redacted instead of exposing raw config. No other scope deviation.

## Milestone 1.3 — Migration infrastructure validation

Date: 2026-10-07. Docker Desktop on macOS, Linux ARM64 app container. SQLx reports `sqlx-cli 0.8.6`; installation used `--locked --no-default-features --features rustls,postgres`. No application dependencies or domain code changed.

| Check | Observed result |
| --- | --- |
| Development image build and Compose health-gated startup | Passed |
| Existing developer cluster | Healthy, published at 127.0.0.1:5433; current configured credentials fail TCP authentication and sqlx migrate info |
| Developer data preservation | No reset, role/password alteration or migration application to the developer cluster |
| Isolated migration generation | CLI produced paired timestamped up/down files under container /tmp/milestone13-generated |
| Isolated migration info/run/revert/run | Passed; pending → installed → pending → installed |
| Namespace inspection | relay present after apply, absent after revert, present after reapply; zero relay tables |
| SQLx history | Version 20261007000000, success true; repeated run applied nothing |
| Reserved-character credential encoding | Passed authenticated SQLx connection with fixture password containing : @ / % ? # |
| Fixture container TCP and published-host TCP authentication | SELECT 1 passed; host.docker.internal:15433 mapped to fixture PostgreSQL |
| Changed initialization environment with persisted fixture data | New credentials failed while original app credentials still connected; restoration preserved installed migration |
| PostgreSQL unavailable | SQLx exited nonzero with DNS/connection error; healthy restart restored installed status |
| cargo fmt --check | Passed in Docker |
| Clippy all targets/features, warnings denied | Passed in Docker |
| cargo test --locked | Passed: 2 unit + 6 envelope + 2 lifecycle tests (10) |
| cargo build --locked | Passed in Docker |
| git diff --check and local relative Markdown links | Passed |

The existing cluster's credential mismatch was observed directly; its real username/password are omitted. Validation used a separate `relay-milestone13-validation` Compose project with a /tmp data bind mount and loopback port 15433. The fixture's example-only configuration and exact Compose function are in [testing](strategy.md#isolated-validation-fixture). No developer data directory was deleted. The fixture containers/network were removed at the end; temporary fixture data remains under /tmp. The original development environment remains running.

Port publishing and credentials were validated on the isolated fixture, but DBeaver UI connectivity was not directly tested. Original port publishing was confirmed; original configured credentials failed, so connectivity with those credentials is not claimed. No outbox integration, production rollback, host-native Rust, Linux host portability or GitHub CI execution was validated.

### Exact commands executed

On the original project (authentication failures below are expected findings, not successful checks):

```bash
docker compose ps
docker compose up -d --build --wait --wait-timeout 120
docker compose exec -T app sqlx --version
docker compose exec -T app sqlx migrate info
docker compose exec -T postgres sh -c 'PGPASSWORD="$POSTGRES_PASSWORD" psql -h postgres -p 5432 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT 1;"'
docker compose port postgres 5432
docker compose exec -T app cargo fmt --check
docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings
docker compose exec -T app cargo test --locked
docker compose exec -T app cargo build --locked
```

After creating the fixture files exactly as shown in testing:

```bash
docker compose -p relay-milestone13-validation --env-file /tmp/relay-milestone13-validation/test.env -f compose.yaml -f /tmp/relay-milestone13-validation/compose.yaml up -d --wait --wait-timeout 120
dc() {
  docker compose -p relay-milestone13-validation --env-file /tmp/relay-milestone13-validation/test.env -f compose.yaml -f /tmp/relay-milestone13-validation/compose.yaml "$@"
}
dc ps
dc exec -T app sqlx --version
dc exec -T app sqlx migrate add -r --source /tmp/milestone13-generated create_relay_schema
dc exec -T app sqlx migrate info
dc exec -T app sqlx migrate run
dc exec -T app sqlx migrate info
dc exec -T postgres sh -c 'PGPASSWORD="$POSTGRES_PASSWORD" psql -h postgres -p 5432 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT nspname FROM pg_namespace WHERE nspname = '\''relay'\''; SELECT version, success FROM _sqlx_migrations; SELECT tablename FROM pg_tables WHERE schemaname = '\''relay'\'';"'
dc exec -T app sqlx migrate revert
dc exec -T app sqlx migrate info
dc exec -T postgres sh -c 'psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT count(*) AS relay_schema_count FROM pg_namespace WHERE nspname = '\''relay'\'';"'
dc exec -T app sqlx migrate run
dc exec -T app sqlx migrate run
dc exec -T app sqlx migrate info
dc port postgres 5432
dc exec -T postgres sh -c 'PGPASSWORD="$POSTGRES_PASSWORD" psql -h host.docker.internal -p 15433 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT 1;"'
```

For credential-initialization reproduction, wrote this additional fixture-only override:

```yaml
# /tmp/relay-milestone13-validation/changed-environment.yaml
services:
  postgres:
    environment:
      POSTGRES_USER: validation_changed_user
      POSTGRES_PASSWORD: validation_changed_password
```

Then executed (with the same dc function):

```bash
dc -f /tmp/relay-milestone13-validation/changed-environment.yaml up -d --force-recreate --no-deps --wait --wait-timeout 120 postgres
if dc exec -T postgres sh -c 'PGPASSWORD="$POSTGRES_PASSWORD" psql -h postgres -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT 1;"'; then
  echo 'Unexpected success with changed initialization credentials' >&2
  exit 1
fi
# app retains the original isolated test credentials, proving they still work.
dc exec -T app sqlx migrate info
dc up -d --force-recreate --no-deps --wait --wait-timeout 120 postgres
dc exec -T postgres sh -c 'PGPASSWORD="$POSTGRES_PASSWORD" psql -h postgres -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT 1;"'
dc exec -T app sqlx migrate info
```

Unavailable-server failure and cleanup:

```bash
dc stop postgres
if dc exec -T app sqlx migrate info --connect-timeout 2; then
  echo 'Unexpected success with unavailable PostgreSQL' >&2
  exit 1
fi
dc up -d --wait --wait-timeout 120 postgres
dc exec -T app sqlx migrate info
dc down
```

The three groups ran as temporary shell scripts using `set -eu`; expected failures used if statements to assert nonzero exit status. `dc exec -T app sqlx migrate info --help` was also executed to inspect the connection timeout option. Workspace verification ran `git diff --check`, `git check-ignore .env .dockerized-postgres/ .cargo-cache/ target/` and a Python pathlib/re scan resolving local Markdown links and anchors. No commit or push was made.

URL-encoding and topology assertions also passed inside Docker. The exact additional command used synthetic credentials only:

```bash
docker compose exec -T app python3 - <<'PY'
import importlib.util
from urllib.parse import unquote, urlsplit
spec = importlib.util.spec_from_file_location('sqlx_wrapper', '/app/docker/sqlx.py')
wrapper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wrapper)
environment = dict(POSTGRES_USER='user:@/雪', POSTGRES_PASSWORD='password:@/%?#雪', POSTGRES_DB='db/@?#雪', POSTGRES_HOST='postgres', POSTGRES_PORT='5432')
url = urlsplit(wrapper.database_url(environment))
assert unquote(url.username) == environment['POSTGRES_USER']
assert unquote(url.password) == environment['POSTGRES_PASSWORD']
assert unquote(url.path[1:]) == environment['POSTGRES_DB']
assert url.hostname == 'postgres' and url.port == 5432
assert not url.query and not url.fragment
for key, value in [('POSTGRES_HOST', 'localhost'), ('POSTGRES_PORT', '5433')]:
    try:
        wrapper.database_url({**environment, key: value})
    except ValueError:
        pass
    else:
        raise AssertionError('invalid Docker topology accepted')
print('URL encoding and topology validation passed')
PY
```

## Milestone 1.2 — 2026-10-07

Environment: macOS with Docker Desktop; existing Rust development image rebuilt and PostgreSQL official image downloaded. No Rust source, Cargo dependencies, migrations or application persistence were added.

| Validation | Observed result |
| --- | --- |
| Compose build/start with bounded readiness wait | Passed; app running and PostgreSQL healthy |
| Compose configuration and PostgreSQL logs | Passed; server ready to accept connections |
| pg_isready | Passed; 127.0.0.1:5432 accepting connections |
| Password-authenticated TCP SQL via postgres service DNS | Passed; SELECT 1 returned 1 |
| Selected server | 18.6 (Debian 18.6-1.pgdg12+2) |
| Data directory / mount | /var/lib/postgresql/18/docker; one bind mount from host .dockerized-postgres to /var/lib/postgresql, no data volume |
| Published host port | 127.0.0.1:5433 -> container 5432 |
| Host TCP socket | Passed with Python; initial sandbox-denied attempt was rerun with approved local network access |
| Host physical files | PG_VERSION exists under .dockerized-postgres/18/docker and contains 18 |
| App-container TCP access | Passed; bounded Bash /dev/tcp connection to postgres:5432 |
| Container recreation survival | Passed; regular disposable table marker survived a changed container ID with the same bind mount |
| Validation cleanup | Passed; table dropped and user_tables query returned 0 |
| Docker cargo fmt --check | Passed |
| Docker Clippy all targets/features, warnings denied | Passed |
| Docker cargo test --locked | Passed; 10 tests (2 configuration, 6 envelope, 2 lifecycle) |
| Docker cargo build --locked | Passed |
| Git ignores / documentation links / git diff --check | Passed; all relative links resolve |

The marker table `milestone12_validation_20261007` contained only `survives-recreation`. The PostgreSQL container ID changed from `601cd8ad9a28...` to `ee3342e7a231...`; the marker remained, then the table was removed. No application schema or validation artifact remains. Database contents were never reset or deleted. Both development containers remain running; the app workspace does not automatically run the HTTP binary.

Not validated: a host SQL session or DBeaver GUI (host psql is absent); the host probe establishes TCP reachability only. Authenticated SQL was validated inside Docker. Native Rust checks were not repeated because Cargo remains unavailable on the host; all four required checks ran in Docker. Linux host permission portability, production backups/operations and application persistence integration remain unvalidated/out of scope. No deviations from implementation scope; no unused Rust connection configuration was introduced.

Exact commands executed (checks of container IDs/mounts occurred before and after recreation; the socket probe was rerun after sandbox access approval):

```bash
docker compose up -d --build --wait --wait-timeout 120
docker compose config --quiet
docker compose ps
docker compose logs --tail=20 postgres
docker compose exec -T postgres pg_isready -h 127.0.0.1 -p 5432 -U relay -d reliable_event_relay
docker compose exec -T postgres sh -c 'PGPASSWORD="$POSTGRES_PASSWORD" psql -h postgres -p 5432 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT 1; SHOW server_version; SHOW data_directory;"'
docker compose port postgres 5432
docker compose exec -T app bash -c 'timeout 5 bash -c "</dev/tcp/postgres/5432"'
docker compose exec -T app cargo fmt --check
docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings
docker compose exec -T app cargo test --locked
docker compose exec -T app cargo build --locked
command -v psql
python3 - <<'PY'
import socket
from pathlib import Path
with socket.create_connection(('127.0.0.1',5433), timeout=5):
 print('Host TCP 127.0.0.1:5433 reachable')
p=Path('.dockerized-postgres/18/docker/PG_VERSION')
print('Host PG_VERSION:',p.read_text().strip())
PY
docker compose ps -q postgres
docker inspect rust-event-relay-postgres-1 --format '{{json .Mounts}}'
docker compose exec -T postgres psql -U relay -d reliable_event_relay -v ON_ERROR_STOP=1 -c "CREATE TABLE milestone12_validation_20261007 (marker text); INSERT INTO milestone12_validation_20261007 VALUES ('survives-recreation');"
docker compose up -d --force-recreate --no-deps --wait --wait-timeout 120 postgres
docker compose ps -q postgres
docker inspect rust-event-relay-postgres-1 --format '{{json .Mounts}}'
docker compose exec -T postgres psql -U relay -d reliable_event_relay -v ON_ERROR_STOP=1 -c "SELECT marker FROM milestone12_validation_20261007;"
docker compose exec -T postgres psql -U relay -d reliable_event_relay -v ON_ERROR_STOP=1 -c "DROP TABLE milestone12_validation_20261007;"
docker compose exec -T postgres psql -U relay -d reliable_event_relay -v ON_ERROR_STOP=1 -c "SELECT count(*) AS user_tables FROM pg_tables WHERE schemaname NOT IN ('pg_catalog', 'information_schema');"
git check-ignore .dockerized-postgres/ .cargo-cache/ target/
python3 - <<'PY'
from pathlib import Path
import re
errors=[]; count=0
for p in [Path('README.md'),Path('README.pt-BR.md'),*Path('docs').rglob('*.md')]:
 for target in re.findall(r'\]\(([^)]+)\)',p.read_text()):
  if '://' in target or target.startswith('#'): continue
  path=target.split('#')[0]
  if not (p.parent/path).exists(): errors.append(f'{p}: {target}')
  count+=1
print(f'Checked {count} relative documentation links')
if errors: raise SystemExit('\n'.join(errors))
print('All relative documentation links resolve')
PY
git diff --check
```

## Milestone 1.1 — 2026-10-07

The existing Docker development container was available. Native checks were attempted individually but could not run because Cargo is not installed/on PATH on the macOS host. Docker checks executed against the mounted source; no image rebuild was needed. `cargo check` resolved the new dependencies and updated Cargo.lock before the locked checks.

| Check | Observed result |
| --- | --- |
| Native fmt, Clippy, test, build | Unavailable: all four returned `command not found: cargo` |
| Docker `cargo fmt --check` | Passed |
| Docker `cargo clippy --locked --all-targets --all-features -- -D warnings` | Passed |
| Docker `cargo test --locked` | Passed: 2 configuration unit tests, 6 envelope integration tests, 2 lifecycle integration tests (10 total) |
| Docker `cargo build --locked` | Passed |
| `git diff --check` | Passed |

Six new integration tests cover creation, generated unique UUID v7 IDs, current UTC timestamps, all four validation rules (including whitespace-only identifiers), explicit nine-field JSON shape, restoration of original ID/time, round-trips with absent/individual/combined correlation and causation metadata, invalid JSON values and normalization of timestamp offsets to UTC. No persistence or delivery components were implemented or validated. Native checks remain the only environment-related deviation from the milestone validation request.

Exact commands executed for this milestone:

```bash
# Native host attempts: each returned command not found: cargo
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked
cargo build --locked

# Docker development container
docker compose ps
docker compose exec -T app cargo fmt
docker compose exec -T app cargo check
docker compose exec -T app cargo fmt --check
docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings
docker compose exec -T app cargo test --locked
docker compose exec -T app cargo build --locked

# Workspace review
git diff --check
```

## Foundation validation (previous)

Date: 2026-10-07. Environment: macOS host with Docker Desktop, Linux ARM64 development container, `rustc 1.95.0 (59807616e 2026-04-14)`, UID/GID 1000. Native Rust was not installed, so Rust checks ran inside Docker against the mounted repository.

| Validation | Observed result |
| --- | --- |
| Docker development image build and Compose startup | Passed |
| Bash and non-root toolchain | Passed; developer UID/GID 1000 |
| Cargo fetch and lockfile generation | Passed; Cargo.lock generated and included in source |
| cargo build --locked | Passed |
| cargo test --locked and cargo test | Passed; 2 unit tests and 2 integration tests |
| cargo fmt --check | Passed after applying cargo fmt |
| Clippy all targets/features, warnings denied | Passed |
| Application startup and JSON tracing | Passed; development environment and address logged |
| Host HTTP request | Passed; /health returned HTTP 200 and `ok` |
| Local .env loading | Passed through isolated subprocess test |
| SIGINT and SIGTERM graceful exit | Passed through subprocess test for each signal |
| Full source-cache/build-artifact recovery | Passed after deleting both host directories and restarting Compose |
| Host visibility of generated data | Passed; .cargo-cache/registry and target/debug recreated |
| Documentation relative links and ignored directories | Checked locally |

## Foundation commands (previous validation)

The first fetch intentionally omitted `--locked` to generate the initial lockfile; subsequent fetch/build validations used the lockfile. The Python deletion below removed only generated project directories, equivalent to the documented recovery `rm -rf .cargo-cache target`.

```bash
docker compose up -d --build
docker compose exec -T app bash -c 'cargo fmt && cargo fetch && cargo build --locked && cargo test --locked && cargo fmt --check && cargo clippy --locked --all-targets --all-features -- -D warnings'
docker compose exec -T -d app bash -c 'exec /app/target/debug/reliable-event-relay >/tmp/relay-live.log 2>&1'
curl --fail --include http://127.0.0.1:8080/health
docker compose exec -T app bash -c 'id && rustc --version && cat /tmp/relay-live.log'
docker compose down
python3 - <<'PY'
from pathlib import Path
import shutil
for name in ['.cargo-cache', 'target']:
    p = Path(name)
    assert p.is_dir() and not p.is_symlink()
    shutil.rmtree(p)
PY
docker compose up -d
docker compose exec -T app bash -c 'cargo fetch --locked && cargo build --locked && cargo test --locked && cargo fmt --check && cargo clippy --locked --all-targets --all-features -- -D warnings'
docker compose exec -T app cargo test
```

The unit tests cover default configuration and rejection cases. The HTTP test uses a real socket and an injected shutdown trigger. The subprocess test supplies a temporary `.env` outside the repository and validates both Unix signals. Git ignore rules were checked with `git check-ignore .env .cargo-cache/ target/ .vscode/`; relative Markdown links were resolved against the filesystem. A review confirmed that relay/durability features are marked as planned.

## Foundation limitations (previous validation)

Native host Rust, Windows signal behavior, Linux host UID/GID portability, GitHub-hosted CI execution and production deployment were not validated. No PostgreSQL, RabbitMQ, Redis, webhook delivery, retries, idempotency, backpressure or crash recovery was implemented or tested. There are no benchmarks or throughput/latency claims. Health indicates only HTTP liveness. The development workspace container remains running after validation; the temporary live HTTP process ended when Compose was stopped for recovery. Start the service with `docker compose exec app cargo run --locked`.

## Milestone 1.6 — PostgreSQL repository (2026-10-08)

Checkout started clean at `223953f` after `39eca7a`. No applicable AGENTS.md was found. Docker Desktop app and healthy PostgreSQL were accessible after sandbox-approved Docker socket access; app reports Rust 1.95.0. SQLx application dependency 0.8.6 matches the installed CLI, defaults disabled with postgres/runtime-tokio/uuid/chrono/json. serde_json arbitrary_precision prevents numeric payload rounding. Cargo resolution retained every preexisting locked package version; new transitive packages were added.

Both existing migrations were initially pending. The existing `docker compose exec -T app sqlx migrate run` wrapper explicitly applied them; final info reports both installed. No migration was rewritten, no credentials changed and no developer database reset occurred.

| Final executed check | Result |
| --- | --- |
| cargo fmt --check | Passed |
| cargo clippy --locked --all-targets --all-features -- -D warnings | Passed |
| cargo test --locked | Passed: 22 tests; one opt-in smoke ignored |
| cargo build --locked | Passed |
| RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps | Passed |
| cargo test --locked --test postgres_read_smoke -- --ignored | Passed: one isolated PostgreSQL smoke |
| git diff --check | Passed |

All Cargo commands ran via `docker compose exec -T app` (rustdoc flags injected with `-e`). No compile-time SQL macros or database-dependent build metadata are needed. Smoke uses a uniquely named temporary database and unchanged migration DDL, checking empty reads, cutoff/pending filtering, deterministic bounded selection, full version range, preserved strings/IDs/timestamps/optional UUIDs/precise JSON numbers, repeated snapshots, and all-column before/after equality. Selected infinity/out-of-Chrono-range finite timestamps and whitespace-invalid fields fail the whole read as InvalidStoredData.

Initial smoke exposed a panic in SQLx 0.8.6 Chrono decoding of infinity. Checked private timestamp decoding fixed it. The first panic left a fixture database, which was explicitly removed by its exact generated name. The test now runs the check in a spawned task and performs database cleanup even after a panic. A later numeric fixture comparison exposed PostgreSQL's normalization of exponent spelling; final fixture uses exact stored numeric spelling while unit tests cover large exponents. A temporary boxed-source test compile error was also corrected before final checks. Public fixture failure output is static; no raw sources or credentials are printed.

Final catalog inspection found zero remaining relay_smoke_* databases and zero rows in the developer outbox. No fixture writes touched that outbox. No validation remains blocked. This smoke does not complete 1.7's full integration/concurrency matrix or 1.8's broader failure/transaction/recovery analysis; no processing or delivery is implemented. Existing historical validation sections below/above describe their original milestones, not current dependency scope.

## Milestone 1.7 — PostgreSQL repository integration (2026-10-08)

Started from clean `c771f62`; no applicable AGENTS.md exists. The running Docker app reports Rust 1.95.0. Replaced the historical Milestone 1.6 smoke with `tests/postgres_repository.rs` and shared `tests/support/mod.rs`: 14 ignored PostgreSQL cases plus one database-independent public-contract failure case. No production defect was found; production code, dependencies, migrations, initialized credentials and application data were unchanged. No commit or push was performed.

Final commands ran through `docker compose exec -T app` (rustdoc flags supplied with `-e`):

| Command | Actual result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo clippy --locked --all-targets --all-features -- -D warnings` | Passed |
| `cargo test --locked` | Passed: 23 tests; 14 database cases ignored |
| `cargo test --locked --test postgres_repository -- --ignored --test-threads=1` | Passed: 14 cases |
| `cargo test --locked --test postgres_repository -- --ignored --test-threads=4` | Passed: 14 cases; parallel isolation verified |
| `cargo build --locked` | Passed |
| `RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps` | Passed |
| `git diff --check` | Passed |

Additional deliberate failure probes selected only `eligibility_bounds_and_adapter_tie_breakers -- --ignored --exact`: `docker compose exec -T app env -u POSTGRES_USER cargo test --locked --test postgres_repository ...` exited 101 with `missing required POSTGRES_USER`; overriding only the command's POSTGRES_HOST=127.0.0.1 and POSTGRES_PORT=1 exited 101 after 10 seconds with sanitized administrative connection failure. These expected failures prove explicit opt-in does not silently skip missing/unavailable infrastructure. They did not create databases or change the container/server configuration.

Every successful harness case closes its test pool, drops its exact controlled database name and verifies absence by parameterized catalog query, including cases intentionally panicking or returning an ordinary error. Final read-only catalog query `SELECT count(*) FROM pg_database WHERE datname ~ '^relay_it_[0-9a-f]{32}$'` returned 0 after all runs and probes. No unrelated database was dropped or reset. Abrupt process termination or failed cleanup can still leave an isolated fixture; this is documented with exact-name manual cleanup guidance.

Initial harness compilation rejected an SQLx raw_sql after_connect closure's lifetime; session deadlines were moved to PgConnectOptions startup options. No production fix was needed. All final checks passed. A dedicated PostgreSQL 18.6 CI job was added with disposable credentials; no remote GitHub run was executed. Native host execution and abrupt-process/crash recovery were not validated. Milestone 1.7 is implemented; Milestone 1.8 is next for broader failure/recovery and transaction analysis. Two-reader observation does not establish safe concurrent delivery or every concurrent-write snapshot schedule.

## Milestone 1.8 and Milestone 1 closure (2026-10-08)

Current execution began from clean `7cba38d`, the committed Milestone 1.7 integration suite. No applicable AGENTS.md exists. The earlier 1.7 records above are historical; every check below was executed again for this closing tree. Milestone 1.8 is documentation/review, not a crash recovery implementation. Milestone 1 is closed within acceptance criteria 1.1-1.8; no blockers remain and Milestone 2 was not begun. Closing changes are uncommitted; no commit, push or remote CI run occurred.

The [review](../operations/milestone-1-review.md) found three medium documentation inconsistencies (stale status, lock-free implication, source SQL versus database migration metadata) and two low coverage/validation improvements. They were corrected in both languages. No production defect was confirmed, so no production code, dependency, Cargo.lock or migration changes were required. The complex JSON case now asserts the entire explicit fixture, normalizing only the exponent representation. One ignored case runs the unchanged 25-case schema SQL fixture in the existing isolated harness and verifies rollback leaves zero rows.

### Commands and current results

Cargo commands used the running app via `docker compose exec -T app`; strict rustdoc used `docker compose exec -T -e RUSTDOCFLAGS='-D warnings' app cargo doc --locked --no-deps`. Formatting was applied once with `cargo fmt` before the checks.

| Executed command | Result in this execution |
| --- | --- |
| `docker compose exec -T app rustc --version` | Rust 1.95.0 |
| `./scripts/check-postgres.sh` | Passed: healthy PostgreSQL, 127.0.0.1:5433 TCP probe, current/running configuration equality, authenticated role/database equality, two successful history entries |
| `docker compose exec -T app sqlx migrate info` | Both 20261007000000 and 20261007175358 installed; no migration application necessary |
| `cargo fmt --check` | Passed |
| `cargo clippy --locked --all-targets --all-features -- -D warnings` | Passed |
| `cargo test --locked` | Passed: 23 database-independent tests; 15 database cases ignored |
| `cargo test --locked --test postgres_repository -- --ignored --test-threads=1` | Passed: 15 opt-in cases |
| `cargo test --locked --test postgres_repository -- --ignored --test-threads=4` | Passed: 15 cases, parallel isolated databases |
| `cargo build --locked` | Passed |
| `RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps` | Passed |
| `git diff --check` | Passed |

The committed `tests/sql/inspect_outbox_schema.sql` was run read-only with `docker compose exec -T postgres sh -c 'PGCONNECT_TIMEOUT=5 PGOPTIONS="-c statement_timeout=5000" psql -X -w -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1' < tests/sql/inspect_outbox_schema.sql`. It reported 15 columns, 11 NOT NULL fields, primary key/seven checks (PostgreSQL 18 also exposes NOT NULL as catalog constraints), identity/pending indexes, both successful migrations and zero application outbox rows. All schema fixture writes ran in a generated test database; no writes touched the application outbox. No shared database reset/rollback, credentials change or PostgreSQL restart occurred.

A bounded read-only query observed PostgreSQL `18.6 (Debian 18.6-1.pgdg12+2)`, transaction isolation `read committed`, and fsync/synchronous_commit/full_page_writes `on`. These are session/configuration observations, not a power-loss or storage durability test. `SELECT count(*) FROM pg_database WHERE datname ~ '^relay_it_[0-9a-f]{32}$'` returned 0 after the final integration runs. Each harness case additionally verifies absence of its exact parameterized database name after closing pools/dropping it, including assertion/error cleanup cases. No unrelated database was removed.

### Documentation and PDF checks

[Failure/transaction semantics](../architecture/failure-and-transactions.md) uses PostgreSQL major-version 18 and SQLx 0.8.6 official documentation; tested behavior, inspected code, published semantics and future engineering inference are labeled separately. ADRs 001-005 were retained without changing their historical decisions. README/status, architecture, project guides, storage/contract/repository/testing pages and closing review were reconciled in both languages; historical validation sections were preserved.

`HANDOFF_MARCO_1.pdf` is generated from `docs/pt-BR/operations/handoff-marco-1.md` by `scripts/generate_handoff.py` using ReportLab in an isolated temporary Python environment, without adding Rust or Dockerfile dependencies. Poppler 22.12.0 was installed only in the running development container for QA. PDF metadata and strict pypdf reopening passed; all eight A4 pages have selectable text and Portuguese accents. All pages were rendered at 110 dpi with pdftoppm and visually inspected. An initial orphan reference paragraph created a ninth page; the reference table was condensed, regenerated, and the final eight-page layout checked. Text extraction, page bounds, page numbering, relative documentation links and diff whitespace were checked. The reviewed base and uncommitted status are explicit; no credentials, personal paths or raw logs appear in the handoff.

### Limits of the closing evidence

The dedicated CI job was reviewed against the tested command and PostgreSQL 18.6 service; no GitHub-hosted execution was performed. Native host Rust, Windows behavior, production deployment, benchmark/performance, backup/restore exercises, abrupt-process cleanup/crash recovery, publisher/ack failure injection, query-cancellation timing, exhausted-pool integration injection and every concurrent-write schedule were not validated. No current acceptance check was blocked or skipped. These future limits do not convert persistence/observation into a working delivery guarantee; see the failure matrix and open decisions.

## Milestone 2.1 — 2026-10-08

Executed on the user's Docker Desktop host with Rust 1.95.0, Lapin 4.12.0, RabbitMQ 4.3.6 management and preserved PostgreSQL 18.6. Changes were implemented directly without commit/push. The Milestone 1 handoff was not edited. [Milestone 2.1 scope and commands](../architecture/milestone-2-1.md) supersede earlier runtime/setup descriptions.

| Check | Actual result |
| --- | --- |
| App image rebuild and real binary build | Passed, developer user and writable Cargo mounts retained |
| Format / strict Clippy / strict rustdoc / diff whitespace | Passed |
| Default Rust suite | 27 passed, 18 opt-in cases ignored; no PostgreSQL/RabbitMQ needed |
| Existing PostgreSQL integration | All 15 ignored cases passed with 4 threads |
| RabbitMQ integration target | 2 passed: exact envelope/properties, routed ack, ack-with-return failure, confirmation timeout and post-send cancellation through isolated proxy, no retry |
| Closed owned RabbitMQ connection | 1 opt-in library case passed |
| Bounded stalled handshake / refused transport / typed sanitized sources | Passed in default unit tests |
| Host management endpoint | HTTP 200 login page at exactly http://localhost:15672/ |
| Authenticated management | `/api/whoami` and visible project vhost passed with `.env` credentials and management tag; scoped configure/write/read permissions verified with node CLI |
| Management-port conflict detection | Temporary occupied host port reported; neither shared service nor default port changed |
| Supervisor control | `status`, collective stop/start and named stop/start/restart passed using direct supervisorctl, executable supervisor alias and both interactive consoles |
| Child lifecycle | HTTP unavailable while stopped and healthy after start/restart; manager PID unchanged; private developer-owned socket modes verified |
| Unexpected child exit | SIGKILL only to managed HTTP child; automatic restart and health recovery passed |
| Graceful shutdown | Direct binary SIGTERM/SIGINT tests passed; managed intentional stops logged shutdown requested/application stopped and expected exit 0; app recreation retained broker/database data |

An initial management diagnostic attempted the permission-listing API and received 401 because the management-only tag does not grant administrative permission listing. `/api/whoami` authenticated successfully; the diagnostic was corrected to validate visible vhosts via API and actual permission regex through the node CLI. No password, tag or broker data was reset to resolve that diagnostic error.

**Not directly tested:** browser form login (no host browser-control tool available), remote GitHub CI, production TLS/HA, broker power loss, real broker nack injection, prolonged HTTP drain beyond Supervisor's budget, exhausted publication-admission load, or abrupt integration-harness death. Publisher serialization failure is a defensive category, not a deliberately injected failure of the validated current envelope. HTTP/API verification is not reported as browser login. At-least-once worker delivery remains incomplete.


## Milestone 2.2 — 2026-10-09

Verified clean checkout at `5eac9ff`; no applicable project/ancestor AGENTS.md. Earlier sections record historical checks, not current evidence. Existing app/PostgreSQL/RabbitMQ containers were already running; no services started, rebuilt, reset or restarted. No credentials, dependencies/lockfile, original migrations or application data changed; no commit/push.

All Cargo commands ran via `docker compose exec -T app` in existing Docker tooling (native cargo is unavailable in host PATH). Rustdoc used `docker compose exec -T -e RUSTDOCFLAGS='-D warnings' app cargo doc --locked --no-deps`.

| Actual final check | Result |
| --- | --- |
| cargo fmt --check | Passed after formatting |
| cargo clippy --locked --all-targets --all-features -- -D warnings | Passed, including updated lifecycle test |
| cargo test --locked | Passed: 32 tests; 19 opt-in infrastructure tests ignored in standard invocation |
| cargo test --locked --test delivery_ownership_schema -- --ignored | Passed: 1 isolated PostgreSQL migration case |
| cargo test --locked --test postgres_repository -- --ignored --test-threads=4 | Passed: 15 isolated existing integration cases |
| cargo test --locked --test rabbitmq_publisher -- --ignored | Passed: 2 existing broker cases |
| cargo test --locked --lib infrastructure::rabbitmq::tests::closed_owned_connection_is_unavailable -- --ignored | Passed: 1 existing broker case |
| cargo build --locked | Passed |
| Strict cargo doc --locked --no-deps | Passed |
| Markdown relative file-link check and git diff --check | Passed |
| sqlx migrate info (read-only) | Original two installed; new ownership migration pending in development database |
| Bounded read-only catalog/outbox queries | 0 relay_it_ databases remaining; 0 shared outbox rows |

The isolated new case seeds an original-schema event, compares all original columns after migration, rejects each partial ownership-field combination and invalid token/time/state/count combination, accepts coherent ownership, verifies the existing reader observes leased pending data without mutation, clears ownership with valid processed shape, checks reader exclusion, applies down and verifies event identity remains. Existing harness closes pools, drops only its exact generated database and verifies absence. New migration was never applied to shared/development data. Existing PostgreSQL tests intentionally retain original schema fixtures. CI job now includes new isolated schema case; no remote CI run.

Initial and second full-suite invocations failed only the existing health_and_graceful_shutdown assertion that an immediate synchronous TCP connection must fail after server return. Its target rerun passed unchanged. Replaced the one-shot probe with bounded asynchronous checks requiring eventual listener unavailability within the same three-second budget; no HTTP production change. The final complete suite passed. Exact cause of the intermittent socket observation was not established; do not interpret the new check as evidence for an untested shutdown mechanism.

Five new pure/fake tests verify duration/token/timestamp validity, exact expiry, terminal shape, eligibility, stale/expired owner rejection, replacement, repeat rejection, counter boundaries without partial mutation, stable event identity and Send/static contracts. They are not PostgreSQL locking/concurrency/durability evidence. No production acquisition/completion/release or orchestration is implemented. Those operations, clock sampling after lock waits, competing owners, stale mutation predicates, commit uncertainty and concurrency integration belong to 2.3. Worker/end-to-end recovery, production performance, power-loss behavior, native-host Rust and remote CI remain unvalidated. No required implemented-scope check remains blocked.

The Markdown check initially found four existing links to HANDOFF_MARCO_1.pdf, which is absent from this checkout and tracked files. Current navigation now links the existing historical Markdown source; historical PDF generation records remain unchanged.

Final diagnostic coverage extension: `cargo test --locked --test persistence_contract` passed all 5 cases including CommitUncertain redaction/source retention; final formatting and strict Clippy passed again.

## Static documentation audit — 2026-10-09

Rechecked HTTP composition, read-only SQL, publisher confirmation/uncertainty, pure ownership models/ports, lease migration and CI definitions. Removed stale current claims about absent broker dependencies or unstarted Milestone 2, added ADR 006/007 navigation and aligned bilingual current scope. Ownership SQL mutations, orchestration and polling remain deferred; no end-to-end delivery guarantee is established. Default/opt-in tests, CI jobs, migrations, Cargo commands, broker and PostgreSQL were not executed. Historical milestone results remain unchanged.

Static local-link/anchor, fence, language-pair and documentation-only SHA-256 checks are recorded in the shared [interview review](../../../../engineering-library/docs/en/testing/verification.md). Runtime environment files and secret-bearing backups were not read or modified.

## Milestone 2.3 — 2026-10-12

Starting checkout: clean `bbc2c1f`; no commit/push. Existing healthy app, PostgreSQL and RabbitMQ containers reused without restart/rebuild/reset. No dependencies or shared publisher code changed. Commands ran through `docker compose exec -T app` unless noted. New [scope/semantics](../architecture/milestone-2-3.md) and Portuguese continuity report (file absent in the 2.4 checkout) describe limitations separately from evidence.

| Check actually executed | Result |
| --- | --- |
| cargo fmt --check | Passed |
| cargo clippy --locked --all-targets --all-features -- -D warnings | Passed, all final targets/features including the proxy |
| cargo test --locked | 33 non-ignored tests passed; opt-in infrastructure cases executed separately |
| cargo test --locked --test postgres_ownership -- --ignored --test-threads=4 | 12 real PostgreSQL cases passed |
| cargo test --locked --test postgres_repository -- --ignored --test-threads=4 | 15 existing regressions passed |
| cargo test --locked --test delivery_ownership_schema -- --ignored | 1 existing schema regression passed |
| cargo build --locked | Passed |
| RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps | Passed |
| Read-only development migration/schema metadata and fixture database catalog | Only original migrations installed; ownership column absent; zero relay_it_ databases remain |

Initial Clippy failed because a test binding shadowed the request helper; renamed it. First eight-case integration run passed seven cases and failed the lock-wait transaction-start assertion. The coordinator held a cached activity snapshot; explicit pg_stat_clear_snapshot refresh fixes the evidence probe. The corrected suite passed, then the final expanded 12-case suite passed. No production code was altered to weaken the asserted authority semantics.

Real tests demonstrate concurrent disjoint acquisitions, locked-row skipping, post-lock expiration rejection for completion/release, stale-token rejection after recovery, malformed selected data/checked duration overflow without mutation, bounded counter exhaustion, stable envelope/unrelated metadata, atomic clearing and repeated/missing/terminal conflicts. A future-timestamp fixture tests the acquired_at lower bound; it is not a real clock reversal. A one-microsecond lease is already expired at post-return observation. Existing deterministic model tests were rerun for exact expiration equality, which is separate from integration evidence around real expiry.

Immediate SQL rejection and deferred constraint rejection at COMMIT both preserve rows and return OperationFailed. A test-owned protocol proxy discards the server's COMMIT completion response for all three operations; each returns CommitUncertain while independent durable-state inspection confirms the commit. This proves one committed uncertain scenario, not that every uncertain result implies commit or rollback. Cancelled pre-mutation lock wait leaves the fixture unchanged and pool usable. Cancellation during COMMIT, crash/network failure before observed server completion, every concurrency/isolation schedule, fairness, production performance/power-loss durability, native-host Rust and remote CI remain unverified.

Ownership migration was applied only in test-owned databases and remains pending in development; no new migration or schema edits. Real RabbitMQ opt-in regressions were not rerun because no shared publisher/dependency change or applicable instruction required them; default publisher tests passed. Prior validation records above remain historical.

Documentation scope: Rust READMEs/catalogs, architecture/contracts/failures, schema/migration/repository guidance, security/deployment updates, strategy/results, bilingual 2.3 page and the Portuguese handoff. Engineering Library updated only its bilingual existing dossier/interview/catalog/verification pages, preserving historical review and distinguishing general lease/locking/uncertainty principles from Rust choices. Documentation validation results are appended after final checks below.

Final documentation checks: `python3 -B engineering-library/tools/check_docs.py --workspace .` from the sibling workspace passed: 212 documents, zero errors and 18 pre-existing link-blocked fixture warnings. This checks Rust and Library language pairs, indexes, relative links/anchors and interview structure; external links are not fetched. A separate check of `Reports/MILESTONE_2_3_HANDOFF.md` passed every relative file/anchor link because Reports is outside the catalog scan. `git diff --check` passed separately in rust-event-relay and engineering-library. No Library application tests/runtime commands were executed; its eight changed files are Markdown only.

## Milestone 2.4 — 2026-10-12

Started from clean `edb1f90`; the previous milestone was committed. The requested 2.3 handoff is absent from this checkout. Dangling links to it were corrected to existing milestone/validation evidence, retaining historical validation prose. [Current use-case scope](../architecture/milestone-2-4.md) and 2.4 handoff (absent in the 2.5 checkout) describe implemented behavior and evidence limits.

Existing app/PostgreSQL/RabbitMQ containers were inspected and reused without restart/rebuild/reset. Read-only development metadata shows successful migrations `20261007000000` and `20261007175358`, with no ownership_token column: ownership migration remains pending. No migration was applied and no event rows or credentials were printed. Infrastructure source, dependencies and schema were unchanged; opt-in infrastructure regressions were not rerun without a relevant change.

| Executed command | Result |
| --- | --- |
| `docker compose exec -T app cargo fmt --check` | Passed |
| `docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings` | Passed |
| `docker compose exec -T app cargo test --locked --test delivery_use_case` | 5 passed |
| `docker compose exec -T app cargo test --locked` | 38 passed, 31 opt-in tests ignored; no failures |
| `docker compose exec -T app cargo build --locked` | Passed |
| `docker compose exec -T -e RUSTDOCFLAGS='-D warnings' app cargo doc --locked --no-deps --all-features` | Passed |
| `python3 -B engineering-library/tools/check_docs.py --workspace .` from parent workspace | 214 documents, zero errors, 18 pre-existing restricted-fixture warnings |
| Separate relative-link check for Reports/MILESTONE_2_4_HANDOFF.md | Passed; Reports is outside the catalog scan |
| `git diff --check` in both repositories | Passed |

Five fake tests iterate all five publication categories, all transition outcomes/error categories and all acquisition failure categories. They prove complete original-envelope equality, single acquisition/publication calls, correct owner key, confirmation-before-completion, absence of forbidden cleanup/retry, preserved original/source and release errors, sanitized Debug and Send/static dispatch. Explicit Notify synchronization tests waiting for publisher confirmation and cancellation during acquisition/publication/completion/release. They do not establish actual PostgreSQL rollback, RabbitMQ nonacceptance, commit cancellation behavior or end-to-end crash recovery. No application test failed; incomplete documentation links were resolved during drafting. No commit/push. Next: 2.5 worker composition and operation; 2.6 end-to-end failure experiments.

## Milestone 2.5 — 2026-10-12

Executed in the existing Docker environment from clean Rust `7be956c` / Library `769bb66`. The requested 2.4 handoff was absent; actual API/tests/history were inspected. No shared migration/reset/credential changes. Only app image rebuilt and app container recreated with `docker compose build app` / `docker compose up -d --no-deps app`; PostgreSQL/RabbitMQ remained running.

| Executed check | Result |
| --- | --- |
| cargo fmt --check | Passed |
| cargo clippy --locked --all-targets --all-features -- -D warnings | Passed |
| cargo test --locked | 47 passed, 34 opt-in ignored, no failures |
| cargo build --locked --bins | Passed, both compiled programs |
| RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps | Passed |
| postgres_repository --ignored --test-threads=4 | 15 passed |
| postgres_ownership --ignored --test-threads=4 | 12 passed |
| delivery_ownership_schema --ignored | 1 passed |
| rabbitmq_publisher --ignored | 2 passed |
| library closed_owned_connection_is_unavailable --ignored | 1 passed |
| worker_smoke --ignored --test-threads=1 | 3 passed: binary delivery/SIGTERM, isolated Supervisor controls, sanitized missing-schema setup |
| python3 scripts/check-supervisor.py inside app | HTTP named/interactive controls, unexpected-exit restart, manager/private socket and relay STOPPED passed |

Commands use `docker compose exec -T app` and Cargo `--locked`; broker/library ignored cases explicitly selected. Admission usability and missing-schema tests were validated with all standard checks. The final setup query checks every column used by ownership operations; its composition/schema smoke, strict Clippy, formatting, build and rustdoc were rerun. Infrastructure regression results above are this session's executions, not historical claims. Formatting/temp-shutdown-borrow and unused-import test corrections were resolved before final checks.

Read-only `sqlx migrate info` before and after app recreation: two original migrations installed, `20261009000000` still pending. Supervisor reports http RUNNING / relay STOPPED. Actual shared relay startup was not attempted; the negative schema check and real named/collective relay controls used isolated test-owned state. `all` now includes both programs, so the shared HTTP script intentionally avoids `start all`.

Nine controlled worker/config tests use paused Tokio time and notifications. They cover every outcome decision and independent release combinations, fresh tokens, paced idle/failure, replacement only where warranted, no acquisition with missing/known unusable publisher, interrupted waits, preexisting shutdown, in-flight drain and bounded cleanup. The fake stalled close consumes the same absolute overall deadline; it proves policy, not every real SQLx shutdown schedule. Three opt-in compositions own generated databases/exact UUID broker names; the real binary preserves event ID/payload, confirms and acknowledges processed with one acquisition. Test child/daemon cleanup precedes exact queue/exchange deletion and test database removal. Supervisor test derives its temporary physical config from versioned source, preserves autostart false, uses an ephemeral HTTP binding and tests both aliases with stop/start/restart of relay/http/all. It is a local Linux container check, not a new Supervisor CI job.

Configuration check: Compose validation and installed/source Supervisor equality plus executable launchers; shared read-only migration/status and HTTP probe. Documentation checker from sibling tools validates 216 documents, zero errors and 18 existing restricted-fixture warnings; handoff (2.5 report absent from this checkout) links checked separately. git diff --check passes in both projects. Library changes are Markdown only; application/runtime tests or remote GitHub Actions execution were not claimed.

No throughput/fairness/consumer-effect/end-to-end at-least-once claim follows. Happy path, fake cancellation and isolated named operation do not establish crash-window or two-worker recovery: those remain 2.6. [Current operation, settings and rollout](../architecture/milestone-2-5.md) describes budgets/limits. Shared migration/application startup is deliberately pending authorization; no UI, readiness framework, metrics or dashboard added.


## Milestone 2.6 — 2026-10-12

Started clean at Rust `d6f529b` and Library `88f8e62`. Missing 2.5 handoff was not reconstructed; versioned code/docs/history provided the baseline. [Matrix and actual observations](../architecture/milestone-2-6.md), [acceptance review](../operations/milestone-2-review.md), handoff (historical handoff unavailable in this checkout) and sanitized final transcript (historical artifact unavailable in this checkout) record the evidence.

| Executed command in existing app (`docker compose exec -T app`) | Actual result |
| --- | --- |
| cargo fmt --check | Passed |
| cargo clippy --locked --all-targets --all-features -- -D warnings | Passed |
| cargo test --locked | 47 passed, 45 opt-in ignored, no failures |
| cargo test --locked --test integrated_recovery -- --ignored --test-threads=1 --nocapture | 11 passed, including one internal no-op entry; ten experiment tests; final suite 27.09 s |
| cargo test --locked --test postgres_ownership -- --ignored --test-threads=4 | 12 passed |
| cargo test --locked --test postgres_repository -- --ignored --test-threads=4 | 15 passed |
| cargo test --locked --test delivery_ownership_schema -- --ignored | 1 passed |
| cargo test --locked --test rabbitmq_publisher -- --ignored | 2 passed |
| cargo test --locked --lib infrastructure::rabbitmq::tests::closed_owned_connection_is_unavailable -- --ignored | 1 passed |
| cargo test --locked --test worker_smoke -- --ignored --test-threads=1 | 3 passed; isolated physical Supervisor included; 22.81 s |
| cargo build --locked --bins | Both binaries built |
| cargo doc --locked --no-deps --all-features with RUSTDOCFLAGS='-D warnings' | Passed |

Compose configuration passed `docker compose config --quiet`; Ruby/Psych parsed the workflow and its four jobs. Supervisor configuration/launchers were unchanged; its isolated regression passed. New combined CI services/job are configured, not remotely executed. An intermediate Clippy error from test helper import/unused function was corrected; no production defect was confirmed. The deliberate cleanup assertion is expected and the enclosing regression passes only after independent absence checks.

Real expiration, durable states and original broker identities support the tested recovery schedules; finite tests are not universal liveness/exactly-once proof. Broker disconnect during delivery used a real worker with test-scoped lifecycle; crash barriers used an instrumented test executable with real use case/adapters; restarted workers, concurrency and signal scenarios used the unmodified relay binary. Response-loss proxy demonstrates one already-committed uncertain completion. No source/schema/dependency changes, durable retry/quarantine/replay or producer/consumer business behavior added.

Final read-only environment checks: original services continuously running; http RUNNING, relay STOPPED; two original migrations installed, ownership migration 20261009000000 pending. Zero fixture databases, no fixture queues/exchanges, no test worker/helper process or relay_recovery_ directory remained. No shared migration, service restart/recreation, queue purge or unrelated session termination. Exact cleanup also passed after intentional assertion failure. Shared rollout remains separately unauthorized.

Final documentation validation: read-only Library validator from parent workspace checked 220 documents, zero errors, 18 existing restricted-fixture warnings. Handoff relative file/anchor links checked separately because Reports is outside the catalog scan. git diff --check passed in both repositories. Library changes are Markdown only; no Library application/dependency/runtime edits, no Viva Bem edits, no commit/push.


## Milestone 3.1 — 2026-10-12

Starting Rust commit: `15067056f379dd98aeb3bd5c6af09b6700504281`; engineering-library: `7caf977d5f3294c05efa1b29f21edff1b8107ca8`. Both trees were clean; no applicable project/ancestor AGENTS.md found. Available skills were considered; no artifact/plugin skill was needed for focused Rust policy and Markdown work.

Executed in the existing app container: cargo fmt --all (then --check), cargo clippy --locked --all-targets -- -D warnings, cargo test --locked, cargo build --locked and RUSTDOCFLAGS=-Dwarnings cargo doc --locked --no-deps. Final checks passed: 50 tests passed, 45 ignored, no failures; three new pure policy tests cover acquisition/cycle/storage boundaries, capped equal jitter, invalid inputs, precision and timestamp overflow. Initial Clippy attempts rejected manual zero-clamping of derived allowance; the final expression clamps only the derived remaining allowance, never lifetime counters.

Documentation validator was run across both repositories/workspace: zero errors, 18 restricted-target warnings in existing Go image fixture links, none in either changed repository. git diff --check passed in both repositories. These checks do not prove SQL scheduling, contention, isolation or replay; none is wired. Existing acquisition/use-case/worker categories were reviewed against the new matrix.

Read-only docker compose ps confirmed existing app/frontend up and PostgreSQL/RabbitMQ healthy. No shared migration/status query, credential inspection/change, service restart, shared relay start or data reset occurred. Opt-in infrastructure/recovery tests were not rerun because only independent pure policy code and documentation changed. Historical Milestone 2 evidence remains historical; CI was not remotely executed. No staging, commit or push.

[Policy and remaining slices](../architecture/milestone-3-1.md) | [ADR 008](../adr/008-durable-failure-policy.md).

## Milestone 3.2 — 2026-10-12

Base commits: rust-event-relay `2257d02543958eff5851a49d7be16887c0b68d6a`; engineering-library `d9f6e556258a9a1739dcb9ad9df7f835f4511853`. Both Git trees were clean at entry; no applicable project/ancestor AGENTS.md was found. Current ADRs, policy, migrations, models, ownership SQL, isolated harness/fault proxies and regression tests were inspected. Dev Career Copilot instructions and the existing documentation methodology were consulted; PostgreSQL primary documentation verified CHECK NULL and clock/timestamp semantics.

Executed locally in the existing app container; final checks passed:

- cargo fmt --check; cargo clippy --locked --all-targets --all-features -- -D warnings.
- cargo test --locked: 53 passed, 63 opt-in cases ignored, zero failures.
- cargo test --locked --test postgres_retry -- --ignored --test-threads=4: 17 passed.
- PostgreSQL regressions: postgres_ownership 12, postgres_repository 15 opt-in, delivery_ownership_schema 1; all passed in isolated databases. The database-independent repository case also passed in standard tests.
- integrated_recovery: 11 passed (10 experiments plus the no-op internal child entry), sequential execution against exact test-owned database/broker resources.
- worker_smoke: original three cases passed, including isolated Supervisor collective control; the new missing-retry-schema preflight case passed. Both missing-schema cases were checked again after the helper update. No shared worker startup.
- cargo build --locked --bins; RUSTDOCFLAGS=-Dwarnings cargo doc --locked --no-deps.
- Bilingual documentation validator: 230 documents across its configured workspace, zero errors, 18 existing restricted Go-fixture warnings; no findings in either changed repository. git diff --check passed in both repositories.

Initial checks identified a Clippy item-order issue and old fixture assumptions about revision/terminal metadata. These were corrected without removing existing assertions. Legacy terminal_at backfill was refined to migration observation time rather than inventing a historical failure time. Final SQL tests cover that observation and preservation of every original field. New uncertainty/cancellation experiments reuse the existing proxy with a controlled server-COMMIT barrier; observed durable writes do not generalize every unknown outcome into success.

Read-only environment inspection confirmed app bind `/app`, PostgreSQL bind `/var/lib/postgresql`, preserved RabbitMQ volume `/var/lib/rabbitmq` and configuration bind. Shared SQLx history still contains only `20261007000000` and `20261007175358`, both successful; shared outbox has zero rows and shared relay is STOPPED. Ownership/retry migrations remain pending there; up/down ran only in isolated databases. No test databases remained at catalog inspection. No shared reset, migration, broker credential change, application restart or relay activation occurred in this milestone. Supervisor source directories were corrected from the obsolete `/app/Backend` to `/app/backend`; isolated lifecycle tests verified the source configuration without rebuilding/restarting the shared app.

CI now includes the new PostgreSQL target but was not remotely executed. Pipeline policy integration, general poison scanning, authenticated audited replay and integrated Milestone 3 closure remain 3.3/3.4/3.5/3.6 respectively. No staging, commit or push. The temporary continuation report is returned in chat, not stored in either repository.

[Implementation and rollout](../architecture/milestone-3-2.md).

## Milestone 3.3 — 2026-10-13

Base Rust `e6f8a2f44d9ac250e1bcdf5f0b6a3b5b7bd5f1eb`; Library `303ed8602aa5727e2a47bf02a2ef7f0856e21760`, both initially clean. No applicable AGENTS.md or supplied continuation report. Historical entries above were not counted as current checks.

Executed in the existing app container (Rust 1.95.0), using harness-owned UUID databases, unique broker topology and owned child/proxy processes:

- Standard tests: 60 passed; opt-in tests separately executed below. Controlled delivery tests: 8; worker tests: 12; retry policy tests: 4. No flaky statistical jitter checks.
- PostgreSQL retry: 18 passed, including injected Rejected publication through deliver_once with real schedule/terminal COMMIT acknowledgement loss. The publisher is a fake in those two transition experiments, explicitly not a broker NACK.
- Integrated recovery: 12 real cases passed plus internal boundary_child entry (no-op without RECOVERY_CHILD). Includes real unroutability → persisted schedule → early acquisition denied → repaired route/fresh token/original identity completed, terminal final failure, and configured worker maintenance protecting active ownership/unrelated delivery.
- PostgreSQL ownership: 12; repository: 15 opt-in; ownership schema: 1; RabbitMQ publisher: 2; worker smoke: 4; closed-connection adapter regression: 1, all passed. Across opt-in targets: 65 substantive cases plus the internal no-op entry.
- cargo fmt -- --check, strict cargo clippy --locked --all-targets -- -D warnings, cargo build --locked --bins and RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps passed.
- Documentation validator unit suite: 18 passed. Final read-only documentation validator: 232 documents, zero errors and 18 existing restricted-fixture warnings; git diff --check passed in both repositories.

Corrected during this work: strict Clippy caught a test MutexGuard scope spanning await; the test now clones the bounded observations before suspension. The first interim documentation scan found four missing new validation anchors while these entries were still being written; final validation includes them. No production defect requiring SQL/schema/protocol changes was found. Existing recovery acquisition counts remain unchanged; the new scheduled-recovery case uses two committed acquisitions because the first definitively fails and the second completes after durable eligibility returns.

Read-only environment inspection: PostgreSQL and RabbitMQ healthy; HTTP RUNNING, shared relay STOPPED/Not started. sqlx migrate info reports only 20261007000000/20261007175358 installed; 20261009000000 ownership and 20261012000000 durable retry pending. Shared migrations were not applied; no shared service stop/start/rebuild, credentials or volumes changed. Only isolated fixtures apply migrations. Remote CI was not executed. No staging, commit or push. Library changes are documentation only. No Viva Bem modifications.

Limitations: generic serialization lacks event/global context and conservatively retains ownership. Finite configured maintenance supplies no discovery or general backlog coverage, pauses without a usable publisher and has no durable restart cursor; stale configuration must be removed and metadata re-observed. No generic uncertain-result reconciliation or consumer idempotency exists. [Contracts and exact remaining 3.4 scope](../architecture/milestone-3-3.md).

Commands executed:

```bash
docker compose exec -T app cargo test --locked
docker compose exec -T app cargo test --locked --test postgres_retry --test postgres_ownership --test postgres_repository --test delivery_ownership_schema --test rabbitmq_publisher --test worker_smoke --test integrated_recovery -- --ignored --test-threads=4
docker compose exec -T app cargo test --locked --lib closed_owned_connection_is_unavailable -- --ignored
docker compose exec -T app cargo test --locked --test integrated_recovery configured_worker_maintenance -- --ignored --nocapture
docker compose exec -T app cargo clippy --locked --all-targets -- -D warnings
docker compose exec -T app cargo build --locked --bins
docker compose exec -T app sh -c 'RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps'
docker compose exec -T app cargo fmt -- --check
python3 -B ../engineering-library/tools/check_docs.py --workspace ..
python3 -B -m unittest discover -s ../engineering-library/tools -p 'test_check_docs.py' -v
git diff --check
git -C ../engineering-library diff --check
```

## Local relay autostart rollout — 2026-10-13

Bases: Rust cae62cc654efc0b411408dd84a0b92a57738eb38; Library c272476429fe4c9223f274d74293edeabb8c79b5, both clean. User explicitly authorized local migration/activation. Reviewed current Supervisor/Compose/adapter/schema preflight and both additive migration SQL files. Shared outbox had zero rows/exhausted events; initial migrations only were installed. Authenticated host management verified existing credentials without disclosure.

Explicit sqlx migrate run applied 20261009000000 and 20261012000000; all four migrations now installed/successful, nine retry columns present, outbox still empty. Only app rebuilt/recreated using docker compose up -d --build --no-deps app; existing PostgreSQL, broker, frontend, credentials and storage preserved. No automatic startup migrations, data reset, volume deletion, queue purge or demonstration publication. Frontend source diff is empty.

Executed: automatic http/relay RUNNING; authenticated management page/API at localhost:15672 and configured durable exchange/queue/binding; direct/interactive controls through supervisorctl and supervisor; stop relay remained stopped while HTTP and broker management/queue metadata/counts remained available; start resumed; SIGKILL of each running child triggered unexpected-exit recovery. Existing graceful shutdown limits unchanged. Shared queue observed zero ready/unacknowledged messages; nonempty payload persistence was not tested by inventing messages. Browser form login not operated.

Strict Clippy all targets passed; cargo fmt check passed; four isolated worker_smoke tests passed, including autostart from physical Supervisor config and real test-owned database/broker resources. Standard suite: 60 passed. Final read-only documentation validator: 234 documents, zero errors and 18 existing restricted-fixture warnings; git diff --check passed in both repositories. Host management/API and HTTP health were also reverified while relay was intentionally stopped, then relay was restored to RUNNING. Historical 3.3 integration executions retain their own scope; remote CI not run. No staging/commit/push. [Current local instructions](../operations/local-relay-rollout.md).

Commands executed:

```bash
python3 scripts/check-rabbitmq.py
python3 scripts/check-rabbitmq.py --require-topology
docker compose exec -T app sqlx migrate info
docker compose exec -T app sqlx migrate run
docker compose up -d --build --no-deps app
docker compose exec -T app python3 /app/scripts/check-supervisor.py
docker compose exec -T app supervisorctl stop relay
docker compose exec -T app supervisorctl start relay
docker compose exec -T app supervisor status
docker compose exec -T app cargo clippy --locked --all-targets -- -D warnings
docker compose exec -T app cargo test --locked --test worker_smoke -- --ignored --test-threads=4
docker compose exec -T app cargo test --locked
python3 -B ../engineering-library/tools/check_docs.py --workspace ..
git diff --check
git -C ../engineering-library diff --check
```


## Milestone 3.4 — executed local validation, 2026-10-13

Base Rust a756797bbdd64d50671d536ae7de697a3705ce8b; Library c272476429fe4c9223f274d74293edeabb8c79b5. Rust initially clean; Library already contained eight modified documentation files, preserved. [Detection, authority and limits](../architecture/milestone-3-4.md).

Passed: final standard all-target suite 65 tests; three deterministic maintenance tests and 14 worker tests are included in that count. Real isolated PostgreSQL: discovery 7, retry 18, ownership 12, reader 15 and ownership-schema migration 1. Worker/Supervisor smoke 4 passed in isolated resources. Full integrated recovery target 15 passed (including its boundary-child/harness cases); the two new discovery/publication cases were also rerun successfully against final binaries after the final equivalent identity-check refactor and application admission guard. Strict Clippy all targets, rustdoc with -D warnings, both binary builds and final formatting check passed. The new discovery target is added to CI configuration; remote CI was not executed.

New evidence covers mixed invalid/budget/capacity/valid rows, raw-content/history preservation, continuation/high-watermark/wrap, concurrent insert/state change, unsafe-prefix traversal, exact selection revalidation, held-row skip, competing isolation, stale cycle/revision, unversioned valid envelope repair, live final owner/completion, expired ownership, unsupported schema/timestamp, unsafe revision/ownership, closed-pool failure, deferred commit rejection, lost commit acknowledgement and post-commit cancellation with separate durable observation. Deterministic tests cover no mutation on observation/error, cursor consumption, worker outage maintenance, interruptible pacing and no maintenance catch-up bursts. Real worker leaves nil identity pending/intact, isolates whitespace poison and delivers original valid ID; database processed state and broker message are observed separately. Test-owned schema constraints are dropped only in the explicit malformed fixture; production constraints are unchanged.

Read-only operational checks: Compose configuration valid; http/relay still RUNNING, PostgreSQL healthy/authenticated and current running settings match Compose. Four existing SQLx migrations are successful, including 20261009000000 and 20261012000000. No new/shared migration, service restart, credential change, storage reset or shared queue purge. Existing containers reused. Frontend and Viva Bem untouched. No staging, commit or push. The new behavior is validated in isolated workers; the already-running shared relay was not restarted to load it.

Documentation validator: 236 documents, zero errors, 18 existing restricted-fixture warnings; validator's 18 unit tests passed. git diff --check passed in both projects. These finite tests do not prove fairness/full backlog coverage, byte bounds, power-loss behavior, every concurrency schedule, exactly-once or consumer idempotency. Audited replay and full milestone closure remain 3.5/3.6.

Executed commands (existing app container; each ignored suite explicitly opted in):

```bash
docker compose exec -T app cargo test --locked --all-targets --quiet
docker compose exec -T app cargo test --locked --test postgres_discovery --test postgres_retry --test postgres_ownership --test postgres_repository --test delivery_ownership_schema --test worker_smoke -- --ignored --test-threads=1
docker compose exec -T app cargo test --locked --test integrated_recovery -- --ignored --test-threads=1
docker compose exec -T app cargo test --locked --test integrated_recovery discovered_poison -- --ignored --test-threads=1
docker compose exec -T app cargo test --locked --test integrated_recovery real_worker_discovers -- --ignored --test-threads=1
docker compose exec -T app cargo clippy --locked --all-targets -- -D warnings
docker compose exec -T app sh -c 'RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps'
docker compose exec -T app cargo build --locked --bins
docker compose exec -T app cargo fmt --all -- --check
docker compose config --quiet
docker compose exec -T app supervisor status
sh scripts/check-postgres.sh
python3 -B ../engineering-library/tools/check_docs.py --workspace ..
python3 -B -m unittest discover -s ../engineering-library/tools -p 'test_check_docs.py' -q
git diff --check
git -C ../engineering-library diff --check
```

## Milestone 3.5 — 2026-10-13

[Executed replay checks and operational limits](../architecture/milestone-3-5.md#executed-validation). Migration 5/shared privileged activation remain pending; independent isolated database/broker tests do not prove consumer business effects or remote CI execution.


## Current Milestone 3.6 acceptance — 2026-10-13

[Full-project audit, traceable acceptance matrix, actual validation counts, operational gaps and study path](../operations/milestone-3-review.md). Milestone 3 closed with explicit nonblocking limitations; shared replay rollout, consumer idempotency, live frontend integration and production readiness remain separate. Earlier sections retain their dated historical scope.


## Milestone 4.1 — 2026-10-13

Existing app container, separate CARGO_TARGET_DIR=/tmp/relay-m41-target: cargo fmt --check; cargo clippy --locked --all-targets --all-features -- -D warnings; cargo test --locked (75 passed, 0 failed, 88 ignored, including six new pure consumer tests); cargo build --locked --bins; RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps. Bilingual workspace documentation validator: 0 errors, 18 warnings restricted to unrelated Go image fixtures. git diff --check passed in both repositories.

Consumer tests cover identifier/quantity/stock bounds, supported type/version, coherent identity, immutable comparisons, outcome ACK policy, body/property bounds and duplicate quarantine publication after simulated crash/uncertain source ACK. The recorder is a fake: no actual database atomicity, RabbitMQ preservation, return/confirm correlation or crash recovery evidence. Infrastructure regression suites remain ignored and were not invoked: no adapter, migration or runtime changed. Docker services were inspected/reused, without restart, shared message consumption, topology/credentials changes or migrations. Engineering Library is documentation-only. [Contracts and planned real experiments](../architecture/milestone-4-1.md).

## Milestone 4.2 — actual local execution, 2026-10-13

Baseline Rust `4d3734c79b3d635a91a4e7d415a903b18b43a610`; Library `281231333240e20a9704e15bb9fbcdaeb5ed151c`; both initially clean. Docker existing app uses Rust 1.95.0, pinned SQLx 0.8.6 and PostgreSQL 18.6. No service rebuild/restart or shared write/consumption.

- `cargo fmt --check`: passed on final source.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: passed on final source.
- `cargo test --locked`: standard suite passed; database/broker tests retain their explicit ignored gating.
- `cargo test --locked --test postgres_consumer -- --ignored --test-threads=1`: **10 passed**, 0 failed. Includes deterministic observed stock/identity contention, existing relay envelope/history upgrade preservation, nanos/UTC-offset representation comparison, constraints and test-only corruption, statement/deferred rejection, rollback, overflow, historical duplicates, commit-loss proxy and cancellation.
- Existing affected isolated suites: postgres_repository **15**, delivery_ownership_schema **1**, postgres_ownership **12**, postgres_retry **18**, postgres_discovery **7**, postgres_replay **10** passed (63 total). Existing repository opt-in fault case remains filtered by its gate.
- `cargo build --locked --bins`: both HTTP and relay binaries built. `RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps`: passed.
- `python3 scripts/check-migrations.py`: six staged SQLx upgrades/checksums, no-op, consumer and replay down/re-up passed; exact generated database removed and absence verified. Consumer SQL down/up also tested with stock/receipt data, demonstrating loss. Harness independently verifies cleanup of every exact test database.
- Engineering Library documentation validator: 0 errors, 18 existing restricted fixture-link warnings; its 18 synthetic tests passed. Migration checker Python syntax passed. `git diff --check` passed in both repositories.

CI now runs the consumer suite in its PostgreSQL job; no remote CI execution is claimed. Frontend/broker/integrated crash acceptance were not changed or rerun. Shared read-only inspection reports four installed versions (20261007000000, 20261007175358, 20261009000000, 20261012000000); replay 20261013000000 and consumer 20261014000000 remain pending. Shared Supervisor HTTP PID 11 and relay PID 12 remained running; disposable-database catalog observation returned zero remaining relay_it_/relay_migration_ databases. No staging, commit or push.

[Transaction, trigger rationale, uncertainty limits and exact 4.3 prerequisites](../architecture/milestone-4-2.md). Cancellation/protocol faults are finite local evidence, not process-kill, power-loss, throughput or production-readiness proof. Earlier validation remains historical.
