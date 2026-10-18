# Startup and token verification


The supported host startup is `python3 scripts/start-local.py`. It builds the runtime artifacts with `--locked` in `target/runtime`, verifies/backs up existing storage before any pending forward migrations, and starts Supervisor only after prerequisites. On manager startup, the entrypoint prepares runtime binaries automatically; direct `docker compose up` does not migrate retained storage. Use the supported startup for fresh installations. Missing schemas are an application startup failure, not necessarily a launcher execution failure. Check `docker compose logs --tail 60 app` and `docker compose exec app supervisorctl status`; do not repeatedly restart or reset volumes. Named operator STOPPED states remain respected under the same manager. Tests build in `target/tests` and never replace service binaries.

Inside app: `python3 /app/scripts/demo-token.py rotate`. Rotation saves the authoritative token first, restarts only running HTTP to reload it, starts FATAL/EXITED programs once, and preserves intentional STOPPED programs. It verifies new-token access and rejection of the previous token, named process stability, readiness of running processes and durable RabbitMQ topology through read-only management queries. These checks do not publish business events or purge queues. HTTP intentionally STOPPED leaves activation pending and returns nonzero. A failure after activation explicitly reports that rotation was already applied; retrieve the token privately from `.env` and fix the stated service fault before retrying. Supervisor process status and queue topology are separate checks.

In the reproduced startup incident all launchers existed, were executable and successfully launched their binaries. The binaries then exited: the configured database had no application tables or SQLx history. Logs reported ownership schema unavailable, consumer schema unavailable and demo forward migration required. After an explicitly authorized backup/disposable restore proof and forward migration, the supported startup recovered all three roles. Artifact-directory waits and manually interrupted commands are not evidence of an application defect.

---

[Token and connectivity](demo-token-connectivity.md)

[Português brasileiro](../../pt-BR/operations/application-scenarios.md) | [Catalog](../index.md)

# Application guide

Start with the [README](../../../README.md). One main Compose environment owns app, frontend, PostgreSQL and RabbitMQ. `compose.demo.yaml` and its scripts are historical dedicated-runtime artifacts; do not use them for normal startup. No Engineering Library or sibling repository is required.

## Components and lifecycle

`backend/src/domain` validates canonical envelope and inventory identities. `application` defines ports, ownership/retry/replay decisions and bounded worker lifecycles. `infrastructure/postgres` implements short SQL transactions and statement-snapshot inspection; `infrastructure/rabbitmq` owns publisher confirms, manual ACK and quarantine. `infrastructure/demo_delivery.rs` executes bounded action sessions. `infrastructure/demo_http.rs` exposes authenticated fixed intents and read-only evidence. `frontend/src/components` renders reusable Vue controls and the modal; `composables/useDemoRun.ts` keeps execution and inspection separate. `backend/migrations` is ordered SQLx history; `docker/supervisor` manages three independent binaries; `scripts` contains startup and disposable verification.

1. Producer commits a goods receipt, stock fixture and canonical outbox event atomically. Repeated identical request UUID returns existing evidence; changed intent conflicts.
2. Relay acquires leased publication authority, publishes and observes broker acceptance via confirm.
3. Relay persists completion only with current ownership/revision authority.
4. Consumer atomically commits inventory effect and deduplication receipt, or verifies immutable identity/content and commits Duplicate classification without a second effect.
5. Consumer attempts ACK send after persistence. A local successful send does not prove broker receipt.

These describe responsibilities, not a strict observation order: consumer evidence can precede observed relay completion. Processed outbox does not prove consumer completion. Publisher confirms and consumer ACK are different protocols. SQL COMMIT and AMQP cannot be atomic; unknown does not mean rolled back. Even a negative SQL snapshot cannot exclude a transaction committing afterward.

## Configuration

Required local values: replace POSTGRES_USER/PASSWORD and RABBITMQ_DEFAULT_USER/PASS placeholders in ignored `.env`. Use fresh values only for a fresh installation; initialization variables do not rotate an existing cluster/broker credential. DEMO_TOKEN is generated once when empty, requires 64 hexadecimal characters and stays private. The browser receives it only through runtime entry, keeps it in memory, and requires entry after reload. sessionStorage retains only request/selection/action identifiers. No secret belongs in VITE_* or the frontend bundle.

