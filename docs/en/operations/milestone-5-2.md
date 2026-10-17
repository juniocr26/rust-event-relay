[Current 5.3 measurements, remotely validated 5.2 rollout and Milestone 5 closure](milestone-5-3.md). Earlier pending statements below are historical.

[Português brasileiro](../../pt-BR/operations/milestone-5-2.md) | [Catalog](../index.md)

# Milestone 5.2 — readiness and operational freshness

## Validated 5.1 rollout

On 2026-10-17, the clean checkout and tracking branch both identified `9b0323af3fad769b1d0813e0330b32e87d64a91c`. The GitHub API confirmed [run 38974809458](https://github.com/juniocr26/rust-event-relay/actions/runs/38974809458): check, documentation, frontend, postgres-integration, rabbitmq-integration and integrated-recovery all succeeded for that exact SHA. This supersedes the 5.1 implementation document's historical pending-CI/deployment statements; its original evidence is retained.

Initial containers were stopped. The user explicitly authorized starting services and consumer. A dependency start and a one-off `cargo build --locked --bins` were executed; the next inspection found all services, including app/frontend, running. The available observations do not establish which actor started app/frontend. No application restart/recreation was needed: all three loaded executable hashes matched the on-disk artifacts confirmed by the locked 5.1 build. No manager configuration, credential, volume, topology or migration was changed. Six successful migration versions remained present. Safe metadata inspection found both queues empty, source consumers=1, quarantine consumers=0. The retained event `481fb751-81d5-46b7-aea2-eed344084725` remained processed/attempt_count=1, stock=15 and receipt before=10/quantity=5/after=15/completed_at=`2026-10-16 15:34:23.514788+00`. No demonstration publication or fetch was used.

| Loaded program | SHA-256 matching disk |
| --- | --- |
| HTTP | `77f940a7a64330e4f9d7343728ebca5a220bf3daca6fd3404a37550df83c8f26` |
| Relay | `38e872d2a9bf6a6bf7702ae4b971675c20e80e904fb9e44c1b7391bba5ba54cc` |
| Consumer | `cda31435b1b7c0acbe7377516680a67b59a9805574261855c0cda44608593dc6` |

The binaries have no embedded revision attestation; clean-source locked build plus loaded/disk hash equality is the source evidence. All actual exporters returned 200 in the app namespace with their correct role labels and resource limits; `/health` returned `200 ok`. Exporter address variables were absent from the existing container injection, so the reviewed defaults applied. Replay remained disabled. No image rebuild was necessary. Consumer is intentionally RUNNING per this session's explicit instruction.

## Admission contract

`GET /ready` extends each existing loopback exporter. `/health` on the HTTP application remains liveness (`200 ok`), independent of workers, broker and database. Readiness means this particular process currently has fresh observations supporting admission of its intended work. It does not prove future delivery, business completion, backlog emptiness, durability, HA or the availability of another role.

| Role | Startup / required observations |
| --- | --- |
| HTTP | Valid configuration, operational listener, application listener bound; no database or broker requirement when replay disabled. Enabled replay additionally requires credential parsing, initial replay schema preflight and recurring replay database/schema observations. |
| Relay | Valid configuration, initial database/schema preflight, lifecycle admission enabled, recurring outbox database/schema observation, connected usable publisher/channel. Empty acquisition is still ready. |
| Consumer | Valid configuration and initial consumer schema preflight, lifecycle admission enabled, recurring stock/receipt schema observation, successfully established subscription with usable source connection/channel and independent quarantine publisher. An idle subscription is ready. Stream termination/cancel/recovery clears broker readiness. |

Startup failures may expose starting briefly, then exit and become absent; this milestone does not change startup failure/retry policy. Preflight/observations verify selected columns and SELECT privileges; they cannot prove future mutation success, every constraint/function/grant, concurrent schema stability or broker routing after observation. Existing delivery validation remains authoritative. No business transaction is performed as a health probe.

States: `starting` before admission; `ready` when every required observation is successful and fresh; `not_ready` for unknown, failed or stale dependencies; `stopping` from signal recognition, before draining work. Stopping is sticky: late successes cannot restore readiness. These are reporting transitions; they do not introduce a second work-admission gate or change existing retry, delivery, deduplication or cancellation semantics.

Closed reason vocabulary, in precedence order: `stopping`, `starting`, then database (`database_unknown`, `database_stale`, `database_unavailable`), broker (`broker_unknown`, `broker_stale`, `broker_unavailable`), otherwise `ready`. The JSON response contains `process`, `state`, one `reason`, `freshness_seconds`, monotonic `observation_age_seconds` and last successful observation Unix timestamps (null when unknown/not applicable). HTTP 200 only for ready; all other states 503. Responses use `Cache-Control: no-store`; no URLs, secrets, payloads, queue names, identities or error chains. First reason wins; metrics expose dependency freshness individually. No query/connection is opened by a readiness request or scrape.

## Observation budgets and absence

Database observation starts immediately after preflight, then waits **5 seconds after completion**, without overlap/catch-up. One dedicated lazy pool with maximum **1 additional connection** per database-dependent role; pool acquisition and the entire sequential sample share a **2-second client deadline**, server statement timeout **1000 ms**, lock timeout **500 ms**, default read-only transactions. Each role executes its existing column preflight with `LIMIT 0` (consumer: two statements). No row materialization/count/scan of backlog is requested. Relation/catalog lock and planning costs remain; a timeout does not prove constant server cost. Cancellation may leave server work until its own deadline. Shutdown cancels and joins the sampler (1 s), then bounds pool close (1 s); Drop aborts its task. The worker's existing cleanup deadline is retained.

Total configured PostgreSQL connections now include business pools **plus 1 per sampler**: default relay 3, consumer 3; HTTP 0 when replay disabled, 3 when enabled. These are per-process limits, not cluster quotas. No new environment variable is introduced: interval/deadline/freshness are named constants and deliberately not individually tunable. Existing database credentials are reused; no new broker management credentials/polling.

Broker observations use in-memory installed-client connection/channel status and actual lifecycle results, never test publication. Relay refreshes before delivery and reports false on connection/replacement; consumer refreshes after subscription and once per second while awaiting deliveries, and clears before reconnect/close. Idle tick does not restart subscription or change broker credit. An interrupted/long operation can delay refresh, conservatively producing stale. A transport considered connected can fail immediately afterward; this is observation, not an end-to-end guarantee. Silent half-open connections remain limited by client heartbeat/failure detection. These limitations apply even with empty queues.

Freshness uses Tokio monotonic elapsed time: **age >15 seconds is stale**, including successful old observations. Wall-clock Unix timestamps are diagnostic only and never decide freshness. Failed samples update attempt age/failure count but preserve last-success timestamp; they never restore health. Unknown timestamps are omitted in metrics and null in JSON, never substituted with successful zero. A stopped sampler eventually makes the process stale.

| Observation | Meaning / operational action |
| --- | --- |
| Reachable `/ready`, 200 | Fresh process-local admission evidence. |
| Reachable `/ready`, 503 | Inspect finite reason, dependency freshness and logs. |
| Reachable exporter, stale reason | Listener works but required observation no longer current. |
| Endpoint absent/unreachable | No process report exists: inspect external scrape `up`, namespace, Supervisor and logs. Network failure/exporter failure/process exit are not distinguished by absence alone. |
| Intentionally stopped process | No endpoint expected; use named Supervisor state and operator intent externally. Do not synthesize worker health in HTTP or call this a reported 503. |

No collector, Prometheus/Grafana deployment or live frontend is introduced. A process cannot report after exit; `up` must come from an external scraper, and intentional stop requires external context. Process-local counters/state reset on restart.

## Metrics and deferred backlog

Existing metric contract remains. New labels are bounded: `process` (three roles), `dependency` (`database`, `broker`) and `reason` (nine values).

| Metric | Semantics |
| --- | --- |
| `relay_readiness_pool_connections_limit` | Dedicated observation pool maximum (0 or 1); additional to the business pool gauge. |
| `relay_ready` | 1 only for current ready snapshot, else 0. |
| `relay_readiness_reason{reason}` | Fixed one-hot reason set, including zeros. |
| `relay_dependency_observed{dependency}` | 1 after any completed observation, else 0. Only required dependencies emitted. |
| `relay_dependency_fresh{dependency}` | 1 when an observation exists and age ≤15 s; a fresh failed observation is still unready. |
| `relay_dependency_failures_total{dependency}` | Saturating failed observation counter, reset on restart. Broker counts false lifecycle/status observations, not distinct incidents. |
| `relay_dependency_last_success_timestamp_seconds{dependency}` | Last success Unix seconds, omitted before success. |
| `relay_dependency_observation_age_seconds{dependency}` | Monotonic age of last completed attempt, omitted before observation. |
| `relay_dependency_observation_duration_seconds{dependency}` | Latest sample elapsed seconds; broker in-memory status reports zero duration. Not a latency histogram. |

Readiness snapshot is internally coherent under one short mutex; other 5.1 phase metrics remain independent observations. Exporter retains four-client/1024-byte/2-second bounds. No dynamic error labels or telemetry queues. Instrumentation has no business side effects.

**Backlog sampler deferred deliberately.** No count, age gauge or false zero approximation is added. Before implementing it: inspect plans/indexes and realistic table size; define mutually exclusive pending live-lease / unleased delayed / unleased due candidates, separate invalid/exhausted candidates and terminal dead_letter; specify PostgreSQL authoritative cutoff in one statement snapshot, oldest valid eligible age, dedicated pool/statement/scan budget, cancellation and unavailable/stale representation. Lease takes precedence over due/delayed classification to avoid overlap. Readiness schema probes do not measure backlog, and no cross-PostgreSQL/RabbitMQ atomic snapshot is claimed. Broker metrics remain external; no management polling is duplicated.

## Inspection, validation and next deployment

Default addresses: `HTTP_METRICS_ADDR=127.0.0.1:9100`, `RELAY_METRICS_ADDR=127.0.0.1:9101`, `CONSUMER_METRICS_ADDR=127.0.0.1:9102`; append `/metrics` or `/ready`. Existing parser requires loopback/nonzero ports, all distinct and distinct from the HTTP port; OS bind collision fails startup. No exporter port is published by Compose. Host localhost is a different namespace.

```sh
docker compose exec -T app supervisorctl status
docker compose exec -T app python3 -c 'import urllib.request; print(urllib.request.urlopen("http://127.0.0.1:9101/metrics", timeout=3).read().decode())'
# For /ready, urllib raises HTTPError for a truthful 503; inspect status/body.
# Shared runtime remains 5.1; readiness requires later deliberate 5.2 deployment.
docker compose exec -T -e CARGO_TARGET_DIR=/app/target/m52 app cargo test --locked
```

5.2 artifacts use `/app/target/m52`, preserving shared `/app/target/debug` executables. New source/local tests are not remotely validated or deployed. Before deploying 5.2: review diff, user-controlled commit/push, require CI for its exact SHA, review the extra database connection budget, build selected revision, capture named states/data/queues and deliberately restart only affected programs. Preserve consumer intent, replay setting and demonstration receipt; no migration required. Do not casually recreate Supervisor (consumer autostart=true).

Executed checks and any limits: [validation results](../testing/validation-results.md). Tests cover deterministic transitions/freshness/sticky shutdown and optional dependency, actual HTTP/relay/consumer endpoints, isolated PostgreSQL lock/deadline failure and recovery, and isolated subscription transport loss/recovery without publication. Existing delivery/consumer fault suites remain necessary.

**5.3 prerequisites/scope:** first review and remotely validate 5.2, deliberately deploy its selected revision and verify actual role exporters; retain demonstration/consumer intent. Then define reproducible isolated workload/payload distributions, resource/measurement budgets, warmup, backpressure/recovery protocol and exact cleanup before capacity experiments. No benchmark, concurrency increase, live frontend, business endpoint, quarantine replay/purge, HA/TLS or identity expansion belongs to 5.2.

Primary references: [probe separation](https://kubernetes.io/docs/concepts/workloads/pods/probes/), [RabbitMQ consumer lifecycle](https://www.rabbitmq.com/docs/consumers), [PostgreSQL deadlines](https://www.postgresql.org/docs/18/runtime-config-client.html). These inform semantics; test evidence establishes only the exercised schedules.
