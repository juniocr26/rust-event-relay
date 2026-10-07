[Português brasileiro](../pt-BR/postgresql.md) | [README](../../README.md)

# PostgreSQL decision and local infrastructure

## Implemented today

Milestone 1.2 adds only a local PostgreSQL service, configuration, healthcheck, network access and visible host storage. The selected official image is `postgres:18.6-bookworm`, a stable explicit patch version on Debian Bookworm, rather than `latest`. Version selection follows the [18.6 release announcement](https://www.postgresql.org/about/news/postgresql-186-1711-1615-1519-1424-and-19-beta-3-released-3365/). The tag is pinned, not the image digest; upstream rebuilds can still change OS packages. Upgrade deliberately, with backup and compatibility checks; changing major versions does not upgrade existing data automatically.

The [official image](https://hub.docker.com/_/postgres) uses `/var/lib/postgresql/18/docker` for PostgreSQL 18. Compose binds `.dockerized-postgres/` to `/var/lib/postgresql`, so files appear under `.dockerized-postgres/18/docker/` without named or anonymous data volumes. Container recreation and `docker compose down` preserve those files. This local directory is not a production backup strategy.

The Rust binary does not read database settings or establish connections yet. Compose supplies `DATABASE_URL` as a future connection setting; no Rust database dependency or unused configuration type was added. `/health` remains HTTP liveness. `depends_on: service_healthy` gates development container startup, while `pg_isready` checks server acceptance, not application authentication, schema availability or continued readiness after startup.

## Useful for the intended architecture

PostgreSQL fits the intended transactional outbox through ACID transactions and mature concurrency control. A business change and an outbox record can commit atomically when the producer writes both to the **same local PostgreSQL database transaction**. This does not bridge separate databases or a remote broker. PostgreSQL offers strong SQL, indexing, row-level locking and reliable transactional behavior; none of these has been wired into application persistence yet. See [transactions](https://www.postgresql.org/docs/18/tutorial-transactions.html) and [locking](https://www.postgresql.org/docs/18/explicit-locking.html).

Conceptual future boundary only (no tables or migrations exist):

```text
BEGIN
update business_state ...
insert into outbox_events ...
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
