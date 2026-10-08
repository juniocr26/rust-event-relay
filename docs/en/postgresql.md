[Português brasileiro](../pt-BR/postgresql.md) | [README](../../README.md)

# PostgreSQL decision and local infrastructure

## Implemented today

Milestone 1.2 adds only a local PostgreSQL service, configuration, healthcheck, network access and visible host storage. The selected official image is `postgres:18.6-bookworm`, a stable explicit patch version on Debian Bookworm, rather than `latest`. Version selection follows the [18.6 release announcement](https://www.postgresql.org/about/news/postgresql-186-1711-1615-1519-1424-and-19-beta-3-released-3365/). The tag is pinned, not the image digest; upstream rebuilds can still change OS packages. Upgrade deliberately, with backup and compatibility checks; changing major versions does not upgrade existing data automatically.

The [official image](https://hub.docker.com/_/postgres) uses `/var/lib/postgresql/18/docker` for PostgreSQL 18. Compose binds `.dockerized-postgres/` to `/var/lib/postgresql`, so files appear under `.dockerized-postgres/18/docker/` without named or anonymous data volumes. Container recreation and `docker compose down` preserve those files. This local directory is not a production backup strategy.

The Rust binary does not read database settings or establish connections yet. The SQLx wrapper builds `DATABASE_URL` from Compose settings for migration commands; SQLx now backs the read adapter; no unused database configuration was added to the HTTP bootstrap. `/health` remains HTTP liveness. `depends_on: service_healthy` gates development container startup, and the healthcheck executes `SELECT 1` over TCP with the configured credentials. It uses the network address `POSTGRES_HOST`, avoiding loopback with its `trust` rule, and suppresses output. This validates authentication and database access, without checking schema availability or continued readiness after startup.

## Useful for the intended architecture

PostgreSQL fits the intended transactional outbox through ACID transactions and mature concurrency control. A business change and an outbox record can commit atomically when the producer writes both to the **same local PostgreSQL database transaction**. This does not bridge separate databases or a remote broker. PostgreSQL offers strong SQL, indexing, row-level locking and reliable transactional behavior; none of these has been wired into application persistence yet. See [transactions](https://www.postgresql.org/docs/18/tutorial-transactions.html) and [locking](https://www.postgresql.org/docs/18/explicit-locking.html).

Future write transaction (the outbox table exists, but application writes are not implemented):

```text
BEGIN
update business_state ...
insert into relay.outbox_events ...
COMMIT
```

This addresses the unsafe dual write: commit a business change, publish a message, then lose publication if the producer crashes between those actions. The future relay will asynchronously process durable outbox records. Publication and acknowledgement can still fail independently, so consumers need idempotency; there is no exactly-once delivery claim.

JSON/JSONB can accommodate the envelope payload; JSONB supports indexing but changes some textual representation properties. The eventual schema and index strategy remain undecided. See [JSON types](https://www.postgresql.org/docs/18/datatype-json.html). `FOR UPDATE SKIP LOCKED` is a possible future worker-claim mechanism, allowing busy rows to be skipped; it is not implemented and offers neither strict ordering nor fairness. Mature tools, SQL clients, logs and system views make inspection and operational monitoring familiar; production monitoring has not been configured.

## Trade-offs

- Stateful operations require storage management, backups in real deployments, connection management, schema evolution and monitoring.
- A single database has finite capacity. High write volume, poor indexes or claim patterns can cause contention; vertical scaling is not unlimited.
- A future durable outbox needs processed-event cleanup, retention/archive policies, index design, table-growth monitoring, vacuum behavior and capacity planning.
- Concurrent workers may compete for rows. Locking and SKIP LOCKED help with coordination but require short transactions, recovery rules and explicit fairness/ordering decisions.
- PostgreSQL-specific semantics reduce portability. Database portability is not a current project goal; this is a deliberate trade-off, not a claim of universal superiority.

See [ADR 002](adr/002-use-postgresql-for-durable-event-storage.md), [Docker setup and DBeaver](docker-and-configuration.md), [infrastructure checks](testing.md) and [actual validation](validation-results.md).

## Initialization and deliberate reset

POSTGRES_DB, POSTGRES_USER and POSTGRES_PASSWORD are primarily **initialization variables** for the official image. They initialize roles/database/password only when the cluster data directory is empty. Changing `.env`, rebuilding or recreating containers does not mutate existing PostgreSQL roles, databases or passwords.

```text
Docker environment != already-created PostgreSQL roles/database state
```

Example: initialize with user `first_user`, then change `.env` to `second_user`. The container environment can report second_user while the persisted cluster still contains first_user, causing `FATAL: role "second_user" does not exist`. Changing only the password can similarly cause authentication failure. `pg_isready` may still report healthy; verify authenticated TCP SQL, not just environment values. Do not print passwords for debugging.

Keep the original cluster credentials, or deliberately administer roles/databases through SQL with existing authorized access. In early development, intentionally recreating the cluster is another option, after backing up anything needed. No Compose operation or application entrypoint automatically deletes or reinitializes `.dockerized-postgres/`.

**WARNING: this permanently deletes the local PostgreSQL database state.** These manual commands require an explicit decision to discard the cluster; they are never run automatically. Run them from the repository root and back up anything needed first.

```bash
# Only after explicitly deciding to discard all local database state:
docker compose down
rm -rf .dockerized-postgres/
docker compose up -d --build --wait --wait-timeout 120
docker compose exec app sqlx migrate run
```

Reset deletes all databases, roles and data in the cluster; migration rollback executes controlled down SQL in one database. **Database reset != migration rollback.** See [migrations](database-migrations.md).

## Shell expansion and external clients

Single quotes below keep variables from expanding in the host shell; `sh -c` expands them inside the container where Compose supplied them:

```bash
docker compose exec postgres sh -c 'psql -U "$POSTGRES_USER" -d "$POSTGRES_DB"'
docker compose exec -T postgres sh -c 'PGPASSWORD="$POSTGRES_PASSWORD" psql -h postgres -p 5432 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT 1;"'
```

The first command uses the local Unix socket and may not validate the password; the second authenticates over TCP. Writing `-U "$POSTGRES_USER"` directly in a host command expands host variables, which may be unset or different. Container environment values still need to match the initialized cluster.

DBeaver external connection:

```text
Host: 127.0.0.1
Port: 5433 (or POSTGRES_HOST_PORT)
Database: value from POSTGRES_DB
Username: value from POSTGRES_USER
Password: value from POSTGRES_PASSWORD
```

Use 127.0.0.1 to match intentional IPv4 loopback publication and avoid localhost IPv4/IPv6 ambiguity. Containers use postgres:5432; host clients use 127.0.0.1:5433. Values from `.env` must match actual persisted credentials, not merely the current container environment.

## Schema ownership

[migrations/](../../migrations/) contains canonical source-controlled SQL history, managed by SQLx CLI 0.8.6 inside Docker. DBeaver is for inspection, querying and debugging; intended schema changes belong in migrations. Physical `.dockerized-postgres/18/docker/` data is ignored local state and survives up, down, build and container recreation. Migration rollback does not delete that directory. Milestone 1.3 creates only an empty relay namespace; Milestone 1.4 adds the outbox schema; Milestone 1.6 adds application reads; writes remain future work.

## Authentication remediation on the real local cluster

On 2026-10-07, authorized inspection confirmed the configured role was **absent** from the persisted cluster. TCP returned `password authentication failed`; server logs additionally reported `Role "<configured-user>" does not exist`. That TCP error alone does not prove a role exists or has a stale password. Check server details and actual `pg_roles` through authorized access; never expose password hashes or credentials.

Both non-template databases had only public, no user relations/routines and no SQLx history or application data. This satisfied the developer's explicit condition for reset. The empty `.dockerized-postgres/` cluster was deleted after stopping Compose, recreated from the unchanged current `.env`, and received the existing namespace migration. The real cluster now authenticates with those credentials. No DBeaver credential changes were required.


### Focused authentication revalidation

During revalidation on 2026-10-07, the container was stopped. After `docker compose start postgres`, the existing cluster accepted the exact current `.env` credentials through the Docker network and published host port. An incorrect password was rejected on both routes, confirming actual authentication rather than a `trust` rule. The historical failure was not reproduced; this revalidation did not change passwords, create roles, reset the cluster or run migrations. The healthcheck now tests authenticated SQL; loopback `pg_isready` did not prove password validity.

## PostgreSQL repository — Milestone 1.6 implemented

`src/infrastructure/postgres/` implements the existing `OutboxReader` using an injected `PgPool`. Infrastructure depends inward on persistence and domain; SQL, SQLx, private rows and driver mapping stay in infrastructure. Models own validation/conversion and contain no queries. No duplicate interface, empty layers, new migration or ADR is needed under ADR 005. Native Send futures/static dispatch remain intact. HTTP startup remains independent of PostgreSQL.

See [query, restoration, bounds and errors](postgres-repository.md), [checks and smoke guidance](testing.md) and [executed validation](validation-results.md). SQLx is now an application dependency as well as separate migration tooling. Writes, claims and processing remain deferred; Milestone 1.7 covers the full database integration suite and 1.8 broader failures/transactions.
