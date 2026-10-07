[Português brasileiro](../pt-BR/validation-results.md) | [README](../../README.md)

# Validation results

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

The existing cluster's credential mismatch was observed directly; its real username/password are omitted. Validation used a separate `relay-milestone13-validation` Compose project with a /tmp data bind mount and loopback port 15433. The fixture's example-only configuration and exact Compose function are in [testing](testing.md#isolated-validation-fixture). No developer data directory was deleted. The fixture containers/network were removed at the end; temporary fixture data remains under /tmp. The original development environment remains running.

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
