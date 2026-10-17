[Historical record: current installation and addresses](../operations/application-scenarios.md)

[Português brasileiro](../../pt-BR/operations/milestone-6-1.md) | [Catalog](../index.md)

# Milestone 6.1 — real delivery with persistent evidence

Scope: successful delivery only. [ADR 011](../adr/011-persistent-demo-runs.md) defines identity, transactions, authorization, limits and retention. Crash/race/ACK-loss/deduplication remain explicitly illustrative. No CDC, chaos endpoint, management credential in a browser bundle or exactly-once transport claim.

## Start and stop

Run from the repository root. Docker Desktop and the existing Rust development Dockerfile/toolchain are required. No host Rust/Node installation is needed. The toolchain image is tagged `relay-demo61-toolchain`, separate from the shared app image. Initial builds may download dependencies. Never substitute the ordinary `compose.yaml` in these commands.

```sh
python3 scripts/demo-stack.py init
python3 scripts/demo-stack.py infra
python3 scripts/demo-stack.py build
python3 scripts/demo-stack.py migrate
python3 scripts/demo-stack.py start
python3 scripts/demo-stack.py status
# Normal stop retains all evidence volumes:
python3 scripts/demo-stack.py stop
# Resume infrastructure then application services:
python3 scripts/demo-stack.py infra
python3 scripts/demo-stack.py start
```

`init` creates private ignored `.demo.env` with three different random secrets; existing credentials are preserved. Open that file locally and enter **DEMO_TOKEN** in the modal. It is a demo credential, never the privileged replay token. It is kept in browser memory; only the request UUID is kept in sessionStorage for safe same-tab retry/reload. No credential is compiled into VITE_* variables. For another intentional run, use **Prepare a new run** after confirmed completion. An uncertain submission keeps its original request identity, including after closing/reopening the modal.

Migrations verify Compose label `relay-demo61/postgres`, database/user/server port `relay_demo61/demo61/5432` before SQLx execution. They do not migrate the existing shared database. Application processes do not run migrations. Application instances mount only their read-only binary volume, without the repository or shared dotenv; the explicit tools service alone can compile/format. No Docker socket is mounted. Each run owns unique warehouse/SKU/fact/event/run IDs. A separate broker prevents process/queue interference with the original shared example.

| Service | Address | Identity |
| --- | --- | --- |
| Browser | http://localhost:5174 | Exact permitted browser Origin |
| Demo API | http://localhost:8081 | Same-origin browser proxy `/demo`; bearer required |
| PostgreSQL | localhost:5441 | Database `relay_demo61`, user `demo61`, password from `DEMO_DB_PASSWORD` locally |
| RabbitMQ management | http://localhost:15681 | User `demo61`, password from `DEMO_BROKER_PASSWORD` locally; vhost `demo61` |
| AMQP inside stack | rabbitmq:5672 | No host AMQP port exposed |

Broker topology: durable direct exchange `relay.events`, routing key `event`, durable source queue `relay.local`; consumer responsibility `inventory.stock.v1`. Separate quarantine exchange `relay.quarantine` → queue `relay.quarantine.inventory`, key `inventory`. Management counts are queue aggregate observations, never evidence of one Event ID. No decorative activity plot is supplied; exact publisher-confirm and broker ACK-receipt instants are unavailable.

## API and evidence

All requests require `Authorization: Bearer <local demo token>` and `X-Demo-Client: demo61`. Browser Origin, when present, must equal `http://localhost:5174`. No CORS, cookie authentication or operator actions. One trusted local demo principal can inspect all runs of this dedicated stack; this is not public multi-tenant authorization.

- `POST /demo/runs`: strict JSON `{ "request_id": "<UUID>", "scenario": "normal", "initial_stock": 10, "quantity": 5 }`. 202 means producer commit acknowledged, or the same existing committed run reconciled. It does not mean successful delivery.
- `GET /demo/runs/{request_id}`: authorized read-only evidence, 404 when no committed request is observed in that snapshot. Reuse this request UUID after an uncertain submission; absence alone does not justify creating another identity.
- 400 unsupported/malformed intent; 401 authentication; 403 browser/header boundary; 409 divergent identity reuse; 413 oversized body; 422 known producer transaction rejection; 429 capacity/read budget; 503 unavailable or unknown commit (inspect the sanitized error code). An HTTP deadline or failed read never proves durable execution failed.

