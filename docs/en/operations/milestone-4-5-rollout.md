[Português brasileiro](../../pt-BR/operations/milestone-4-5-rollout.md) | [Catalog](../index.md)

# Milestone 4.5 — controlled local rollout and operational verification

Executed 2026-10-16, after explicit local development authorization. **Local rollout complete.** Base `a1b0b389fe2216618298614c144f4187c3257666`; initial tracked/untracked status clean. No project/ancestor AGENTS.md applies. The invoked Dev Career Copilot methodology and current source/contracts governed the work. Only rust-event-relay was changed; no staging, commit or push. This record supersedes earlier pending shared-rollout statements; [4.4 acceptance](milestone-4-4-review.md) remains dated isolated-test evidence.

## Baseline and configuration

Fresh inspection confirmed the previous report: HTTP/relay running old deleted executable inodes; installed Supervisor lacked consumer and its launcher; four migrations; empty outbox; replay disabled; durable classic relay.local empty with zero unacknowledged deliveries/consumers; quarantine absent. No unexpected messages or concurrent writers were found. The frontend remained an illustration.

Initial loaded SHA-256: HTTP `0896f95c8b9c901ca01b2f83f52cf9fad2bf6d62c34ba9e01c59bdf360f4a502`; relay `b3f0490f339a90f06910673decf553c9cd00a426632c8a24d1b0a74861014f92`. Their exact source revision is unknown.

Reviewed code/config parsers, .env example and private local settings, Compose, actual injection, process environments, launchers, migration SQL, consumer/replay contracts and existing diagnostics. Required credentials were present and authenticated; no credentials were printed, generated, rotated or changed. Consumer variables and REPLAY_OPERATORS were missing from the old container injection. App recreation installed the current explicit mapping; all mapped values now match Compose and each child's initial environment. No user-specific value blocks this local rollout. REPLAY_OPERATORS remains empty and maintenance candidates remain empty; no privileged replay activation was needed.

Process variables override dotenv, which fills missing values from `/app/backend` → `/app/.env`; Compose shell interpolation overrides root .env. APP_ENV/HTTP_ADDR/RUST_LOG retain this documented mounted-dotenv fallback. Internal database/broker endpoints remain postgres:5432 and rabbitmq:5672. Existing settings were preserved. The [inventory](../docker/environment-inventory.md) defines ranges and conditional requirements; successful current parser/preflight startup establishes admission, not production capacity. Consumer identity is `inventory.stock.v1`, prefetch 1, one active operation, default operation/drain/shutdown 10/15/25 s; Supervisor child stop bound 30 s and Compose grace 95 s.

The configured development database role remains superuser/CREATEDB. Required runtime privileges work, but least privilege is **not installed**. Broker configure/write/read permissions remain scoped to `^relay[.].*`. Broader deployment requires separately reviewed restricted roles/grants and topology authority. No frontend credentials or API settings were added.

## Quiesced backup and restoration

Stopped named HTTP and relay gracefully before the final backup; consumer was not yet installed, replay disabled. Checkpoint observations: outbox=0, active leases=0, other database sessions=0, source ready/unacknowledged/consumers=0/0/0. PostgreSQL/RabbitMQ remained running. This excludes observed application activity, not every possible future administrator connection.

Host backup: `.local-backups/20261016T153255Z/pre-milestone-4-5.dump` under the checkout, outside tracked files and container-only storage. Created **2026-10-16 15:32:55 UTC / 12:32:55 America/Sao_Paulo**, 8,943 bytes, SHA-256 `6a82bb0801cb33e65d8dfcbdee317c9c27b390f86960de8a8b3bb7c1dd4a03d2`. Directories 0700; dump and private verification records 0600. Exact absolute path, checksum and restore-test identity are in the protected `verification.json` beside it. Both Git and Docker build context exclude `.local-backups/`; never force-add or publish it. Host storage loss is not covered by this local copy.

Used PostgreSQL 18.6 container tooling, full `pg_dump -Fc` streamed to the host with checked exit status. Restored into a fresh exact UUID-named `relay_restore45_…` database using `pg_restore --exit-on-error --single-transaction`, retaining ownership/ACL in the same cluster. Independently compared all four migration rows/checksums, outbox count and full schema-only dumps (excluding only randomized dump restrict markers). All matched. Dropped only that exact test database and verified absence. No shared migration ran before successful restoration.

Scope: one database's schema/data/ACL, including outbox/history at that checkpoint. Excludes RabbitMQ messages/topology backup, global roles/passwords/tablespaces, other databases, configuration secrets and external effects. The demonstration rows were created later and are absent from this backup. See official [pg_dump scope](https://www.postgresql.org/docs/18/app-pgdump.html) and [pg_restore options](https://www.postgresql.org/docs/18/app-pgrestore.html).

