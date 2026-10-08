[Português brasileiro](../pt-BR/project-guide.md) | [README](../../README.md)

# Project guide

```text
.
├── .github/workflows/ci.yaml
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── config.rs
│   ├── application.rs
│   ├── telemetry.rs
│   ├── domain/event.rs
│   ├── infrastructure/postgres/mod.rs
│   └── persistence/ (mod.rs, model.rs, error.rs)
├── tests/ (lifecycle.rs, event_envelope.rs, persistence_contract.rs, postgres_repository.rs, support/, sql/)
├── docs/
│   ├── en/ (architecture, dependencies, Docker, guide, testing, validation, adr/)
│   └── pt-BR/ (equivalent documents)
├── docker/entrypoint.sh
├── .cargo-cache/ (generated, ignored)
├── target/ (generated, ignored)
├── .dockerized-postgres/ (generated, ignored)
├── Cargo.toml
├── Cargo.lock
├── .env.example
├── .gitignore
├── .dockerignore
├── Dockerfile
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
- CI: format, Clippy and database-independent tests on pushes/pull requests, plus a dedicated PostgreSQL 18.6 integration job using disposable credentials. GitHub execution is separate from local validation.
- README files: entry points; LICENSE: MIT permission terms.

## Runtime dependencies

| Crate | Reason |
| --- | --- |
| Tokio | Async runtime, TCP listener, signals; sync/time also support bounded lifecycle tests |
| Axum | Small HTTP health route and graceful HTTP server shutdown |
| dotenvy | Local `.env` loading with process environment precedence |
| tracing | Structured lifecycle events |
| tracing-subscriber | JSON formatting and environment filter parsing |

Serde and serde_json provide envelope JSON serialization; UUID generates v7 event IDs; Chrono supplies UTC timestamps. Errors use the standard library; no direct thiserror or broker dependency is present; SQLx supplies the PostgreSQL driver. `domain/event.rs` contains real envelope behavior, not an empty architecture layer. Compose supplies local PostgreSQL with physical data storage; SQLx migrations define the relay namespace and outbox table; Rust persistence contracts now exist separately; a read-only PostgreSQL adapter exists; application writes remain deferred. See [PostgreSQL](postgresql.md) and [ADR 002](adr/002-use-postgresql-for-durable-event-storage.md). No Makefile or separate development Compose override is necessary for the current commands.

The intended public repository name is `reliable-event-relay`; the existing local checkout folder can keep its current name. The Cargo package and project title use the intended name. The GitHub description is the first README sentence and Cargo description; no remote repository settings are changed by this scaffold.

## Persistence module responsibilities

- `persistence/mod.rs`: exports application-facing types and the read-only OutboxReader trait. Callers supply an explicit bounded UTC request; results confer no ownership.
- `persistence/model.rs`: BatchSize validation, EligibleRead and PendingOutboxEvent (canonical envelope plus attempt/availability metadata). It does not mirror every SQL column or add relay metadata to EventEnvelope.
- `persistence/error.rs`: three driver-independent error classifications, source preservation and sanitized Display/Debug.
- `tests/persistence_contract.rs`: five Rust-level tests and a scripted single-response fake; no database, environment reads or alternate production repository.

EventEnvelope describes the canonical event; the outbox migration defines durable SQL representation; these persistence contracts define what future application callers expect; the PostgreSQL adapter and signed-width/row/error mappings belong to Milestone 1.6. No empty adapter placeholder is added. Runtime configuration/HTTP lifecycle remain independent. See [design details](persistence-abstraction.md) and [ADR 005](adr/005-separate-persistence-contracts-from-postgresql.md).

## PostgreSQL repository — Milestone 1.6 implemented

`src/infrastructure/postgres/` implements the existing `OutboxReader` using an injected `PgPool`. Infrastructure depends inward on persistence and domain; SQL, SQLx, private rows and driver mapping stay in infrastructure. Models own validation/conversion and contain no queries. No duplicate interface, empty layers, new migration or ADR is needed under ADR 005. Native Send futures/static dispatch remain intact. HTTP startup remains independent of PostgreSQL.

See [query, restoration, bounds and errors](postgres-repository.md), [integration test guidance](testing.md) and [executed validation](validation-results.md). SQLx is now an application dependency as well as separate migration tooling. Writes, claims and processing remain deferred; Milestones 1.7 and 1.8 are complete: integration evidence and [failure/transaction semantics](failure-and-transaction-semantics.md). Milestone 1 is closed; later delivery work has not begun.

## Milestone 1 handoff

Milestone 1 is closed; Milestone 2 has not begun. Read the [review and acceptance evidence](milestone-1-review.md), [failure/transaction semantics](failure-and-transaction-semantics.md), [Portuguese handoff source](../pt-BR/handoff-marco-1.md) and [standalone PDF](../../HANDOFF_MARCO_1.pdf). The reviewed base is 7cba38d; closing changes are uncommitted.

Controllers should delegate to use cases; use cases orchestrate application/business work; repositories own queries and focused contracts; external API/messaging goes in adapters; services hold reusable business behavior; helpers hold generic utilities; models hold data, invariants and model conversions. Infrastructure depends inward. These boundaries are applied proportionally: the fixed HTTP health response needs no unused use-case/service layers. No forwarding classes or generic CRUD layer were added.

To regenerate the PDF, install ReportLab in a separate Python environment and run `python scripts/generate_handoff.py`. Fonts are selected from system Arial or DejaVu (or `HANDOFF_FONT_DIR`); generation reads the Markdown source and writes the root PDF. Render and inspect every page and verify text extraction before accepting a regenerated artifact. Rust dependencies are unaffected.