All remaining `.env.example` values have explicit defaults. HOST_HTTP_PORT, HOST_FRONTEND_PORT, POSTGRES_HOST_PORT and RABBITMQ_MANAGEMENT_PORT change host loopback publications; keep internal HTTP 8080, PostgreSQL 5432 and frontend 5173 aligned with Compose. LOCAL_UID/GID select the Linux development user. REPLAY_OPERATORS empty disables privileged replay; RELAY_MAINTENANCE_CANDIDATES empty disables explicit candidate injection. Normal bounded discovery still runs. Metrics addresses are internal loopback only. Consumer and relay timing/pool settings are finite bounds, not tuning suggestions. Fixed local scenarios require the default inventory consumer and 10+5 intent; changing topology requires matching producer/consumer settings. Startup rejects placeholders and invalid token without logging values. Busy ports require freeing the configured port, not silently switching addresses.

`python3 scripts/start-local.py` starts infrastructure, builds locked binaries, verifies existing migration checksums, creates a private pg_dump with disposable restore proof before necessary forward migration, then starts the application. Fresh SQLx migrations run before workers. A no-op migration check preserves data. Older migration rollbacks can delete evidence: use only the disposable migration harness. No applied migration is rewritten by 6.3.

## Real and illustrative scenarios

- Successful delivery: new fixed intent, stock 10 → 15, processed outbox and one receipt.
- Duplicate delivery: create a fixture or explicitly choose a successful retained run. A bounded action republishes the identical original envelope once; inspect a correlated Duplicate observation, one receipt and stock 15. “Request another duplicate” is explicit new work with a new action request UUID.
- Concurrent duplicate: three bounded publication calls, two temporary consumer sessions; correlated Duplicate classifications preserve the same single effect. Closing each completed session releases any unsettled prefetched copy for redelivery. Arrival order/count is not an exactly-once guarantee.
- Controlled recovery: one run-scoped durable rejection before AMQP send, recorded retry schedule/revision and later successful delivery. It is not a broker outage or broker NACK.
- Crash, race and ACK-loss playback: illustrative only. Real fault boundary coverage belongs to isolated integration tests; there are no global shutdown, arbitrary SQL/shell or Docker socket controls in the UI.

Open “Inspect retained runs and actions”, authenticate, load a page (10 rows, maximum 100 retained), select and inspect any pending or historical run. Backend `/demo/history?limit=10&offset=0` permits 1–20 rows and offsets 0–100 in stable accepted-time/UUID order. Existing `/demo/runs` remains the successful-run selector. Select an action and inspect its own publication/classification evidence. Closing/reloading never submits work; inspection is GET only. Recovering unknown actions never republishes. If no committed request is visible, inspect again; explicit “Submit same producer intent” is an idempotent submission with the same fixed identity, not evidence that earlier work rolled back.

Loading, pending, unavailable, terminal, unknown and stale evidence have separate descriptions. Every successful snapshot exposes observed_at, becomes stale after 15 seconds and remains a previously observed fact after a failed refresh. Polling is sequential, bounded to 60 seconds with a nine-second request timeout. Browser deadlines do not cancel committed backend work.

## Where to verify

Use a PostgreSQL client at localhost:5433 with POSTGRES_DB, POSTGRES_USER and private POSTGRES_PASSWORD from `.env`, or avoid password arguments entirely:

```sh
docker compose exec -T postgres sh -c 'PGPASSWORD="$POSTGRES_PASSWORD" psql -X -h postgres -U "$POSTGRES_USER" -d "$POSTGRES_DB"'
```

Copy IDs and filtered read-only SQL from the modal. Replace placeholders below only with the displayed UUIDs/stock identity:

```sql
BEGIN READ ONLY;
SELECT * FROM demo.runs WHERE run_id = '<run UUID>';
SELECT * FROM demo.goods_receipts WHERE run_id = '<run UUID>';
SELECT id,status,attempt_count,available_at,processed_at FROM relay.outbox_events WHERE id = '<event UUID>';
SELECT * FROM demo.actions WHERE run_id = '<run UUID>';
SELECT * FROM demo.actions WHERE request_id = '<action request UUID>';
SELECT * FROM demo.publications WHERE action_id = '<action UUID>';
SELECT * FROM demo.consumer_observations WHERE run_id = '<run UUID>';
SELECT * FROM demo.faults WHERE run_id = '<run UUID>';
SELECT consumer_name,event_id,stock_before,quantity,stock_after,completed_at FROM consumer.receipts WHERE event_id = '<event UUID>';
SELECT units FROM consumer.stock WHERE warehouse_id = '<warehouse>' AND sku = '<SKU>';
COMMIT;
```

