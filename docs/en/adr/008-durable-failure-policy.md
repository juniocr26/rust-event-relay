[Other language](../../pt-BR/adr/008-durable-failure-policy.md)

# ADR 008 — Durable failure policy and recovery contracts

Status: accepted; 3.2 persistence and 3.3 application integration implemented; shared rollout planned. Date: 2026-10-12.

## Context

Milestone 2 preserves leases, acquisition counters and ambiguous effects. Volatile worker pacing cannot implement durable scheduling. Categories do not determine permanence or safe retries. Invalid stored rows cannot always produce OwnedOutboxEvent.

## Decision

Adopt the [complete policy and matrix](../architecture/milestone-3-1.md): ten committed acquisitions per cycle; preserve lifetime attempt_count; baseline zero for existing rows; explicit new replay cycle. Use persisted PostgreSQL post-lock time, checked microsecond arithmetic and capped equal jitter (1 s base, 300 s cap). Schedule only eligible definitive contextual failures with live ownership. Infrastructure recovery does not imply permanent event invalidity. Uncertain effects retain leases and require observation/expiry; no generic reconciliation exists today.

Preserve terminal events in existing dead_letter state with sanitized diagnostic/evidence/audit metadata. Add a raw-row maintenance authority path for invalid/exhausted events, protecting live owners. Replay requires explicit intent, expected revision, durable request audit and original identity; it does not promise a repeated consumer effect.

## Alternatives and consequences

Publication budgets need an additional durable protocol and still cannot atomically measure broker acceptance. Acquisition budgets are smaller and auditable but charge pre-send crashes/outages. Separate quarantine tables improve access separation but add movement and lifecycle consistency costs; retain rows and add audit evidence instead. Immediate release on uncertain send accelerates possible duplicates; lease retention delays recovery without eliminating duplicates. Ten acquisitions is a conservative finite operational default, not evidence that ten failures prove poison; preserve terminal reason and allow audited replay.

3.1 implements only pure budget/backoff calculations and tests. No migrations or pipeline wiring. 3.2–3.6 acceptance and deployment prerequisites are specified in the policy. ADR 007 guarantees remain unchanged.

Implementation update (2026-10-12): 3.2 persistence transitions and isolated tests are implemented; pipeline integration and shared rollout remain pending. [Milestone 3.2](../architecture/milestone-3-2.md).

Implementation update (2026-10-13): [3.3 application integration](../architecture/milestone-3-3.md) is implemented. Uncontextualized serialization conservatively retains its lease; explicit permanent attestation authorizes preservation. Infrastructure Unavailable retains ADR before-send release only while allowance remains; final definitive failure preserves exhaustion. Shared rollout remains pending.
