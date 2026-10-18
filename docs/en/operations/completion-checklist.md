# Documentation maintenance — 2026-10-18

Baseline `6e5e64fbffea1eb3ec50b828648f3fa9e708cf6c`; initially clean worktree. This is static documentation maintenance. Historical test counts below belong to their original date/revision and were not rerun.

- [x] Inventory/source review
- [x] Structure and implementation content
- [x] Equivalent English/Portuguese coverage
- [x] Navigation and local links
- [x] Project interview and Engineering Library coverage
- [x] Evidence gaps and justified exceptions

[Current source review and evidence limits](../guides/implementation-study.md).

Completion concerns documentation coverage, not production readiness, runtime certification or closure of evidence gaps. Validator results are recorded in the library review.

## Historical checklists (original dates and scope)

[Historical record: current application guide and acceptance](../operations/milestone-6-3.md)

**Current 5.3 acceptance — 2026-10-17:** [Full matrix and Milestone 5 closure](milestone-5-3.md). Remote 5.2 selected and deliberately deployed; finite isolated measurements/final checks passed. Project production readiness not established; Milestone 6 undefined. Entries below retain their dated scope.

**Current local state — Milestone 4.5 (2026-10-16):** [Verified rollout and recovery](milestone-4-5-rollout.md). Six migrations applied; current HTTP/relay/consumer artifacts installed and running; consumer autostart=true. Protected backup/isolated restore and real 10 → 15 → 15 demonstration verified. Replay API remains disabled; frontend remains simulation. Earlier observations below retain their dates.

Current operational contract (2026-10-17): [5.2 readiness/freshness and validated 5.1 rollout](milestone-5-2.md). Earlier checklist entries below retain historical scope.

## Milestone 4.4 — current verification and closure — 2026-10-16

Milestone 4 is closed with explicit nonblocking limitations. Real inventory/deduplication, ACK/quarantine and recovery evidence is separate from pending shared rollout. Presentation frontend verified; live backend integration is not implemented. Earlier dated sections retain their historical scope. [Verification matrix](../operations/milestone-4-4-review.md) | [Environment inventory](../docker/environment-inventory.md).

#
Current worker behavior, configuration, cancellation and authorized rollout: [Milestone 2.5](../architecture/milestone-2-5.md). Earlier sections retain their historical scope.

 rust-event-relay: Repository completion checklist

## Milestone 2.4 — 2026-10-12

The callable delivery use case now performs one acknowledged acquisition → original envelope publication → completion only after confirmed acceptance. Definitive publication failures release once; uncertainty retains the lease. Results preserve primary and cleanup failures separately. HTTP remains independent; no worker is composed. See [Milestone 2.4](../architecture/milestone-2-4.md) for outcomes, cancellation and recovery limitations. Next: 2.5 worker composition/operation; 2.6 end-to-end failure experiments.

Earlier sections retain their recorded milestone/review scope.

Static review on 2026-10-10; no runtime execution. Checked items describe completed documentation work, not completed application features or closed evidence gaps.

- [x] Inventory: existing documentation and source/config/tests inspected; original inventory retained in library manifest.
- [x] Restructuring: matching language categories; existing ADR IDs/history retained; required source/tool files kept in place.
- [x] Content review: implementation mechanisms, contracts, alternatives, failure and evidence boundaries explained.
- [x] Bilingual coverage: equivalent maintained pages in en and pt-BR; historical records explicitly identified.
- [x] Navigation: README and language catalog link every maintained document.
- [x] Corresponding Engineering Library coverage: complete explanations and retained/expanded substantive answers.
- [x] Link/anchor/numbering validation: final workspace check has zero errors; the 18 restricted fixture-link warnings are recorded explicitly in the library report.

## Evidence inspected