A transaction serializes request reconciliation and capacity admission with one demo-only advisory lock. It inserts fixture stock 10, `demo.runs`, `demo.goods_receipts` (receipt of 5 units as a producer fact) and `relay.outbox_events`. The producer never increments stock to 15. PostgreSQL supplies occurrence time at its microsecond precision; the existing relay restores that canonical envelope before publication. Repeating identical submissions returns the original run/event without resetting stock or reinserting business/outbox facts. Database uniqueness and transaction locking protect concurrent requests, including uncertain-commit recovery.

Evidence contains submitted input, producer fact ID, Run ID, Event ID, warehouse/SKU, accepted_at, outbox status/attempts/availability/creation/completion, receipt consumer/event/before/quantity/after/completed_at, current stock and observed_at. `accepted_at` is the run insertion timestamp within the acknowledged producer transaction, not an exact COMMIT timestamp. Receipt, current stock and relay processed are independent evidence. A receipt can commit before relay completion. HTTP success returns a fresh SQL statement snapshot; the browser explicitly labels cached/error/deadline observations stale. There is no atomic distributed broker snapshot or inferred live lease history.

## Independent inspection

Connect using your database client with the table above or `psql -h localhost -p 5441 -U demo61 -d relay_demo61 -W` (enter the private password at the prompt). In the existing modal, **Where to verify** shows read-only SQL already filtered to that run's IDs. Schemas: `relay` stores outbox publication/retry/replay; `consumer` stores processing receipts and inventory effects; `public` holds SQLx migration metadata; `demo` holds run correlation and producer business records.

```sql
BEGIN READ ONLY;
-- Set these to identifiers copied from the modal in psql:
\set run_id '01a149f0-bb7e-74db-96f4-750800816a7c'
SELECT * FROM demo.runs WHERE run_id = :'run_id'::uuid;
SELECT * FROM demo.goods_receipts WHERE run_id = :'run_id'::uuid;
SELECT o.id,o.status,o.attempt_count,o.processed_at
FROM relay.outbox_events o JOIN demo.runs r ON r.event_id=o.id
WHERE r.run_id=:'run_id'::uuid;
SELECT c.consumer_name,c.event_id,c.stock_before,c.quantity,c.stock_after,c.completed_at
FROM consumer.receipts c JOIN demo.runs r ON r.event_id=c.event_id
WHERE r.run_id=:'run_id'::uuid;
SELECT s.warehouse_id,s.sku,s.units FROM consumer.stock s
JOIN demo.runs r ON r.warehouse_id=s.warehouse_id AND r.sku=s.sku
WHERE r.run_id=:'run_id'::uuid;
COMMIT;
```

Retained operator demonstration: Run ID `01a149f0-bb7e-74db-96f4-750800816a7c`, request `19c660e8-3f75-4a92-a961-3ad60dd95529`, Event ID `01a149f0-bb7e-74db-96f4-750996a3bcc8`. Direct SQL independently observed processed / receipt 10+5=15 / stock 15. These are study identifiers, not permanent public fixture requirements.

## Budgets and manual retirement

Two concurrent demo HTTP requests, global 10/s, body 1024 bytes, HTTP 8s, pool 2 / acquire 2s / statements 3s / locks 1s. At most four unfinished pending runs and 100 retained runs; capacity stops new admission without eviction. One relay and consumer, each pool max 2 plus a readiness sampler connection; one in-flight delivery/prefetch 1. Their existing finite connect, publish, operation and shutdown budgets remain. Worker/consumer exporters remain loopback-only, four connections and fixed registry labels; demo API does not add another exporter. Database max_connections=30. Runtime limits are 256 MiB/0.5 CPU per Rust process and PostgreSQL, 512 MiB/1 CPU per broker/frontend, PID limits 128 (broker 256), rotated logs 2×5 MiB. Build tools use 3 GiB/2 CPU; test Chromium uses 2 GiB/2 CPU/512 PIDs. These bound the selected demo, not all host/disk consumption.

