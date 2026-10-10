# Milestone 1 handoff

Reliable Event Relay | Durable event model

**Historical generation date:** 2026-10-08 (America/Sao_Paulo). **Reviewed base:** `7cba38d`, Milestone 1.7 commit. Closing documentation, strengthened tests and the PDF were uncommitted changes in that delivery. No future hash or remote CI execution is attributed to it. This English counterpart translates the historical Portuguese handoff; its milestone status and executions describe that delivery, not the present checkout or this documentation review.

[Portuguese historical source](../../pt-BR/operations/handoff-marco-1.md). The Portuguese source remains the input to the PDF generator.

## Delivery status

**Milestone 1 closed against criteria 1.1–1.8, without remaining blockers. Milestone 2 had not started.** Closing validation was repeated then; earlier 1.7 results were retained separately.

This open portfolio/study project investigates distributed event delivery under failure. Rust is the tool; preserving meaning, identity and recovery boundaries is the problem. At this historical point, the implementation supplied a validated envelope, outbox schema, observation contract and PostgreSQL adapter. The binary started HTTP/health without connecting to the database or delivering events.

| Delivery | Closing result |
| --- | --- |
| 1.1 Canonical envelope | Identity, time, metadata and payload validated/restored |
| 1.2 Local PostgreSQL | Docker, healthcheck, loopback access and bind mount |
| 1.3 Migrations | Versioned SQL, explicit CLI and applied history |
| 1.4 Outbox schema | Constraints, representable states and pending index |
| 1.5 Abstraction | OutboxReader, bounded reads, snapshots and errors |
| 1.6 Repository | Parameterized SELECT and checked private decoding |
| 1.7 Integration | Real PostgreSQL, isolation, deadlines and cleanup |
| 1.8 Semantics | Transactions, failures, durability and recovery limits |

The closure did not implement producers, claims, leases, transitions, retries or destinations. Local persistence was not a working delivery guarantee. Future decisions remained open without blocking the documentation scope of 1.8.

## Architecture and responsibilities

The historical HTTP path was main → configuration/tracing → listener → application.rs lifecycle. Health returned ok; tests exercised real sockets and Unix signals. Persistence existed as a library separate from HTTP bootstrap. PostgreSQL infrastructure implemented OutboxReader and depended on persistence/domain; domain/contracts did not depend on SQLx. The caller constructed, injected and closed PgPool; no repository-owned global pool or environment lookup existed.

| Responsibility | Applied boundary |
| --- | --- |
| Controllers | Delegate to use cases when orchestration exists |
| Use cases | Application/business control; processing still future |
| Repositories | Queries and focused contracts |
| External adapters | API/messaging communication; not yet implemented then |
| Services/helpers | Reusable business logic/generic utilities |
| Models | Data, invariants and their conversions |
| Infrastructure | Drivers, SQL, decoding, inward dependencies |

Fixed health did not justify empty layers or forwarding classes. The closing review added no generic CRUD, write contracts or behaviorless abstractions.

## Decisions and costs

ADR 001 selected Rust's explicit ownership/concurrency with learning/build costs; it did not remove delivery logic failures. ADR 002 selected PostgreSQL for a local transactional boundary/JSONB with stateful operation, retention and contention costs, without comparative benchmarks. ADR 003 selected versioned SQL and explicit CLI for reviewable history; rollback was not recovery and tags did not make the OS/database immutable. ADR 004 selected durable outbox intent when the producer writes business change/event together; remote publication remained separate and duplicate-prone. ADR 005 selected a focused read contract without nonexistent transition authority; static dispatch/Send futures avoided mandatory boxing and the trait was not dyn-compatible.

Recorded alternatives included Go/Java; MySQL/SQLite or broker storage; manual/ORM schema changes; publish before/after commit, 2PC or CDC; direct SQLx, generic CRUD or a full mutation contract. Choices reflected scope and explicit boundaries, not measured superiority. No material new closing decision needed an ADR. ADRs 001–005 preserved historical context.

## Envelope and storage compatibility

EventEnvelope had nine private fields: id, event_type, aggregate_type, aggregate_id, schema_version, occurred_at, correlation_id, causation_id and payload. `new` generated UUID v7 and current UTC time; restore/Serde preserved historical values without generating identity. UUID v7 did not guarantee global business order. Blank/whitespace-only names and aggregates failed validation while meaningful surrounding spaces were retained. Version zero was invalid. Optional correlation/causation UUIDs were not arbitrary external-token support. Flexible JSON did not validate business payload schemas.

| Representation | Limit/conversion |
| --- | --- |
| UUID/TEXT | Identity preserved; aggregate format belongs to producer |
| NonZeroU32/BIGINT | CHECK range 1–4294967295; checked signed conversion |
| u32 attempts/INTEGER | Database ends at i32::MAX; future writes must check width |
| UTC DateTime/TIMESTAMPTZ | Microsecond instant; original offset/nanoseconds not retained |
| Value/JSONB | Semantic content; formatting/key order normalized |
| JSON numbers | arbitrary_precision retains large integers/long decimals/stored 1e1000 |