`demo` owns runs, goods receipts, actions, publications, consumer observations and controlled faults. `relay.outbox_events` owns publication state/leases/retry; `relay.replay_requests` owns authorized replay audit. `consumer.stock` and `consumer.receipts` own stock effect and immutable-content deduplication proof. `public._sqlx_migrations` records migration versions/checksums/success. A goods receipt proves producer business fact, processed_at relay completion, a consumer receipt 10+5=15 the effect, and correlated Duplicate observations classification without additional effect. Observation caps make counts lower bounds.

RabbitMQ management: http://localhost:15672, private local broker credentials, vhost `relay`, direct durable exchange `relay.events`, routing key `event`, source queue `relay.local`. Each retained duplicate action uses `relay.demo.<action UUID>` as both queue and routing key; inspect the displayed action identity. Quarantine exchange `relay.quarantine`, routing key `inventory`, queue `relay.quarantine.inventory`. These defaults can be overridden consistently; the modal displays configured source topology. Queue graphs are aggregate observations; a quickly consumed message may vanish before the next graph sample. SQL `dead_letter` is terminal relay state, distinct from RabbitMQ quarantine. Rejected consumer input must be preserved through confirmed quarantine before source ACK.

## Operations, capacity and recovery

`/health` proves HTTP liveness only. Inside app, `/ready` and `/metrics` on ports 9100/9101/9102 report HTTP/relay/consumer dependency and freshness state; they are not published. Inspect with Python urllib from the container. A stopped consumer can leave a processed outbox and pending consumer effect: inspect both independently. Use `supervisorctl status` and named start/stop controls. Intentional stops are respected by the existing manager; recreating it resumes autostart. Source/binary/process state must be distinguished after a rebuild.

Bounds: 100 retained runs, 100 actions total, three actions/run, four active runs/actions, one action worker, two temporary contexts, 1–3 publication calls/action, 20-second action deadline, 32 publication and 32 consumer observations/run. Queues are durable and retained even for unknown actions. Capacity exhaustion rejects admission; it does not delete history. No automatic retention or general cleanup endpoint exists. Stop admission and export required SQL/broker evidence before manually designing maintenance; never delete receipts while redelivery remains possible. No general repair or unknown-action resend is provided.

PostgreSQL data persists under `.dockerized-postgres/`; RabbitMQ uses its named volume. `docker compose stop` preserves both. `.local-backups` is private and ignored. Before a required main migration, `consolidate-storage.py --apply` backs up, verifies restoration in a disposable database and applies forward-only changes; when current it checks versions/checksums without migration. Keep backup material access restricted. Restore only into a fresh disposable database to verify first; do not overwrite the main database automatically. Broker queues are not included in pg_dump; independent broker backup/restore and production disaster recovery remain outside this local demonstration.

## Study path

Trace one displayed Event ID: start at `application/demo.rs` and `postgres/demo.rs` producer transaction; inspect its goods receipt and outbox; follow `application/worker.rs`, `delivery.rs` and PostgreSQL ownership/retry adapters; follow `rabbitmq.rs` confirms; inspect `application/consume.rs`, PostgreSQL consumer adapter and RabbitMQ ACK/quarantine code; compare the consumer receipt with stock. Then trace a duplicate Action ID through `demo_delivery.rs` and the correlated observation tables. Read ADRs 007–010/012 for uncertainty and ownership trade-offs, and the corresponding tests for failure boundaries. Compare actual persisted facts before interpreting the frontend status.

Protocol references consulted for these boundaries: [RabbitMQ confirms and consumer acknowledgements](https://www.rabbitmq.com/docs/confirms), [PostgreSQL Read Committed snapshots](https://www.postgresql.org/docs/current/transaction-iso.html). They explain protocol/database semantics; application tests establish the behavior of this implementation.
