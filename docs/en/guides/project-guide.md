# Current project guide — 2026-10-18 source review

Start at the [README](../../../README.md) for installation and the [implementation study](implementation-study.md) for the code-to-evidence reading path. `backend/` contains Rust domain, application ports/policy, PostgreSQL/AMQP/HTTP adapters and test definitions; `frontend/` contains Vue real-scenario/inspection components plus isolated illustrative fixtures. `backend/migrations` supplies nine ordered versioned migrations; `docker/supervisor` manages HTTP, relay and consumer. Runtime artifacts and test artifacts are separate.

One main Compose workflow uses `python3 scripts/start-local.py`; the dedicated demo stack is historical. Producer/run/action/history APIs and atomic consumer are implemented. Use real scenarios to inspect producer/outbox/consumer facts, then study crash/uncertainty tests independently of simulated playback. Historical commands and directory snapshots below belong to their original revisions, not current startup guidance. No remote URL or repository name is changed by this review.

## Historical reference (original dates and milestone scope)

[Historical record: current application guide and acceptance](../operations/milestone-6-3.md)

**Current study flow — Milestone 5.3:** outbox → relay acquisition → RabbitMQ confirmation → SQL relay completion → consumer receipt/stock → ACK-send. Retry/uncertainty, SQL dead_letter and broker quarantine are separate branches; authorized replay keeps identity and cannot repeat an already receipted effect. [Measured scenarios, full coverage and decision limits](../operations/milestone-5-3.md). Frontend remains a labelled local illustration; Milestone 6 is undefined.

Current native Cargo commands run in `backend/`; Docker app commands inherit `/app/backend`. See [frontend/backend](frontend-backend.md).

[Português brasileiro](../../pt-BR/guides/project-guide.md) | [README](../../../README.md)


# Project guide

## Current extension — Milestone 2.2

[Delivery state and ownership](../architecture/milestone-2-2.md) and [ADR 007](../adr/007-durable-delivery-ownership.md) now define durable lease recovery and separate acquisition/completion/release contracts. New migration `20261009000000_add_delivery_ownership` adds nullable token/acquired_at/expires_at with coherent pending-only leases. Earlier milestone sections below describe their original scope; earlier claims that ownership/attempt semantics are undecided are superseded by ADR 007. Production mutation adapters remain deferred to 2.3; reader SELECT and publisher behavior remain unchanged.


```text
.
├── backend/ (Cargo.toml, Cargo.lock, src/, tests/, migrations/)
├── frontend/ (Vue shell, package.json, package-lock.json, src/)
├── docker/ (Dockerfile, entrypoint.sh, sqlx.py, rabbitmq.conf)
│   └── supervisor/ (normal.conf, supervisor, relay-http, relay-worker)
├── docs/ (en/, pt-BR/)
├── Reports/
├── scripts/
├── .github/workflows/ci.yaml
├── .cargo-cache/ (ignored)
├── target/ (ignored)
├── .dockerized-postgres/ (ignored)
├── .env.example
├── compose.yaml
├── README.md
├── README.pt-BR.md
└── LICENSE
```

- `main.rs`: bootstrap and lifecycle logging; `lib.rs`: exposes testable modules.
- `config.rs`: loads dotenv and validates typed environment settings; `application.rs`: health route, server lifecycle and signal handling; `telemetry.rs`: JSON tracing configuration.
- `tests/`: real socket lifecycle integration and Unix subprocess tests for dotenv/SIGINT/SIGTERM. Unit configuration tests live beside their implementation.
- `docs/`: equivalent English/Portuguese guidance and numbered ADRs. English is canonical.
- `docker/entrypoint.sh`: creates physical cache/build directories, then executes the development command as the non-root user.
- Cargo manifest/lock: declared dependencies and exact resolution. `Cargo.lock` is tracked for this binary.
- Dockerfile/Compose: development toolchain, mounted source, host loopback HTTP/PostgreSQL ports, database bind mount and UID/GID build arguments. `.dockerignore` excludes caches, secrets and local metadata from builds.
- `.env.example`: safe defaults; copy it to ignored `.env`. `.gitignore` also excludes `.cargo-cache/`, `target/`, editor and OS artifacts. These generated directories are not source and should never be committed.
- CI: format, Clippy and database-independent tests on pushes/pull requests, plus PostgreSQL 18.6 and RabbitMQ 4.3.6 integration jobs using disposable credentials. GitHub execution is separate from local validation.
- README files: entry points; LICENSE: MIT permission terms.

## Runtime dependencies

| Crate | Reason |
| --- | --- |
| Tokio | Async runtime, TCP listener, signals; sync/time also support bounded lifecycle tests |
| Axum | Small HTTP health route and graceful HTTP server shutdown |
| dotenvy | Local `.env` loading with process environment precedence |
| tracing | Structured lifecycle events |
| tracing-subscriber | JSON formatting and environment filter parsing |

