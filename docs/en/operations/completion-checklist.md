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

[Self-contained dossier / Dossiê](../../../../engineering-library/docs/en/architecture/rust-event-relay.md) | [Interview / Entrevista](../../../../engineering-library/docs/en/interviews/rust-event-relay.md)

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
