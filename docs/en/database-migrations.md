[Português brasileiro](../pt-BR/database-migrations.md) | [README](../../README.md)

# Database migrations — Milestone 1.3

## Purpose and tooling

Source-controlled SQL migrations own PostgreSQL schema evolution. SQLx CLI **0.8.6** is installed in the development Docker image with `cargo install sqlx-cli --version 0.8.6 --locked --no-default-features --features rustls,postgres --root /opt/sqlx`. The migration CLI is separate from the SQLx application dependency introduced for reads in Milestone 1.6. The CLI version and its packaged dependency lockfile are pinned; base-image tags and Debian package revisions are not immutable digests. Rebuild with `docker compose up -d --build --wait --wait-timeout 120`.

`docker/sqlx.py`, installed as `sqlx`, constructs DATABASE_URL for each invocation from Compose-injected POSTGRES_USER/PASSWORD/DB/HOST/PORT. It percent-encodes user, password and database components and passes the URL only in the child environment. It enforces the project's internal `postgres:5432` topology. A DATABASE_URL in the host `.env` is not used by this wrapper. The Rust application still does not connect to PostgreSQL. Do not print connection strings or use real credentials in examples. SQLx subcommand help can display DATABASE_URL as an environment default; redact that output before sharing.

Compose's existing healthcheck gates app startup; commands do not use fixed sleeps. Health is server acceptance, not proof that configured roles or passwords exist. SQLx connection errors fail the command; inspect PostgreSQL logs and verify authenticated TCP access if it is unavailable.

## Directory and initial strategy

```text
migrations/
  20261007000000_create_relay_schema.up.sql
  20261007000000_create_relay_schema.down.sql
  20261007175358_create_outbox_events.up.sql
  20261007175358_create_outbox_events.down.sql
```

Option B: create an empty `relay` namespace reserved for future relay database objects. This establishes an architectural schema boundary and demonstrates a reversible migration without business tables. Future migrations should explicitly qualify objects with `relay.`; the default search_path is unchanged. The up migration fails if an unmanaged namespace already exists. The down migration uses RESTRICT, refusing to destroy dependent objects. SQLx maintains its own `_sqlx_migrations` bookkeeping table in the default public schema; it is tooling metadata, not application storage. Milestone 1.4 adds relay.outbox_events through a separate migration; Milestone 1.6 provides a Rust read repository; inserts and workers remain deferred.

Migration files are source code and must be committed with the relevant code. `.dockerized-postgres/` is ignored local cluster data, never migration history. Rebuilding a container preserves data; a fresh cluster needs `sqlx migrate run` explicitly. Neither entrypoint nor application startup runs migrations or resets the database automatically.

## Development workflow

Create a migration, review its up/down SQL, apply it, inspect the schema, run tests, test rollback, re-apply and commit the migration with code. SQLx uses timestamp-prefixed filenames; use clear names such as `add_relay_namespace_comment`. `create_outbox_events` is the Milestone 1.4 migration; use similarly descriptive names. Avoid `migration1`, `update_db` and `changes`. `-r` explicitly requests paired up/down files. No Makefile is required.

```bash
docker compose up -d --build --wait --wait-timeout 120
docker compose exec app sqlx --version
docker compose exec app sqlx migrate add -r describe_schema_change
# Review both generated SQL files before running them.
docker compose exec app sqlx migrate info
docker compose exec app sqlx migrate run
docker compose exec postgres sh -c 'psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -c "\dn"'
docker compose exec app cargo test --locked
docker compose exec app sqlx migrate revert
docker compose exec app sqlx migrate info
docker compose exec app sqlx migrate run
```

For the committed initial migration, skip the `migrate add` step. `migrate revert` reverts the most recently applied reversible migration; it does not erase the entire history. Confirm `info` reports pending after revert and installed after re-apply. Inspect with DBeaver or container psql. Do not run rollback blindly against a shared database.

## Ownership and immutable history

Migration files are the canonical schema history. DBeaver supports inspection, querying and debugging; manually created DBeaver objects are not authoritative project changes. Capture intended changes in a migration and review the SQL.

Once committed and applied in shared history, prefer a new migration instead of rewriting an old one. SQLx validates checksums; editing applied SQL causes mismatches. Immutable history supports reproducibility, team consistency, synchronization across environments and auditability. Unshared early local experiments may be recreated deliberately; do not normalize rewriting shared history or deleting checksum records.

## Transactions and rollback trade-offs

SQLx's PostgreSQL backend runs each migration and essential bookkeeping in a transaction by default, including down migrations. Transactional DDL prevents a failed migration from leaving half its schema change committed. The entire sequence of multiple migrations is not one transaction. PostgreSQL operations such as CREATE INDEX CONCURRENTLY cannot run inside a transaction block. SQLx supports a leading `-- no-transaction` directive for such migrations; losing atomicity requires careful failure recovery. Do not add explicit BEGIN/COMMIT to ordinary SQLx migration files. See the [versioned SQLx backend](https://github.com/launchbadge/sqlx/blob/v0.8.6/sqlx-postgres/src/migrate.rs) and [PostgreSQL CREATE INDEX restrictions](https://www.postgresql.org/docs/18/sql-createindex.html).

