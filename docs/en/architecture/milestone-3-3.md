[Other language](../../pt-BR/architecture/milestone-3-3.md) | [ADR 008](../adr/008-durable-failure-policy.md)

# Milestone 3.3 — Durable failure policy integration

Implemented 2026-10-13 from `e6f8a2f`; Engineering Library base `303ed86`. No applicable project AGENTS.md was found. The requested continuation report was not supplied; checkout contracts, history and dated 3.1/3.2 documentation were verified instead. Previous results remain historical. [Executed validation](../testing/validation-results.md#milestone-33--2026-10-13).

## Delivery contract and contextual decisions

[deliver_once](../../../backend/src/application/delivery.rs) makes one acquisition, at most one publication of the unchanged original envelope, and at most one selected persistence transition. It uses returned cycle, baseline, maximum, revision and token. Lifetime counts never reset. Confirmed publication completes even on the final permitted acquisition. Only acknowledged Applied completion is delivery success; scheduling and terminal preservation are failures of delivery.

| Outcome/context | Event action | Worker action |
| --- | --- | --- |
| None | No publication/transition | Idle pacing |
| Acquisition error, including CommitUncertain | No usable claim; no follow-up | Failure pacing |
| Confirmed publication | Complete once; preserve Applied/OwnershipLost/error independently | Continue only on Applied, otherwise failure pacing |
| Uncertain publication, including final acquisition | Retain lease; no transition | Replace publisher with reconnect pacing |
| Explicit permanent event-specific serialization attestation | Preserve serialization_permanent once | Failure pacing; retain usable publisher |
| Definitive final-budget failure | Preserve acquisition_budget_exhausted; storage_counter_exhausted takes precedence at i32 capacity | Failure pacing, or replacement for Unavailable |
| Unroutable/Rejected with remaining allowance | Schedule once with selected jitter; topology may be repaired | Failure pacing, then general admission |
| Unavailable before send with remaining allowance | Ordinary release once for infrastructure recovery, as ADR 008 specifies | Replace publisher; reconnect pacing |
| Serialization without contextual attestation | Retain lease: current adapter cannot prove event-specific permanence/correctability versus global encoder failure | Failure pacing; report category and retention |

`PublishError::permanent_serialization(FailureEvidence)` is an explicit trusted caller/adapter attestation of a reproduced event-specific encoder defect. Generic constructors never authorize this terminal reason. RabbitMQ supplies no such evidence today. No generic validator, poison inference, global-topology detector or permanent rejection classification is invented. Rejected/Unroutable are qualifying definitive failures; unavailable is infrastructure recovery, not an event schedule. Ordinary release is never used to bypass their durable due times. Error category, event decision and publisher lifecycle remain separate.

`PublicationFailed { claim, error, handling }` preserves the original sanitized publication error/source separately from `FailureHandling`: LeaseRetained, Released, Scheduled, Terminal or PolicyFailed. Scheduled/Terminal reuse `Result<RetryTransition<T>, PersistenceError>` so Applied, Conflict, ordinary errors and CommitUncertain remain explicit. Released retains existing Applied/OwnershipLost semantics. There is no transition retry or compensating completion cleanup. Failed/uncertain persistence cannot overwrite publication evidence or authorize another send.

## Equal jitter and authoritative scheduling

[Pure calculations and sampling](../../../backend/src/application/retry_policy.rs) reuse RetryBackoff's checked ceiling: min(300,000,000 µs, 1,000,000 µs × 2^(n−1)), where n is lifetime count minus persisted cycle baseline. Uniform inclusive whole-microsecond interval is [ceil(ceiling/2), ceiling]. `selected_delay(n, offset)` validates offset and checked addition; `deliver_once_with_delay` injects deterministic selection. Selection occurs once only when scheduling is chosen, never for uncertainty, completion, retention or terminalization.

Promote existing locked transitive rand 0.8.8 to a direct `rand = "0.8"` dependency; no new dependency version was downloaded. `thread_rng().gen_range(0..=ceiling-floor)` uses rand's rejection-based bounded integer sampler, not naive random modulo. The OS-seeded thread-local generator is neither held across await nor used for authority. Deterministic tests cover inclusive endpoints, cap, zero/overflow acquisition and invalid offsets without statistical assertions. [rand source](https://github.com/rust-random/rand/blob/0.8.5/src/distributions/uniform.rs) explains bounded integer rejection. PostgreSQL independently samples its post-lock clock and computes durable availability; application timestamps cannot grant ownership.

## Worker pacing and maintenance admission

The worker retains one in-flight operation, fresh acquisition tokens, publisher usability checks, interruptible waits and existing finite drain/overall cleanup deadlines. All publication failures (even acknowledged schedules or terminal states) incur RELAY_FAILURE_MS before general admission. This prevents a hot failure loop and permits unrelated eligible work; it never sleeps until one particular scheduled event. PostgreSQL excludes that event until available_at. RELAY_RECONNECT_MS protects resource replacement; RELAY_IDLE_MS protects empty polling. Definitive usable publishers are retained; reconnection does not reconcile earlier publication.

Candidate provenance: an operator's independent observation of exhausted raw metadata supplies optional `RELAY_MAINTENANCE_CANDIDATES=UUID:cycle:revision,...`. Empty by default. Parsing rejects nil IDs, invalid cycle/revision, duplicate IDs, more than 32 entries or more than 4096 bytes. The private validated configuration exposes a read-only slice; no unbounded queue or discovery port is added. Compose forwards this optional setting; no shared restart is performed.

Before each general delivery attempt, after usable-publisher admission, consume at most one configured candidate and call existing preserve_exhausted through the same drain boundary. At most 32 attempts per run; after each result, including Conflict/lock skip/ordinary error/CommitUncertain, general delivery remains possible. A candidate is consumed before await and never automatically repeated in that run. Pending configured work does not bypass publisher setup, so maintenance pauses during broker outage; this is deliberately limited coverage. Shutdown suppresses further admission.

Revision freshness comes only from independent observation, not automatic increments, guessed revisions or an old delivery snapshot. PostgreSQL rechecks exact ID/cycle/revision, due state and no live owner with SKIP LOCKED. A live final owner retains its opportunity to complete. Conflict does not establish whether a lock, live owner or changed revision caused it; observe again before another explicitly configured run. Restart re-reads configuration and can attempt the same entries again: remove stale entries and re-observe before restart. There is no durable candidate cursor, automatic discovery, fairness or general exhausted backlog coverage. Final uncertainty/crashes may leave exhausted rows requiring such explicitly supplied maintenance; 3.4 owns scanning.

## Cancellation and uncertainty

| Boundary | Possible effect / permitted recovery |
| --- | --- |
| Acquisition or after acknowledged acquisition | Lease/count may persist; no invented claim or cleanup; future fresh authority only |
| During publication | Acceptance may occur; adapter retires uncertain channel; no later transition from discarded invocation |
| Before schedule/terminal transition | If cancelled before the next await is polled, no new port call; acknowledged claim may remain |
| During schedule/terminal transition | Durable change may precede cancelled result; no detached compensation or blind repeat |
| Completion | Acceptance already confirmed; completion may persist; no release/schedule/terminalization or republish |
| Exact-ID maintenance | Terminal commit may occur despite discarded result; consumed candidate receives no automatic retry |

[Tokio select cancellation](https://docs.rs/tokio/latest/tokio/macro.select.html#cancellation-safety) concerns dropping the future, not proving rollback. Cancellation occurs at await boundaries; synchronous decision/sampling is not an independently interruptible phase. There is no asynchronous Drop mutation or detached cleanup. After uncertainty, future work proceeds only through durable eligibility and valid authority, with original-identity duplicate risk; generic reconciliation remains absent.

## Evidence and milestone limits

Controlled fakes cover no claim, unchanged envelope, all completion results, scheduling errors/conflicts, final/permanent terminalization, primary error preservation, one sampling decision, cancellation and worker admission/lifecycle/pacing. Real PostgreSQL acknowledgement-loss proxies exercise use-case scheduling and terminalization with an explicitly injected Rejected publisher (not claimed as real broker NACK). Real broker routing removal produces Unroutable, durable schedule, denied early acquisition, repaired routing and fresh-token original-identity completion; definitive final routing failure preserves dead_letter with processed_at NULL. Existing uncertain broker recovery still demonstrates possible duplicates. Configured real worker maintenance protects a live final owner while unrelated work completes; a separately observed expired candidate can terminalize in a new run. See actual validation for executed status.

No SQL, schema migration, RabbitMQ protocol, frontend or consumer behavior changes. Shared ownership/retry migrations remain pending and relay remains stopped. No data reset, credential modification, shared migration, activation, staging, commit or push. No new general scanner, replay or acceptance-closure claim.

3.4 must add bounded raw-row candidate discovery and envelope validation/poison evidence, isolate confirmed invalid stored envelopes under maintenance authority without constructing a valid owned envelope, preserve terminal history, protect live owners, define revision freshness/contention/fairness limits and prevent malformed rows repeatedly obstructing independent valid work. It must include exhausted metadata discovery because exact-ID configuration is not general backlog coverage. Invalid ownership/identity shapes need separately authorized audited repair, never invented authority. Authenticated audited replay remains 3.5; full Milestone 3 acceptance/closure remains 3.6.

## Authorized local rollout — 2026-10-13

[Current local operation](../operations/local-relay-rollout.md): both http/relay autostart, ownership/retry migrations installed explicitly, authenticated broker topology and manual/interactive controls verified. Earlier pending/stopped descriptions retain historical scope. Production readiness and remaining 3.4–3.6 work are not claimed.
