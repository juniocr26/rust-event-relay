# Current operation and evidence boundaries

The supported launcher and one main Compose environment run separate HTTP/relay/consumer roles plus Vue. Use [current installation and operation](application-scenarios.md), [token recovery](demo-token-connectivity.md) and [the final event study](../guides/implementation-study.md). Source, on-disk binary and loaded process are separate evidence; rebuilding alone does not prove a running revision. Named Supervisor controls preserve deliberate stops under the same manager. Keep test artifacts separate from runtime artifacts.

The [2026-10-17 final acceptance](final-audit.md) records local portfolio closure at its own base; later startup/token/test-tool source changes were inspected statically here. Historical results do not establish the current live environment, remote CI or production readiness. Database backup does not include broker queues; automatic retention/general repair/HA/TLS are outside this bounded delivery. No service, migration, network or application test was executed in this documentation review. Earlier rollout status below remains dated history.

## Historical reference (original dates and milestone scope)

[Historical record: current installation and addresses](../operations/application-scenarios.md)

**Current deployment/evidence — 2026-10-17:** [5.3](milestone-5-3.md) records exact remote 5.2 CI, controlled named rollout, loaded hashes, finite isolated measurements and preserved demonstration. 5.3 itself is local test/docs/comment work and was not deployed. Older records below retain their original scope.

## Milestone 4.4 — current verification and closure — 2026-10-16

Milestone 4 is closed with explicit nonblocking limitations. Real inventory/deduplication, ACK/quarantine and recovery evidence is separate from pending shared rollout. Presentation frontend verified; live backend integration is not implemented. Earlier dated sections retain their historical scope. [Verification matrix](../operations/milestone-4-4-review.md) | [Environment inventory](../docker/environment-inventory.md).

#
Current worker behavior, configuration, cancellation and authorized rollout: [Milestone 2.5](../architecture/milestone-2-5.md). Earlier sections retain their historical scope.

 Deployment, observability and evidence gaps

[English](deployment-and-evidence.md) | [Português brasileiro](../../pt-BR/operations/deployment-and-evidence.md)

Static source review: 2026-10-10. Implemented facts, general theory and hypothetical changes are distinguished below. Runtime commands were not executed.

## Current update — 2026-10-12

[Milestone 2.3](../architecture/milestone-2-3.md) implements conditional ownership transitions with real contention/expiration evidence. Ownership migration remains pending in development, verified by read-only metadata; only isolated test databases were migrated. No broker fencing, end-to-end orchestration, production performance or lost-COMMIT-acknowledgement experiment is established. The source review below is historical (2026-10-10).

## Historical review


Compose supplies local PostgreSQL, RabbitMQ and a development app managed by Supervisor. The HTTP process waits for config/tracing/listener setup; it does not create a PgPool or RabbitMqPublisher. Container dependency health can gate startup but `/health` remains process liveness. Supervisor allows 30 s for HTTP drain and Compose grants 35 s for manager shutdown. Source changes require stop/build/start; auto-reload and a deployed worker are absent. Bind/named storage survives ordinary recreation, not host/storage loss.

SQLx migrations are versioned and explicitly applied, not applied automatically by HTTP startup. Preserve migration IDs and existing applied content. The ownership migration is source plus isolated test evidence; the previous milestone record says it remained pending in the shared development database. This review did not inspect that database or update its status. A down migration removes ownership columns; earlier downs can destroy the outbox. Neither down nor reapplying schema restores rows. A real rollout needs compatibility review and separately verified backup/restore, not a destructive reset.

Default tests validate domain, mapping and contracts plus HTTP/socket behavior. Opt-in PostgreSQL tests create exact generated fixture databases and verify cleanup; broker tests exercise routing and uncertainty. Some database-independent tests use local sockets, so independent does not mean no I/O. CI workflow presence is configuration evidence, not proof of remote success. Historical results remain in testing/validation-results. No Rust build, test, Docker call, migration, broker request or benchmark ran in this review.

Performance is currently constrained by bounded row counts and one-in-flight publisher admission, but row count does not bound payload bytes. Reader `fetch_all` materializes a batch; invalid selected data fails the whole result and can block future repeated reads. No quarantine, backpressure policy, polling rate, throughput measurement, metrics endpoint or production pool sizing is delivered. Remaining evidence gaps: ownership SQL/concurrency, worker orchestration, duplicate/recovery experiments, poison-row policy, production TLS/HA, retention/backup restore and performance. These are not prerequisites already demonstrated by a green liveness probe.


## Milestone 2.6 — Current scope

[Integrated experiments](../architecture/milestone-2-6.md) and [Milestone 2 review](../operations/milestone-2-review.md) update acceptance under explicit assumptions. Shared rollout remains pending; earlier records retain their historical context.

## Authorized local rollout — 2026-10-13

[Current local operation](local-relay-rollout.md): both http/relay autostart, ownership/retry migrations installed explicitly, authenticated broker topology and manual/interactive controls verified. Earlier pending/stopped descriptions retain historical scope. Production readiness and remaining 3.4–3.6 work are not claimed.
