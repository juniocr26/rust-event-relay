[Português brasileiro](../pt-BR/validation-results.md) | [README](../../README.md)

# Validation results

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
python3 - <<'PYTHON'
import socket
from pathlib import Path
with socket.create_connection(('127.0.0.1',5433), timeout=5):
 print('Host TCP 127.0.0.1:5433 reachable')
p=Path('.dockerized-postgres/18/docker/PG_VERSION')
print('Host PG_VERSION:',p.read_text().strip())
PYTHON
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
python3 - <<'PYTHON'
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
PYTHON
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
python3 - <<'PYTHON'
from pathlib import Path
import shutil
for name in ['.cargo-cache', 'target']:
    p = Path(name)
    assert p.is_dir() and not p.is_symlink()
    shutil.rmtree(p)
PYTHON
docker compose up -d
docker compose exec -T app bash -c 'cargo fetch --locked && cargo build --locked && cargo test --locked && cargo fmt --check && cargo clippy --locked --all-targets --all-features -- -D warnings'
docker compose exec -T app cargo test
```

The unit tests cover default configuration and rejection cases. The HTTP test uses a real socket and an injected shutdown trigger. The subprocess test supplies a temporary `.env` outside the repository and validates both Unix signals. Git ignore rules were checked with `git check-ignore .env .cargo-cache/ target/ .vscode/`; relative Markdown links were resolved against the filesystem. A review confirmed that relay/durability features are marked as planned.

## Foundation limitations (previous validation)

Native host Rust, Windows signal behavior, Linux host UID/GID portability, GitHub-hosted CI execution and production deployment were not validated. No PostgreSQL, RabbitMQ, Redis, webhook delivery, retries, idempotency, backpressure or crash recovery was implemented or tested. There are no benchmarks or throughput/latency claims. Health indicates only HTTP liveness. The development workspace container remains running after validation; the temporary live HTTP process ended when Compose was stopped for recovery. Start the service with `docker compose exec app cargo run --locked`.