Down migrations help local development but do not guarantee recovery of deleted or transformed data. Destructive production rollback can lose information; backups, tested restoration and sometimes a forward corrective migration are preferable. This is a learning/development workflow, not a complete production deployment strategy. Review the down SQL as carefully as the up SQL.

## Reset is not rollback

**Database reset != migration rollback.** Reset deletes the entire `.dockerized-postgres/` cluster, all databases, roles and data; rollback applies controlled down SQL to a selected database. Rollback does not change initialization credentials. A reset requires explicit developer intent and a backup of anything needed. See the [warning and exact reset commands](postgresql.md#initialization-and-deliberate-reset). After reset, re-apply migrations explicitly.

## Troubleshooting

- `sqlx: command not found`: rebuild the app image; host installation is unnecessary.
- Role does not exist/password authentication failed: `.env` may differ from persisted roles. Changing Docker environment does not update an initialized cluster; see [PostgreSQL initialization](postgresql.md).
- Connection refused/unavailable: check `docker compose ps`, `docker compose logs postgres` and health; use internal postgres:5432, never host 5433 inside app.
- Checksum mismatch: restore committed SQL and add a new migration; do not bypass history validation.
- Schema already exists: investigate ownership before applying; never silently adopt or delete unknown objects.
- RESTRICT rollback failure: inspect dependencies and revert their owning migrations first. Do not substitute CASCADE to force deletion.
- Permissions: migration generation writes to the source mount as developer; align Linux LOCAL_UID/GID and mount permissions.

See [SQLx CLI 0.8.6 usage](https://github.com/launchbadge/sqlx/blob/v0.8.6/sqlx-cli/README.md), [ADR 003](adr/003-use-versioned-sql-migrations.md), [testing](testing.md) and [actual validation](validation-results.md).

## Read-only authentication diagnostics

```bash
./scripts/check-postgres.sh
docker compose exec -T app sqlx migrate info
```

[check-postgres.sh](../../scripts/check-postgres.sh) checks health, IPv4 loopback publication, current Compose/.env versus running app/postgres settings, authenticated identity and existing migration history. It prints neither the configured username nor password. It exits nonzero on configuration drift or authentication failure, and never changes roles, creates metadata, applies migrations or resets data. It uses Python inside the running app container; the host TCP probe uses nc or Python 3, explicitly reporting skipped if neither is available. SQLx connectivity is checked separately.

Compose supplies POSTGRES_HOST/PORT to postgres for these client diagnostics; this does not change server listening settings. Host/DBeaver uses 127.0.0.1:5433 and current initialized POSTGRES_DB/USER/PASSWORD; containers use postgres:5432. DBeaver GUI was not tested.

Changing `.env` does not update an initialized cluster. A missing role can produce generic TCP password-authentication failure. Inspect server details/roles to distinguish it from an existing role with a wrong password. The real-cluster repair inspected for data before the explicitly authorized destructive reset, then ran existing migrations. Normal Compose startup/shutdown remains non-destructive. Reset deletes all cluster state; migration rollback does not repair credentials.

Raw Compose config, environment dumps and SQLx help can reveal secrets; inspect through a parser reporting only non-sensitive fields/equality checks and redact identifying credentials before sharing logs. See [PostgreSQL repair](postgresql.md#authentication-remediation-on-the-real-local-cluster) and [actual remediation results](validation-results.md#local-postgresql-authentication-remediation).

The current last migration is create_outbox_events. Revert drops only that table (and its own indexes/constraints), preserving relay; inspect for data first. Re-apply with migrate run. [Outbox schema](outbox-schema.md) documents the full mapping and destructive rollback limitations.

## PostgreSQL repository — Milestone 1.6 implemented

`src/infrastructure/postgres/` implements the existing `OutboxReader` using an injected `PgPool`. Infrastructure depends inward on persistence and domain; SQL, SQLx, private rows and driver mapping stay in infrastructure. Models own validation/conversion and contain no queries. No duplicate interface, empty layers, new migration or ADR is needed under ADR 005. Native Send futures/static dispatch remain intact. HTTP startup remains independent of PostgreSQL.

See [query, restoration, bounds and errors](postgres-repository.md), [integration test guidance](testing.md) and [executed validation](validation-results.md). SQLx is now an application dependency as well as separate migration tooling. Writes, claims and processing remain deferred; Milestones 1.7 and 1.8 are complete: integration evidence and [failure/transaction semantics](failure-and-transaction-semantics.md). Milestone 1 is closed; later delivery work has not begun.
