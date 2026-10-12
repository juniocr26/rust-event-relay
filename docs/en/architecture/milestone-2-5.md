[Português brasileiro](../../pt-BR/architecture/milestone-2-5.md) | [README](../../../README.md)

# Milestone 2.5 — Worker composition and operation

Implemented 2026-10-12 from clean `7be956c`. The requested 2.4 handoff was absent; [2.4 source and semantics](milestone-2-4.md), tests and Git history supplied the baseline. No applicable project/ancestor AGENTS.md was found. Historical milestone sections elsewhere retain their dated scope; this document describes the current worker.

## Composition and admission

[src/bin/relay.rs](../../../src/bin/relay.rs) loads `.env`/configuration, initializes JSON tracing, constructs a dedicated SQLx pool and repository, checks ownership columns read-only, constructs the RabbitMQ lifecycle and runs the application worker. Credentials use existing separate POSTGRES and RABBITMQ fields; DATABASE_URL is not a second worker configuration path. Missing schema/privileges, invalid configuration, telemetry or initial PostgreSQL setup fail with sanitized diagnostics and nonzero exit before polling. RabbitMQ's existing Unavailable setup error cannot distinguish a topology/authentication failure from a runtime outage; failed connection attempts are explicitly paced until shutdown. Runtime persistence failures are paced, with no unrestricted source-chain logging.

[application/worker.rs](../../../src/application/worker.rs) uses static dispatch over repository ports and `PublisherLifecycle`. This focused lifecycle seam owns connection admission, usability checks and consumption on close; concrete SQLx/Lapin construction remains in composition. There are no worker-spawned tasks, delivery queues, batching or parallel attempts. SQLx/Lapin's internal runtime tasks are adapter implementation details, not additional delivery admissions. One admitted attempt creates a new `AcquireRequest`/UUID token and calls `deliver_once` exactly once. Snapshots never grant ownership. Success immediately permits a new attempt; NoEvent means no acquisition in that invocation, not an empty backlog.

## Validated settings

All durations below use integer **milliseconds**, strictly positive. Process values override `.env`. Invalid, overflowing, zero, negative or out-of-range values fail startup; diagnostics name keys/ranges without echoing input. Lease conversion to signed whole microseconds is checked. Pool minimum is SQLx's zero default; one sequential delivery needs no larger minimum.

| Setting | Default | Inclusive valid range | Purpose |
| --- | ---: | --- | --- |
| RELAY_LEASE_MS | 60000 | 1–3600000 | Durable ownership validity |
| RELAY_IDLE_MS | 1000 | 1–60000 | NoEvent pacing |
| RELAY_FAILURE_MS | 1000 | 1–60000 | Unsuccessful delivery pacing |
| RELAY_RECONNECT_MS | 5000 | 1–60000 | Replacement/failed connection pacing |
| RELAY_POOL_MAX | 2 | 1–16 connections | Dedicated pool limit |
| RELAY_POOL_ACQUIRE_MS | 5000 | 1–60000 | Pool admission and initial connection wait |
| RELAY_STATEMENT_MS | 5000 | 1–60000 | PostgreSQL session statement timeout |
| RELAY_LOCK_MS | 3000 | 1–60000 | PostgreSQL session lock timeout |
| RELAY_DRAIN_MS | 15000 | 1–20000 | Current-operation drain after signal |
| RELAY_SHUTDOWN_MS | 25000 | 1–25000 | Overall drain + publisher/pool cleanup |

Drain must be strictly smaller than overall shutdown. Session statement/lock limits are set only on this pool through PgConnectOptions, preserving repository SQL, classification and lease predicates. They bound individual server statements/lock waits, not an entire transaction, network response or delivery. Pool admission is not a query deadline. RabbitMQ retains its existing connect/publish timeout defaults of 10000 ms (1–300000); gate and send/confirm have separate budgets, and connection/setup/failed-setup close can each consume a budget. There is no new delivery-operation deadline or lease renewal. Defaults leave useful lease headroom, but admission, multiple statements, network latency, clock changes and process stalls can consume it; these settings cannot mechanically prevent expiration. Database ownership still cannot fence external publication.

## Explicit outcome policy

All returned outcomes are logged with sanitized primary **and** secondary results, safe event/ownership IDs and acquisition metadata, never envelopes, payloads, connection strings or diagnostic source chains.

| Outcome | Worker decision | Durable interpretation |
| --- | --- | --- |
| NoEvent | Idle delay | No claim in this invocation |
| Acquisition failure: Unavailable/InvalidStoredData/OperationFailed | Failure delay; continue | No acknowledged claim |
| Acquisition CommitUncertain | Failure delay; fresh token on later invocation | Lease/count may already be committed; never fabricate a claim |
| Published + Applied completion | Continue immediately | Confirmed broker acceptance AND acknowledged durable completion |
| Published + OwnershipLost | Failure delay | Acceptance confirmed, completion unacknowledged |
| Published + ordinary completion error | Failure delay | Acceptance confirmed; do not infer rollback |
| Published + CommitUncertain completion | Failure delay | Completion may already be durable |
| Publication Unavailable | Close/replace; reconnect delay | Use case attempted release once |
| Publication Serialization | Failure delay; retain usable connection | Use case attempted release once |
| Publication Unroutable | Failure delay; retain usable connection | Definitive routing failure; release once |
| Publication Rejected | Failure delay; retain usable connection | Definitive rejection; release once |
| Publication Uncertain | Close/replace; reconnect delay | Lease retained; acceptance unknown |