The historical schema had 15 columns, 11 NOT NULL fields, primary key and seven checks. States were pending, processed and dead_letter. Processed required processed_at; other states required null. Attempts were nonnegative. The partial pending index covered available_at, created_at and id. These constraints did not validate transition history or authority. PostgreSQL accepted whitespace-only fields, infinity and values beyond Chrono; selected rows were rejected by decoder/domain. This representation difference did not justify weakening constraints.

Two up migrations created namespace/table in order. Downs used RESTRICT, but DROP TABLE destroyed data. Versioned migration SQL was source; `_sqlx_migrations` was database metadata physically persisted with the cluster.

## Transaction and read boundaries

The future producer needed business change/outbox in one local transaction. The independent relay did not implement that transaction or OutboxWriter. Separate commits could leave only one write; the remote broker did not share local atomicity. [PostgreSQL transaction theory](https://www.postgresql.org/docs/18/tutorial-transactions.html) supported the distinction.

A lost COMMIT response could leave outcome uncertain. This was engineering analysis, not injected commit failure. Duplicate key did not prove identical payload/metadata/effects. Producer reconciliation/idempotency remained future; generating a fresh event ID on every retry could duplicate logical intent.

OutboxReader accepted explicit UTC cutoff and positive BatchSize. One SELECT included pending available_at up to the cutoff, with checked i64 LIMIT; excessive representable limits failed before I/O. Availability/creation/ID ordering was an adapter detail, not delivery ordering. Read Committed snapshots began per statement; stronger levels could use transaction snapshots. The local session was observed as Read Committed, but the adapter forced no isolation or multi-statement transaction. [Isolation theory](https://www.postgresql.org/docs/18/transaction-iso.html) was separate from observations.

There was no FOR UPDATE/SHARE, claim, reservation or lifecycle mutation. Ordinary table locks such as ACCESS SHARE still applied. Two readers could see one row; metadata could immediately become stale. Repeated reads could see new data without writing. Empty batches did not prove permanently exhausted backlog. Count did not limit payload bytes/worker concurrency; production pool/query/polling/admission defaults did not exist. Harness deadlines were test settings. Tests established repeated stable fixture observation, complete before/after equality and two readers without authority, not every concurrent-write schedule.

## Current failures and future windows

The [full matrix](../architecture/failure-and-transactions.md) distinguished real integration, unit tests, source/published semantics and future analysis.

| Failure | Observation/state | Recovery owner/evidence |
| --- | --- | --- |
| Database down, closed/timed-out pool | Unavailable; reader does not write outbox | Caller/operator; unit classification and closed-pool contract |
| Missing table/SQL failure | OperationFailed with source; no repair | Schema/deploy operator; missing table tested in isolated PostgreSQL |
| Invalid selected row | Whole batch fails; row remains | Data owner; whitespace/timestamps tested |
| Cancelled future | No result/claim; immediate server termination unproved | Caller/pool |
| Process restart | In-memory snapshot disappears; no claim to recover | Future app/operator; abrupt crash not injected |
| Future publish before database confirmation | Destination can act while row remains pending; retry duplicates | Worker/idempotent consumer; analyzed only, no publisher then |
| Future processed before publish | Reader excludes row despite possible nondelivery | Worker design; analyzed loss window, no transition then |

Invalid eligible data could repeatedly block useful batches; no skip/repair/quarantine/automatic dead-letter existed. Invalid rows outside cutoff/status did not affect the selected batch, as tested. Fixing fixtures allowed a subsequent valid read. Error categories did not define retry or promise repeated success. Sanitized Display/Debug retained typed sources requiring redaction before logging.

Client cancellation did not prove immediate query termination; PostgreSQL cancellation could arrive after completion. SQLx Pool::close waited for borrowed connections, without forcibly interrupting all queries. Acquire timeout did not bound full query duration. These were published semantics, not measured app deadlines. Persistence/schema alone could not deliver events; separate publication/ack created loss/duplicate windows and consumer effects needed coordinated deduplication.

## Durability, operations and open decisions

Committed state also depended on PostgreSQL, storage and deployment. Async commit could acknowledge before durable WAL flush. fsync, synchronous_commit and full_page_writes were observed on locally, not power-loss/storage/replication/SLA tests. The PostgreSQL bind mount survived ordinary recreation but was not backup. Retention, restore exercises, capacity and production topology needed design. Migration down/reapplying schema did not recover removed rows. The historical closure did not reset shared databases, change credentials or restart PostgreSQL for failure simulation.

## Tests, isolation and configuration

Compose app used postgres:5432; host used 127.0.0.1:5433. Harness accepted explicit POSTGRES_HOST/PORT; CI used 127.0.0.1:5432. Opt-in required POSTGRES_USER/PASSWORD/DB and a role with CREATEDB/ownership of disposable databases. No credentials appear here.

Each case created `relay_it_<hexadecimal UUID>`, with controlled/quoted name, versioned ups and explicit fixtures. Deadlines were 10 s connection/acquisition, 5 s statement, 3 s lock, 45 s case and 10 s administration/cleanup. Readers used barriers rather than fixed sleeps. Pool closed before dropping exactly the created database; catalog absence was verified. Panic/returned-error cleanup had tests. Abrupt process death, unavailable cleanup server or deadline expiry could leave a fixture; manual removal required exact identity/ownership, never broad cleanup.

Open questions then: claims/leases/concurrent recovery, transition authority/expected state/repeated confirmations, retry/backoff/availability limits, dead-letter/quarantine/replay/retention, producer/consumer atomic idempotency, aggregate order versus parallelism. No broker, benchmark or speculative crash framework was delivered in that milestone.

## Validation executed in that delivery

Cargo commands used `docker compose exec -T app`, rustdoc flags via `-e`. Reviewed base was 7cba38d; these results belonged to closing changes rather than the earlier report. Versions were Rust 1.95.0, PostgreSQL 18.6 and SQLx CLI/driver 0.8.6.

| Check | Historical result |
| --- | --- |
| cargo fmt --check | Passed |
| cargo clippy --locked --all-targets --all-features -- -D warnings | Passed |
| cargo test --locked | 23 passed; 15 PostgreSQL cases ignored |
| cargo test --locked --test postgres_repository -- --ignored --test-threads=1 | 15 passed |
| Same integration target with --test-threads=4 | 15 passed; parallel isolation |
| cargo build --locked | Passed |
| RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps | Passed |
| git diff --check | Passed |
| scripts/check-postgres.sh | Health, host TCP, config/authenticated identity passed |
| sqlx migrate info | Two installed, none pending then |
| inspect_outbox_schema.sql | Read-only columns/constraints/index/history inspection |
| Isolated schema SQL fixture | 25 assertions, rollback, zero remaining rows |
| Temporary database catalog | Zero controlled relay_it names remained |

Default suite remained database-independent. Opt-in covered eligibility/bounds, restored fields, numeric JSON, observation without mutation, failures/sources and cleanup. Dedicated CI retained PostgreSQL 18.6/disposable credentials and the same target with four threads; remote execution was not evidenced. Unvalidated: native host Rust, remote GitHub CI, production deployment, real backup/restore, performance, abrupt crash, server cancellation timing, exhausted-pool integration and every concurrent schedule. No blocker remained for that scope. Tests did not prove delivery/capacity/production recovery. The historical PDF had selectable text, accents, pagination and reviewed page rendering; report dependencies were not added to Rust.

## Findings, corrections and resumption

No production defect or production-code/Cargo.lock/migration change was confirmed. Closing review corrected three medium documentation findings: stale roadmap/status, lock-free read wording, and applied-history placement versus SQL source. Two low findings strengthened explicit whole-JSON assertions with exponent normalization and added the existing 25-assertion SQL fixture to isolated opt-in integration.

The historical resumption instruction was to read review/semantics, inspect uncommitted diff/history and select the next task explicitly, preserving boundaries and never treating snapshots as concurrent-delivery permission. Ownership/transition authority needed definition first. Suggested commit was unexecuted: `docs: close milestone 1 with failure and transaction semantics`.

## Repository reference map

| Reference | Content |
| --- | --- |
| src/domain/event.rs; src/persistence/ | Envelope, contracts, snapshots/errors |
| src/infrastructure/postgres/mod.rs; migrations/ | Query, decoders, mapping, up/down SQL |
| tests/postgres_repository.rs; tests/support/; tests/sql/ | Real evidence, isolation/constraints |
| compose.yaml; Dockerfile; docker/sqlx.py; scripts/check-postgres.sh | Topology, tools/diagnostics at historical scope |
| .github/workflows/ci.yaml | Independent/dedicated integration checks |
| [Review](milestone-1-review.md), [results](../testing/validation-results.md), [semantics](../architecture/failure-and-transactions.md), ADRs 001–005 | Closing review, evidence, complete matrix and decisions |

Supporting published sources preserved from the handoff: PostgreSQL 18 [isolation](https://www.postgresql.org/docs/18/transaction-iso.html), [locks](https://www.postgresql.org/docs/18/explicit-locking.html), [protocol/cancellation](https://www.postgresql.org/docs/18/protocol-flow.html), [WAL/commit](https://www.postgresql.org/docs/18/wal-async-commit.html), [backup](https://www.postgresql.org/docs/18/backup.html); SQLx 0.8.6 [pool](https://docs.rs/sqlx/0.8.6/sqlx/struct.Pool.html), [options](https://docs.rs/sqlx/0.8.6/sqlx/pool/struct.PoolOptions.html). Sources, code/test evidence and future inference remain distinct. No web source was fetched in this review.
