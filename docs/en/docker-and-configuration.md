[Português brasileiro](../pt-BR/docker-and-configuration.md) | [README](../../README.md)

# Docker and configuration

The Dockerfile is a development toolchain image, not a production deployment. It pins stable Rust 1.95.0 on Debian Bookworm, includes Bash, rustfmt and Clippy, and runs as `developer`. The version tag is pinned; the base image digest is not, so upstream image revisions may still change system packages. Compose uses an init process and a source bind mount. `sleep infinity` keeps the workspace usable even before dependencies have been downloaded.

```bash
cp .env.example .env
# On Linux: set LOCAL_UID=$(id -u) and LOCAL_GID=$(id -g) values in .env.
docker compose up -d --build
docker compose exec app bash
# Inside the shell:
cargo fetch --locked
cargo build --locked
cargo test --locked
cargo fmt
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo run --locked
```

Use Ctrl-C to stop the foreground application and `docker compose down` to stop the development environment. Changes to source need a new run; changes to UID/GID or Dockerfile need image rebuild. On Linux the mount root must be writable by the configured UID/GID. Do not run initial Cargo commands as root. Numeric IDs default to 1000; choose a non-root ID. Existing IDs in the base image may cause user/group creation to fail: choose compatible IDs or adapt the user creation for your environment. Docker Desktop translates macOS/Windows mounts differently; Linux ownership behavior has not been tested on those systems. Windows contributors should use a shell supporting the documented commands (e.g. WSL). Bash is available via `docker compose exec app bash`.

| Variable | Default | Use |
| --- | --- | --- |
| APP_ENV | development | Label in startup logs; does not change delivery behavior |
| RUST_LOG | info | tracing filter, invalid filters fail startup |
| HTTP_ADDR | 0.0.0.0:8080 | IP and port; invalid socket addresses fail startup |
| HOST_HTTP_PORT | 8080 | Compose host loopback port |
| LOCAL_UID / LOCAL_GID | 1000 / 1000 | Build-time developer identity |
| CARGO_HOME | /app/.cargo-cache | Container Cargo source/download cache |
| CARGO_TARGET_DIR | /app/target | Container compiled artifacts |

The first three are application settings. The remaining settings belong to development tooling. There are no mandatory connection strings. Compose now supplies a PostgreSQL DATABASE_URL, but the Rust binary does not consume it. RabbitMQ/Redis URLs, worker concurrency, retry and batch settings remain planned.

The binary loads `.env` from its working directory or ancestors with dotenvy; already-set process variables win. Compose separately reads root `.env` for interpolation (ports/build IDs); it does not inject all values into the container. The source mount exposes `.env` for the binary to load at runtime. To override explicitly: `docker compose exec -e RUST_LOG=debug app cargo run --locked`. An absent `.env` is valid; malformed `.env`, empty APP_ENV, invalid HTTP_ADDR or RUST_LOG, and occupied sockets cause failure.

Keep `.env` and cache credentials out of Git and Docker build context. Do not place secrets in `.env.example`. The port is published on host loopback only; changing HTTP_ADDR to container loopback prevents host access. Keep port 8080 in the container or adjust the Compose container port as well. `/health` is liveness only, with no readiness dependency checks.

## Reconstructing the local environment

```bash
git clone https://github.com/juniocr26/rust-event-relay.git
cd rust-event-relay
cp .env.example .env
# Linux: align LOCAL_UID and LOCAL_GID before building.
docker compose up -d --build --wait --wait-timeout 120
docker compose ps
docker compose logs postgres
docker compose exec postgres pg_isready -h 127.0.0.1 -p 5432 -U relay -d reliable_event_relay
docker compose exec postgres psql -U relay -d reliable_event_relay -c "SELECT 1;"
docker compose exec app cargo fetch --locked
docker compose exec app cargo build --locked
docker compose exec app cargo run --locked
```

Compose creates a project network. `app` resolves `postgres` through Docker DNS. PostgreSQL listens internally on 5432; only the host mapping uses 5433 to avoid the other project's 5432. `app` waits for PostgreSQL's healthcheck without fixed sleeps. The workspace runs `sleep infinity` until you invoke Cargo; the application itself still needs no database connection. The startup dependency is not runtime application readiness.