Serde and serde_json provide envelope JSON serialization; UUID generates v7 event IDs; Chrono supplies UTC timestamps. Errors use the standard library; no direct thiserror dependency is present; SQLx supplies PostgreSQL and Lapin supplies RabbitMQ. `domain/event.rs` contains real envelope behavior, not an empty architecture layer. Compose supplies local PostgreSQL with physical data storage; SQLx migrations define the relay namespace and outbox table; Rust persistence contracts now exist separately; a read-only PostgreSQL adapter exists; application writes remain deferred. See [PostgreSQL](../database/postgresql.md) and [ADR 002](../adr/002-use-postgresql-for-durable-event-storage.md). No Makefile or separate development Compose override is necessary for the current commands.

The intended public repository name is `reliable-event-relay`; the existing local checkout folder can keep its current name. The Cargo package and project title use the intended name. The GitHub description is the first README sentence and Cargo description; no remote repository settings are changed by this scaffold.

## Persistence module responsibilities

- `persistence/mod.rs`: exports application-facing types and the read-only OutboxReader trait. Callers supply an explicit bounded UTC request; results confer no ownership.
- `persistence/model.rs`: BatchSize validation, EligibleRead and PendingOutboxEvent (canonical envelope plus attempt/availability metadata). It does not mirror every SQL column or add relay metadata to EventEnvelope.
- `persistence/error.rs`: four driver-independent error classifications, including CommitUncertain for future mutations, source preservation and sanitized Display/Debug.
- `tests/persistence_contract.rs`: Rust-level contract tests and a scripted single-response fake; no database, environment reads or alternate production repository.

EventEnvelope describes the canonical event; the outbox migration defines durable SQL representation; these persistence contracts define what future application callers expect; the PostgreSQL adapter and signed-width/row/error mappings belong to Milestone 1.6. No empty adapter placeholder is added. Runtime configuration/HTTP lifecycle remain independent. See [design details](../architecture/persistence-contracts.md) and [ADR 005](../adr/005-separate-persistence-contracts-from-postgresql.md).

## PostgreSQL repository — Milestone 1.6 implemented

`src/infrastructure/postgres/` implements the existing `OutboxReader` using an injected `PgPool`. Infrastructure depends inward on persistence and domain; SQL, SQLx, private rows and driver mapping stay in infrastructure. Models own validation/conversion and contain no queries. No duplicate interface, empty layers, new migration or ADR is needed under ADR 005. Native Send futures/static dispatch remain intact. HTTP startup remains independent of PostgreSQL.

See [query, restoration, bounds and errors](../database/postgres-repository.md), [integration test guidance](../testing/strategy.md) and [executed validation](../testing/validation-results.md). SQLx is now an application dependency as well as separate migration tooling. Writes, claims and processing remain deferred; Milestones 1.7 and 1.8 are complete: integration evidence and [failure/transaction semantics](../architecture/failure-and-transactions.md). Milestone 1 is closed; [Milestone 2.1](../architecture/milestone-2-1.md) adds the publisher while worker delivery remains deferred.

## Milestone 1 handoff

Milestone 1 is closed; [Milestone 2.1](../architecture/milestone-2-1.md) now implements the publisher slice. Read the [review and acceptance evidence](../operations/milestone-1-review.md), [failure/transaction semantics](../architecture/failure-and-transactions.md), [Portuguese handoff source](../../pt-BR/operations/handoff-marco-1.md). The historical closure reviewed base was 7cba38d; recorded Milestone 2.2 baseline was 5eac9ff.

Controllers should delegate to use cases; use cases orchestrate application/business work; repositories own queries and focused contracts; external API/messaging goes in adapters; services hold reusable business behavior; helpers hold generic utilities; models hold data, invariants and model conversions. Infrastructure depends inward. These boundaries are applied proportionally: the fixed HTTP health response needs no unused use-case/service layers. No forwarding classes or generic CRUD layer were added.

To regenerate the PDF, install ReportLab in a separate Python environment and run `python scripts/generate_handoff.py`. Fonts are selected from system Arial or DejaVu (or `HANDOFF_FONT_DIR`); generation reads the Markdown source and writes the root PDF. Render and inspect every page and verify text extraction before accepting a regenerated artifact. Rust dependencies are unaffected.


## Milestone 3 roadmap

Milestone 3.1 selects the next schema and roadmap; all 3.2 migration/cycle/revision/scheduling changes remain planned. Applied migrations are unchanged. [Policy and 3.2–3.6 acceptance](../architecture/milestone-3-1.md).


## Current Milestone 3.6 acceptance — 2026-10-13

[Full-project audit, traceable acceptance matrix, actual validation counts, operational gaps and study path](../operations/milestone-3-review.md). Milestone 3 closed with explicit nonblocking limitations; shared replay rollout, consumer idempotency, live frontend integration and production readiness remain separate. Earlier sections retain their dated historical scope.


## Milestone 4.1 — contracts implemented; runtime planned

[Inventory, deduplication, quarantine and 4.2–4.4](../architecture/milestone-4-1.md) | [ADR 010](../adr/010-atomic-consumer-and-quarantine.md).

[Milestone 4.3 — real consumer flow, ACK/quarantine matrix, topology, shutdown and pending rollout](../architecture/milestone-4-3.md). Atomic effects and runtime are implemented; full crash/redelivery acceptance remains 4.4.