| Independent release result | Reporting and worker action |
| --- | --- |
| Applied | Log acknowledged release; retain primary-error pacing/replacement |
| OwnershipLost | Log lost authority; same primary-error policy |
| Unavailable/InvalidStoredData/OperationFailed | Log classified secondary failure; same primary-error policy |
| CommitUncertain | Log uncertainty; transition may have committed; same primary-error policy |
| LeaseRetained | Log retention after publication uncertainty; replace publisher |

Completion failures never cause compensating release or immediate republication of the claim. A later invocation acquires through durable predicates with a fresh token; original event identity survives. Release does not change available_at, last_error or attempts. Failure pacing prevents immediate reacquisition from becoming a hot loop, but is not a durable retry schedule. There is no exponential backoff/jitter, new retry counter, fairness, quarantine or DLQ. Permanently invalid/failing due rows can still obstruct progress, and acquisition-exhausted rows remain pending.

## Publisher replacement

Before acquisition the loop checks actual retired/channel/connection status. A known unusable publisher is consumed and closed, then replaced before another attempt. Disconnection may still race the usability snapshot; Unavailable covers that case. Unavailable and Uncertain publication outcomes explicitly trigger replacement; Serialization/Unroutable/Rejected do not when the connection remains usable. Failed connections wait interruptibly; no acquisition occurs while disconnected. A replacement does not reconcile earlier acceptance or replay the previous publication. Cancellation after sending retires the adapter but yields no typed outcome; shutdown closes/drops it without blind release. No automatic protocol recovery or hidden adapter retry is enabled.

## Shutdown and supervision

SIGINT/SIGTERM use the established application signal convention. Signal is checked before every admission; idle/reconnect waits interrupt immediately. Current connect/close/delivery operation may drain for 15 seconds. After that its future is dropped and a log explicitly warns that durable/external effects may have occurred without an outcome. No abandoned claim is blindly released. Lease expiration permits later eligibility; it does not prove nonacceptance or rollback.

One absolute deadline starts at observed shutdown: default 25 seconds. Publisher close and SQLx Pool::close share the remaining budget; borrowed connections cannot create an unbounded wait. Cleanup timeout/failure is logged; intentional signal exit is zero even if cleanup had to be bounded. Initial read-only setup can be cancelled directly and closes any constructed pool within budget. Signal-handler installation still follows HTTP's existing fail-fast convention. Tokio deadlines require cooperative polling; SIGKILL/power loss cannot drain.

[docker/supervisor/supervisord.conf](../../../docker/supervisor/supervisord.conf) preserves `http` and adds the executable [relay-worker launcher](../../../docker/supervisor/relay-worker) using `/app/target/debug/relay`, never cargo run. Both use developer permissions, stdout/stderr and the private control socket/alias. relay autostart is **false** pending shared-schema authorization. Both have TERM/30-second child stop limits; Compose allows 65 seconds for sequential group shutdown plus margin. `autorestart=unexpected`, `exitcodes=0` restarts unexpected failures, while intentional stop or successful signal exit does not restart an established RUNNING child. A failure during the one-second STARTING period is retried up to three times then FATAL, even with exit zero. Runtime RabbitMQ outages reconnect inside the worker rather than repeatedly restarting the process.

```bash
docker compose exec app cargo build --locked --bins
# Install changed launchers/config (only app is rebuilt/recreated):
docker compose build app
docker compose up -d --no-deps app
docker compose exec app supervisorctl status
docker compose exec app supervisorctl stop relay
docker compose exec app supervisorctl start relay
docker compose exec app supervisorctl restart relay
docker compose exec app supervisorctl stop http
docker compose exec app supervisorctl start http
docker compose exec app supervisorctl restart http
```

`supervisor` accepts the same arguments; either supports an interactive console. `stop/start/restart all` now targets **both** app programs, including relay even with autostart disabled; it does not touch database/broker services. Do not `start all` against the pending shared schema. Source changes require stop/build/start; physical installed config changes require app rebuild/recreation.

## Authorized rollout prerequisite

Read-only `sqlx migrate info` on 2026-10-12 confirmed 20261009000000 pending; it was not applied to shared data. Current relay remains STOPPED; HTTP /health proves HTTP liveness only. Review data and authorize the existing ownership migration before executing:

```bash
# Only after authorization for this shared database:
docker compose exec -T app sqlx migrate info
docker compose exec -T app sqlx migrate run
docker compose exec -T app sqlx migrate info
docker compose exec app supervisorctl start relay
docker compose exec app supervisorctl status relay
```

No reset, credential change or infrastructure service recreation is needed. Autostart can be enabled later as an explicit versioned operational decision, followed by the app-only installation workflow.

## Validation and remaining scope

[worker tests](../../../tests/worker.rs) use paused time and notifications for fresh tokens, paced outcomes, replacement policy, stopped admission, interruptible waits, drain and cancellation/overall cleanup. [composition smoke tests](../../../tests/worker_smoke.rs) use test-owned migrated databases and UUID broker resources; they verify original identity/payload, one acquisition, acknowledged processed state, SIGTERM and isolated physical Supervisor named/collective controls. Only fixture consumers inspect messages; no production consumer is added. Controlled dependencies prove application policy, not database crash rollback or broker nonacceptance.

See [dated evidence](../testing/validation-results.md#milestone-25--2026-10-12) and [handoff](../../../Reports/MILESTONE_2_5_HANDOFF.md). Milestone **2.6** adds integrated crash/failure/recovery experiments, two-worker recovery and milestone closure. This happy path is not recovery evidence. No throughput, fairness, consumer effects or end-to-end at-least-once guarantee is claimed. A graphical interface may be considered after core completion for a concrete operational need; no UI/readiness framework/metrics/dashboard is part of 2.5.