## Migrations and installation

Confirmed the first four SHA-384 checksums against source before migration. Reviewed exactly the pending additive upgrades:

- `20261013000000`: replay ledger/FK/checks, immutable audit trigger, PUBLIC table revocation.
- `20261014000000`: consumer schema, stock and receipts, primary/deferred foreign keys, coherent completion checks, deferred completeness trigger, immutable identity/removal/truncate guards, PUBLIC revocations.

No destructive up operations or existing-data conflict were found. Explicit `sqlx migrate run` applied only these two pending versions, in order. Final six successful history/checksum entries match source. Inspected actual tables, constraints, three functions and all four user triggers, including deferred receipt completeness and truncate guard. No applied SQL was rewritten, no unrelated migration, down or startup migration was introduced.

Built all binaries with `cargo build --locked --bins` while writers were stopped; Cargo validated current inputs and reused up-to-date artifacts. Built the app image, recreated **only app** with `docker compose up -d --no-deps app`, initially retaining consumer autostart=false. HTTP/relay deliberately resumed and passed schema/health checks. Checked source/installed Supervisor byte equality, executable launchers, `/app/backend` directories and fixed `/app/target/debug` paths.

After safe explicit consumer activation and control verification, changed consumer autostart to true, rebuilt app image, stopped all three **named** programs gracefully and recreated only app again. All three then started automatically. Database, RabbitMQ and frontend container identities/storage were preserved. No `start all`, purge, reset, queue recreation, credential change or shared crash/outage injection occurred.

Final loaded executable SHA-256 (each matches its on-disk executable; none is deleted):

| Program | Path | SHA-256 |
| --- | --- | --- |
| HTTP | `/app/target/debug/reliable-event-relay` | `10ab2225cf166f20a0c862b244db3a404fd3ed86a5419466bcb440422e1f97d6` |
| Relay | `/app/target/debug/relay` | `5b3c3b3c593b65718e235ccf213b1e834224095a45abd48d2fd84016eb2a80c8` |
| Consumer | `/app/target/debug/consumer` | `dcb6e9957017246d109b704137cbed26578cba686419638e53b6823ebaabf80a` |

Tracked application sources/Cargo files match the base revision byte for byte. Rollout configuration/docs form the dirty working tree. Cargo's successful locked build and loaded/file hash agreement provide current-source evidence; these development binaries have no embedded Git revision or reproducible-build attestation, so no exact embedded revision claim is made. Private final verification retains hashes and observation time.

## Consumer admission, topology and retained demonstration

Immediately before subscribing, inspected queue metadata without fetching messages: relay.local ready=0/unacknowledged=0/consumers=0, compatible durable classic declaration, expected binding and no policies/operator policies. Verified source settings agree with relay target and required database privileges. Explicitly started `supervisorctl start consumer`. Current durable direct topology in vhost `relay`:

| Exchange | Queue | Routing key | Final ready / unacked / consumers |
| --- | --- | --- | --- |
| relay.events | relay.local (classic) | event | 0 / 0 / 1 |
| relay.quarantine | relay.quarantine.inventory (classic) | inventory | 0 / 0 / 0 |

Queues are non-auto-delete; subscription uses manual ACK/prefetch 1. No TTL, expiry, DLX, capacity, purge or replay policy was introduced. Quarantine remains preserved storage, with no replay/purge workflow. Durable topology is not a broker backup.

Retained, collision-checked study identifiers:

- Event: `481fb751-81d5-46b7-aea2-eed344084725` (fresh nonnil UUID).
- Warehouse/aggregate ID: `m45-demo-6aef7312e7014b969e0b5552d872abf1`.
- SKU: `M45-DEMO-PLUS5-6aef7312e701`.

Used supported fixture SQL: one transaction inserts fresh stock=10 and a new pending outbox `inventory.units_received` / aggregate `stock` / schema 1 / database-clock occurrence with quantity 5. Collision queries preceded writes; unique constraints provide a final guard. This demonstrates the pipeline, **not atomic producer/business-outbox integration**.

The running relay published it and the running consumer committed stock=15 and one receipt (before=10, quantity=5, after=15, completed_at present). Outbox remains processed, attempt_count=1. A temporary Rust operator helper used the production `RabbitMqPublisher` (persistent/mandatory/confirmed), restoring the complete immutable receipt envelope and independently comparing **every field** with the persistent outbox, including timestamp instant. It republished the same ID/content; stock stayed 15 and original receipt completion/content stayed unchanged. No diagnostic basic.get observer competed with the consumer. The helper/envelope remain private local evidence, excluded from Git; temporary source was removed from backend/examples.