```mermaid
flowchart LR
    DEV[Developer / DBeaver]
    APP[Rust app container]
    DB[(PostgreSQL container)]
    DISK[.dockerized-postgres/]
    DEV -->|localhost:5433| DB
    APP -.->|postgres:5432 - available, not consumed yet| DB
    DB -->|bind mount| DISK
```

## Database configuration and clients

| Variable | Development default | Use |
| --- | --- | --- |
| POSTGRES_DB | reliable_event_relay | Database created at first initialization |
| POSTGRES_USER | relay | Initial database user (official image creates a superuser) |
| POSTGRES_PASSWORD | relay | Initial local-only password |
| POSTGRES_HOST | postgres | Hostname used in the app container URL |
| POSTGRES_PORT | 5432 | Port used in the app URL; keep the server's default |
| POSTGRES_HOST_PORT | 5433 | Host loopback port publication |
| DATABASE_URL | postgresql://relay:relay@postgres:5432/reliable_event_relay | Composed app environment setting, not consumed by Rust yet |

Compose interpolates the six POSTGRES variables from `.env`, with defaults when absent, and explicitly injects initialization variables into `postgres`. It builds DATABASE_URL for `app`; setting DATABASE_URL in `.env` does not override this Compose-generated value. Keep POSTGRES_HOST=postgres and POSTGRES_PORT=5432 for this topology. If customizing credentials with URI-reserved characters, percent-encode their URL representation before a future client consumes it; these simple local defaults need no encoding.

In DBeaver, create a PostgreSQL connection:

```text
Database type: PostgreSQL
Host: localhost
Port: 5433
Database: reliable_event_relay
Username: relay
Password: relay
```

These are development defaults, not production credentials. Publication is bound to `127.0.0.1`, avoiding LAN exposure; clients may use IPv4 explicitly if localhost resolves only to IPv6. DataGrip, TablePlus or any PostgreSQL-compatible client also works. No GUI is required:

```bash
psql -h localhost -p 5433 -U relay -d reliable_event_relay -W
docker compose exec postgres psql -U relay -d reliable_event_relay
```

Host clients use localhost:5433; container clients use postgres:5432, never localhost for the other container.

## Physical state and shutdown

| Host directory | Contents | Reconstruction |
| --- | --- | --- |
| `.cargo-cache/` | Cargo downloads and source cache | `cargo fetch --locked` in app |
| `target/` | Compiled artifacts | `cargo build --locked` in app |
| `.dockerized-postgres/` | PostgreSQL cluster under `18/docker/` | Official image initializes an empty application database if absent |

All are ignored physical bind-mounted directories; no named data volumes are used. They make local generated state visible and intentionally reconstructable, but database contents cannot be recovered by rebuilding source: retain or back them up if needed. `docker compose down` stops/removes containers and the network, **not database contents** or Cargo directories. Recreating containers also preserves data. No migrations exist, so a new database has only PostgreSQL's built-in structures and no application tables.

## Reset only the local database

**Destructive: the following permanently deletes all local database contents. Back up anything needed first.** It leaves Cargo cache and build artifacts intact:

```bash
docker compose down
rm -rf .dockerized-postgres/
docker compose up -d --wait --wait-timeout 120
```

The image initializes a fresh database. Later milestones will add reproducible application schema creation; none exists today. This reset is documented, not performed automatically.

## PostgreSQL troubleshooting

- Inspect `docker compose ps` and `docker compose logs postgres` for initialization or health errors. `pg_isready` is not an authentication test; use an authenticated TCP SQL query too.
- A busy host 5433 prevents publication. Change POSTGRES_HOST_PORT in `.env` and use that port in host clients; keep internal 5432 unchanged.
- Changing POSTGRES_DB/USER/PASSWORD only affects initialization of an empty data directory. Existing credentials/state persist; change them through SQL deliberately or perform the destructive reset after backup.
- On Linux the official entrypoint initializes ownership for its own postgres user, separate from LOCAL_UID/GID. Read-only mounts or restrictive filesystem permissions can prevent startup. Do not chmod all database files to world-writable. Docker Desktop file sharing must allow this checkout.
- An incompatible PG_VERSION after a major image change requires a supported upgrade or backup/restore, not deleting files to silence an error.
- Compose refuses to start app while the database is unhealthy; this is development orchestration. The Rust binary and its tests still work independently without PostgreSQL.

See [PostgreSQL decision](postgresql.md), [testing](testing.md) and [validation results](validation-results.md).