No TTL, purge or automatic deletion. At the 100-run stop, export desired evidence and retire the entire **dedicated** topology; this intentionally removes all demo evidence. Do not remove individual receipts while redelivery remains possible. Exact sequence, only when intentionally retiring the demo:

```sh
python3 scripts/demo-stack.py stop
# Removes only this project's stopped containers/network; volumes still retained:
docker compose --env-file .demo.env -p relay-demo61 -f compose.demo.yaml down
# First remove all dedicated broker deliveries, then SQL evidence:
docker volume rm relay-demo61_demo-rabbitmq
docker volume rm relay-demo61_demo-postgres
```

Never run these volume removals for normal stopping; never use shared stack names, global prune, queue purge or wildcard SQL deletion. Retain `.demo.env` while restarting with retained volumes; changing its passwords does not rotate credentials stored in an existing broker/database.

## Validation and next slices

Focused tests are in `backend/tests/demo_runs.rs`; run with the tools service and `--ignored --test-threads=1`. They create/drop exact unique databases, separate from retained runs. `python3 -B scripts/check-migrations.py --demo` checks seven upgrade checksums and demo/consumer/replay down/re-up on a disposable database in the dedicated cluster. `python3 scripts/test-demo-browser.py` uses a **different** fixed test project, fresh SQL/broker volumes and credentials, no host ports, real application binaries/API and Chromium. It removes only that test project's resources, never retained operator evidence. Do not run two copies of the fixed-project browser harness concurrently. Browser testing requires downloading its test-only image dependencies on first use. Local generated screenshots are temporary test artifacts.

6.2 prerequisites: bounded durable idempotent duplicate action; publish the identical stored envelope/message_id; observe a real correlated consumer **Duplicate** outcome in addition to Applied. One receipt and stock 15 must remain. Two publisher confirmations or stock unchanged alone do not establish Duplicate handling. 6.3 adds expanded inspection/session recovery and richer uncertainty UX; 6.1 already retains request identity and supports minimal safe authorized evidence reads. Production deployment, public authentication, chaos controls and broker activity sampling remain outside this slice.

### Local acceptance recorded — 2026-10-17

Base `f6b0ec11e8035dedbfb7ed3002157302aa827bbb`; pre-existing EN/PT-BR catalog edits and preparation documents preserved. Seven focused real SQL tests passed (atomicity, concurrent idempotency/conflict, statement/deferred-COMMIT rejection, lost COMMIT acknowledgement, post-commit cancellation, authorization/unavailability and both admission/retention caps). Selected existing regressions: 58 passed; library tests: 25 passed, two real-broker opt-ins not rerun in that library invocation. Cargo fmt, strict all-target Clippy, binary build and rustdoc passed. Frontend type-check/build passed. Final real browser campaign: 21 passed, including actual clipboard reads, independent SQL receipt/stock reconciliation, paused test consumer and stopped test API, five viewport widths, focus/reduced motion, close/same-request recovery and illustrative scenarios. Screenshots at 320/1920 were inspected. No route mocks were used.

Seven ordered SQLx upgrades/checksums, no-op and demo/consumer/replay down/re-up passed on an exact disposable database; it was removed and absence verified. Compose config validation, Python parsing, four documentation-checker tests, 117 Markdown link checks and whitespace checks passed. Initial harness corrections included missing tools profile, exit 137 with the small frontend test budget, and two old illustrative text expectations; the final run passed after those corrections. The Supervisor process-control script was attempted on the host and stopped at its missing control-socket prerequisite; it was not run against shared services because it would stop/restart them.

The retained demo run above survived full normal stop/start and subsequent direct SQL verification, with seven successful migrations and exactly one retained operator run. Shared event `481fb751-81d5-46b7-aea2-eed344084725` remains processed/attempt 1, receipt 10+5→15 at `2026-10-16 15:34:23.514788+00`, stock 15; shared `demo.runs` is absent. No shared migrations, fault injection, resets, stage, commit or push. Browser-test containers/network/SQL/broker/node volumes were removed; retained demo services remain running. Remote CI and production readiness are not asserted.
