[Português brasileiro](../../pt-BR/operations/milestone-2-review.md) | [README](../../../README.md)

# Milestone 2 acceptance review — 2026-10-12

**Closed for implemented scope, with bounded evidence. Shared rollout is pending separately.** The pipeline is an **at-least-once foundation under stated assumptions**. It is not a proof of universal delivery or exactly-once consumer effects. No material implemented-scope defect remains confirmed by this finite matrix.

## Requirement mapping

| Requirement | Implementation | Evidence and limit |
| --- | --- | --- |
| Durable ownership | [ADR 007](../adr/007-durable-delivery-ownership.md), PostgreSQL acquisition commit/token/count/lease | [Ownership regressions](../../../tests/postgres_ownership.rs), [2.6 A/B](../architecture/milestone-2-6.md); committed pending state survives exact process SIGKILL. No power-loss proof. |
| Confirm before complete | [deliver_once](../../../src/application/delivery.rs), mandatory routing + confirms in publisher | Boundary B pauses after contract success/before completion; uncertain send never completes; completion trigger is reached after confirmation. Broker acceptance is not consumer business success. |
| Stable identity | Original EventEnvelope, AMQP message_id | Baseline and every duplicate observation preserve inserted IDs; no producer endpoint/transactional source write implemented. |
| Incomplete recovery | Database-time eligibility after lease expiry; fresh AcquireRequest | A/B, SQL rejection/deadline, in-flight shutdown and uncertainty recover with count 2/new token; new experiments never edit expiry timestamps. Eventual availability/restart required. |
| Concurrent protection | Short transaction, SKIP LOCKED, post-lock time and token predicates | Existing competing acquisition tests plus two real binaries with two live distinct durable tokens and completion barrier. No broker-count-only exclusivity inference or fairness claim. |
| Stale authority | Pending + exact token + unexpired interval predicate | Expired A completion/release rejected after B acquires; B token remains. Former publisher still sends externally: SQL ownership does not fence RabbitMQ. |
| Uncertain external outcomes | Uncertain retains lease; adapter retired; worker replaces | Withheld confirms independently show accepted message while pending; disconnected owned proxy causes real worker replacement, without replaying retained claim. Other uncertain schedules could reject or accept. |
| Uncertain database outcomes | CommitUncertain conservatively classified, no compensating action | Real confirmed use case plus completion COMMIT response-loss proxy: independent processed count 1; restart NoEvent/no replay. This injection demonstrates one committed uncertain outcome only. |
| Operable lifecycle | Separate binary, schema gate, pacing/reconnection and bounded shutdown | Baseline/Supervisor smokes; real handshake outage/restoration, TERM idle, INT reconnect, TERM blocked completion; existing deterministic drain/budget tests. Cooperative schedules only. |
| Duplication windows | External accept is outside database transactions | B/C, ordinary completion rejection, SQL timeout and shutdown demonstrate repeated original IDs. Stale sender can also publish. Consumer duplicate handling is required separately. |

## Assumptions and exclusions

Source events must already be durably committed, valid and eligible; PostgreSQL and RabbitMQ must satisfy the supported transaction/confirmation/storage behavior. Durable queues/exchanges and persistent messages are configured, but service power-loss/storage corruption/replicated HA/TLS behavior was not established here. Database clock discipline and sufficient remaining lease time matter; acquisition may return an expired lease and no renewal is provided.

Infrastructure must eventually become available, a worker must run or restart, and event failures must eventually resolve before counter exhaustion. Permanent rejection, malformed/poison events and acquisition count exhaustion can prevent progress; no durable retry/backoff/jitter, quarantine, dead-letter transition or replay policy exists yet. There is no fairness, business ordering, throughput or universal liveness proof. Fixture writes do not prove business/outbox transactional atomicity. Test consumer ACKs do not prove consumer effects or idempotency. Appropriate downstream duplicate handling remains necessary.

Graceful drain/cleanup budgets do not cover power loss, SIGKILL, kernel stalls or every cancellation schedule. Loss before observed COMMIT completion, arbitrary uncertain outcomes and productive HA/TLS remain evidence gaps, not newly discovered defects. Bounded tests inspect exact inserted ID sets, durable states and independent broker observations; terminal failure/discard/dead-letter are never counted as successful delivery.

## Operational status and Milestone 3 decisions

Shared ownership migration `20261009000000` remains pending; http RUNNING, relay STOPPED, autostart=false. Isolated migrations/evidence do not authorize rollout; pending shared rollout does not invalidate isolated acceptance. No service recreation, shared queue purge or unrelated session termination was used. [Handoff](../../../Reports/MILESTONE_2_6_HANDOFF.md) records checks and continuity.

Before Milestone 3 decide: retryable versus permanent versus uncertain classification; durable scheduling/backoff/jitter and attempt semantics; poison-event isolation and quarantine/dead-letter representation; auditable replay preserving identity and authority; reconciliation after unknown acknowledgements; migration/operational acceptance and separate shared rollout authorization. Consumer business idempotency and producer endpoints need their own scope decisions. A graphical interface remains optional after core completion; none was added.