- [src/application.rs](../../../backend/src/application.rs)
- [src/config.rs](../../../backend/src/config.rs)
- [src/telemetry.rs](../../../backend/src/telemetry.rs)
- [src/persistence/ownership.rs](../../../backend/src/persistence/ownership.rs)
- [src/infrastructure/postgres/mod.rs](../../../backend/src/infrastructure/postgres/mod.rs)
- [src/infrastructure/rabbitmq.rs](../../../backend/src/infrastructure/rabbitmq.rs)
- [migrations/20261009000000_add_delivery_ownership.up.sql](../../../backend/migrations/20261009000000_add_delivery_ownership.up.sql)
- [compose.yaml](../../../compose.yaml)
- [tests/delivery_ownership.rs](../../../backend/tests/delivery_ownership.rs)

## Interview coverage and library counterpart

HTTP liveness; envelopes/UUID/JSON/timestamp restoration; outbox transactions/snapshots; leases/authority/expiry; broker confirms/uncertainty; static dispatch, isolated tests and recovery limits.

Self-contained dossier / Dossiê (`engineering-library/docs/en/architecture/rust-event-relay.md`, external repository) | Interview / Entrevista (`engineering-library/docs/en/interviews/rust-event-relay.md`, external repository)

## Category applicability

| Category | Disposition / justification |
| --- | --- |
| payments | No payments. |
| api | Only fixed GET /health, documented in README/architecture. |
| deployment | Supervisor/migrations/update/rollback caveats in operations/Docker. |
| observability | JSON tracing/liveness, no readiness or metrics, in operations/security. |
| benchmarks | Not applicable: no existing verified performance measurements suitable for a chart; none were run. |

Categories present in the index contain maintained content; categories covered elsewhere above do not get empty folders. ADR/comparison material retains its existing history; no new historical motivation or date is invented.

## Remaining evidence-dependent gaps

Production ownership SQL/concurrency, worker orchestration, producer/consumer effects, poison rows, retry/recovery experiments, TLS/HA and performance.


## Milestone 2.6 — Current scope

[Integrated experiments](../architecture/milestone-2-6.md) and [Milestone 2 review](../operations/milestone-2-review.md) update acceptance under explicit assumptions. Shared rollout remains pending; earlier records retain their historical context.


## Milestone 3.1 policy

Milestone 3.1 defines the failure matrix, durable retry/budget/isolation/replay contracts and remaining roadmap. Only pure policy calculations are implemented; future persistence and frontend scenarios remain planned. [Policy](../architecture/milestone-3-1.md) | [ADR 008](../adr/008-durable-failure-policy.md).

## Milestone 3.3 — current integration

Application delivery/worker integration is implemented; earlier sections retain their dated historical scope. [Current contracts, jitter, contextual outcome table, maintenance and cancellation limits](../architecture/milestone-3-3.md). General poison scanning remains 3.4, audited replay 3.5 and full acceptance 3.6; shared rollout remains pending.

## Authorized local rollout — 2026-10-13

[Current local operation](local-relay-rollout.md): both http/relay autostart, ownership/retry migrations installed explicitly, authenticated broker topology and manual/interactive controls verified. Earlier pending/stopped descriptions retain historical scope. Production readiness and remaining 3.4–3.6 work are not claimed.


## Milestone 3.4 — current discovery/isolation

Bounded discovery and fresh-authority isolation are implemented; replay remains 3.5 and integrated closure 3.6. Current local rollout is preserved; this change has no migration or shared restart. [Milestone 3.4 — current discovery/isolation](../architecture/milestone-3-4.md).


## Current Milestone 3.6 acceptance — 2026-10-13

[Full-project audit, traceable acceptance matrix, actual validation counts, operational gaps and study path](milestone-3-review.md). Milestone 3 closed with explicit nonblocking limitations; shared replay rollout, consumer idempotency, live frontend integration and production readiness remain separate. Earlier sections retain their dated historical scope.


## Milestone 4.1 — contracts implemented; runtime planned

[Inventory, deduplication, quarantine and 4.2–4.4](../architecture/milestone-4-1.md) | [ADR 010](../adr/010-atomic-consumer-and-quarantine.md).
