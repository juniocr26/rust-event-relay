[Português brasileiro](../pt-BR/milestone-1-review.md) | [README](../../README.md)

# Milestone 1 closure review

Date: 2026-10-08. Reviewed base: `7cba38d` (Milestone 1.7). The initial checkout was clean, with no applicable AGENTS.md. Closing changes are uncommitted; no new commit hash, push or remote CI execution is asserted. Milestone 1 is **closed within criteria 1.1-1.8**. There are no remaining acceptance blockers. Milestone 2 has not begun.

## Findings and disposition

Severity: medium = materially misleading current semantics/status; low = coverage or navigation improvement. None is a verified production defect.

| Finding | Severity / type | Evidence in reviewed base | Closing action |
| --- | --- | --- | --- |
| Roadmap says persistence integration remains planned despite completed 1.7; several pages still describe 1.8 as next | Medium, documentation inconsistency | architecture.md roadmap/progress; README and repository/testing footers, both languages | Reconcile implemented/planned claims, close 1.8 as documentation, retain future crash testing and historical validation records |
| “No locks” incorrectly suggests lock-free reads | Medium, documentation semantics | postgres-repository.md read section; SQL is a plain SELECT; PostgreSQL 18 ACCESS SHARE documentation | Distinguish absence of row ownership locks/claims from normal table locks; clarify effective isolation and statement snapshot |
| Applied migration history incorrectly described as absent from data directory | Medium, documentation semantics | docker-and-configuration.md physical-state section versus database-migrations.md and actual public._sqlx_migrations catalog | Distinguish source-controlled SQL from database-resident applied metadata; make destructive down limits explicit |
| Complex JSON test compares nested content to another driver read without a full explicit expected-value assertion | Low, test coverage improvement | restores_every_envelope_field_and_pending_metadata compares stored payload; numeric leaves have explicit assertions | Compare entire nested fixture with explicit expected JSON, normalizing exponent spelling; no decoder change |
| Existing 25-case schema script not included in isolated closing suite | Low, validation improvement | tests/sql/outbox_schema.sql has BEGIN/ROLLBACK; Rust harness already provides isolated databases | Add one ignored case executing the unchanged SQL fixture and asserting zero remaining rows |

No production code, Cargo dependencies/lockfile, migrations, shared credentials or application data were changed. No speculative architecture layers or writer/claim methods were introduced. No new material architecture decision requires an ADR; ADRs 001-005 remain applicable historical decisions.

## Acceptance evidence

| Criterion | Reviewed result / evidence |
| --- | --- |
| 1.1 Envelope | Private fields, typed validation, UUID v7 creation; restore/Serde preserve historical identity/time; whitespace preserved when nonempty; six envelope tests |
| 1.2 PostgreSQL | Existing Docker PostgreSQL 18.6 healthy, loopback host port 5433, internal postgres:5432, persistent bind mount; read-only authentication/configuration diagnostic passed |
| 1.3 Migrations | Two immutable up/down pairs; SQLx 0.8.6 wrapper percent-encodes credentials, pins internal topology; both migrations installed; no pending application needed |
| 1.4 Schema | 15 columns, 11 NOT NULL fields, primary key, seven checks and pending partial index; 25 committed schema assertions executed in isolation with rollback |
| 1.5 Contract | Explicit UTC cutoff, positive count bound, pending snapshots, three source-preserving errors; native Send futures/static dispatch; domain/contracts contain no SQLx; five contract tests |
| 1.6 Repository | One bounded parameterized SELECT; private checked timestamps/JSONB/numeric decoding; checked LIMIT before I/O; no writes/claims or automatic migration; seven adapter unit tests |
| 1.7 Integration | 15 current opt-in cases passed serially and with four threads; public closed-pool/oversized-limit test is database-independent; generated databases absent after runs |
| 1.8 Semantics | Coordinated producer/read/failure/durability/recovery documentation, evidence matrix and official PostgreSQL 18/SQLx 0.8.6 sources; explicit future decisions, Portuguese standalone handoff |

Representation differences are deliberate limits, not newly discovered defects: PostgreSQL INTEGER attempts stop at i32::MAX while the model uses u32; future writes must check storage width. BIGINT plus CHECK preserves all positive u32 versions. PostgreSQL accepts whitespace-only fields and a wider timestamp range/infinity than the domain/Chrono; selected unsupported rows correctly fail. JSONB normalizes representation; arbitrary precision preserves stored numeric values. UTC restoration preserves instants at database microseconds, not original offset/nanosecond spelling. UUID identity does not imply business order.

## Responsibility and operations review

Models own invariants/conversions; infrastructure owns SQL/driver mapping and depends inward. The HTTP health route has no business orchestration to delegate, so adding forwarding controller/use-case/service layers would be premature. Future controllers delegate to use cases; use cases orchestrate, reusable business behavior goes in services, generic utilities in helpers and external messaging in adapters. None was added merely to satisfy a diagram.

Pools are injected and caller-managed. The test harness has controlled quoted database names, ordered committed migrations, fixed fixtures, bounded acquisition/statement/lock/case/cleanup waits, barrier synchronization, redacted errors and panic/error cleanup. The two CI jobs preserve database-independent checks and use the same tested parallel target with PostgreSQL 18.6/disposable credentials. Remote GitHub execution is unvalidated. Cargo --locked preserves resolution; pinned tool versions are not immutable base-image/OS digests.

Current production deadlines, worker admission, backup/restore, abrupt crash recovery, cancellation timing, exhausted-pool injection and all concurrent-write schedules remain unvalidated or future work. The HTTP bootstrap remains independent of database readiness and has no forced shutdown deadline. These do not block the explicitly bounded Milestone 1 scope and do not establish a delivery guarantee. See [semantics and open decisions](failure-and-transaction-semantics.md), [executed validation](validation-results.md) and [Portuguese handoff](../pt-BR/handoff-marco-1.md).