Also stopped consumer intentionally (measured graceful stop ~0.246 s), observed it remain STOPPED, then published another identical copy. Independently observed ready=1/unacked=0/consumers=0 while stopped; start resumed it and restored 0/0/1 without another effect. After final app recreation, another identical publication again produced the sanitized `Duplicate(stock_after=15)` outcome and successful ACK. Broker counters finally showed publish=4, deliver_get=4, ACK=4, redeliver=0. This is deliberate duplicate publication, not a crash-redelivery experiment. Exactly one demonstration stock/outbox/receipt remains; no automatic fixture deletion.

Management statistics lagged one early observation; subsequent independent inspection confirmed the retained message. No extra event was published to compensate. Supervisor status returns exit 3 for STOPPED/STARTING; this is a state observation, not a control failure.

## Verification, controls and recovery

Actual checks: locked binary build; Compose build/config; source/installed config and launcher equality; six migration checksums/schema; authenticated PostgreSQL/management checks; real retained-message stop/start/duplicate demonstration; HTTP health 200 and replay route 404; supervisorctl/supervisor named controls; stable child PIDs with no restart loop over the observation interval. An isolated Supervisor consumer regression passed (1 test), retaining its explicit test opt-in override even though normal local startup is now enabled. An initial test compilation was interrupted by app recreation (exit 137); the full focused case was then rerun successfully and cleanup independently checked. No shared fault injection was used.

A Chromium probe opened the **shared** frontend at port 5173: expected title/button, zero runtime/console errors and zero failed/error resources. No UI source or API integration changed. Presentation remains a local simulation. Current app logs contained zero JSON WARN/ERROR records; in-memory scans found no configured passwords or payload fields. Only sanitized outcomes were reported. No raw environment/log dump is tracked. Documentation validation and `git diff --check` passed. Final checks found no relay_it_/relay_migration_/relay_restore45_ databases, relay.it. broker resources or isolated Supervisor paths.

Containment if processing looks wrong:

```sh
docker compose exec -T app supervisorctl stop consumer
docker compose exec -T app supervisorctl status consumer
# Stop new outbox publications as well when investigating the entire pipeline:
docker compose exec -T app supervisorctl stop relay
# Resume only after reviewing durable state/topology:
docker compose exec -T app supervisorctl start relay
docker compose exec -T app supervisorctl start consumer
```

Intentional stop remains stopped for the current manager. **Consumer autostart=true means manager/container restart resumes it.** For a persistent hold, set consumer autostart=false in normal.conf, rebuild/install affected app configuration under a controlled stop, and verify agreement before any manager restart. Never restart/recreate the manager casually during containment. A subscription stop keeps ready messages; interrupted unacknowledged messages can redeliver. Preserve receipts and immutable identity; uncertainty is resolved by same-event deduplication, not deleting receipts or incrementing stock manually. [Supervisor lifecycle semantics](https://supervisord.org/configuration.html#program-x-section-settings).

Recovery preparation: stop relevant writers, preserve current database/broker state and take another protected backup before deciding. Check the retained dump/checksum and restore it to a **new isolated database first**, using compatible tooling and inspected source ownership. Compare schema/history/counts and post-checkpoint work; keep that investigation isolated. Shared restore requires an explicit reconciliation decision because it can erase later data and deduplication history without reversing broker publications, ACKs or external effects. No automatic restore/down migration is a recovery strategy. Prefer a reviewed forward repair when possible. Do not restore this pre-rollout dump over shared state merely to get a green result.

## Milestone 5 entry

Local rollout prerequisites are complete; no missing user-specific configuration blocks continuing development. Before starting Milestone 5: choose and record its actual scope/acceptance criteria (this repository has no detailed Milestone 5 contract); reread this state/4.4 contracts, confirm named process/queue health and six checksums, preserve the retained fixture/receipts, and use isolated test resources for fault/capacity experiments. Any future shared migration requires a new quiesced verified backup. Define workload/message admission and capacity semantics before concurrency/backpressure changes. Producer transactions and live frontend integration remain separate planned work.

Before broader deployment, additionally install/review least privilege, TLS/HA, independent backup retention including broker/global dependencies and capacity/observability. They are explicit limitations, not claims delivered by this local rollout. Replay remains disabled; REPLAY_OPERATORS would be required only for separately chosen privileged API activation, never in chat/frontend. Viva Bem and Engineering Library were not modified.
