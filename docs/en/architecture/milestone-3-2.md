[Other language](../../pt-BR/architecture/milestone-3-2.md) | [ADR 008](../adr/008-durable-failure-policy.md)

# Milestone 3.2 — Durable retry state and PostgreSQL transitions

Implemented 2026-10-12. This is persistence support, not application policy integration. `deliver_once`, the worker loop and frontend are unchanged. Backoff/jitter selection remains the pure 3.1 policy; no adapter chooses randomness or retries transactions.

## Schema and backfill

The additive `20261012000000_add_durable_retry` migration follows ownership migration `20261009000000`. Applied migrations are untouched. Event identity, envelope, status, availability, lifetime attempt_count, last_error and live/expired leases remain intact.

| Column | Representation and invariant | Initial/backfill value |
| --- | --- | --- |
| retry_cycle | INTEGER > 0 | 1 |
| cycle_baseline | INTEGER >= 0 and <= attempt_count | 0 |
| cycle_maximum | INTEGER > 0 | 10 |
| state_revision | BIGINT >= 0 | 0 |
| diagnostic_code | Nullable allowlisted static code | NULL |
| diagnostic_at | Nullable finite TIMESTAMPTZ | NULL |
| failure_evidence | Nullable allowlisted JSONB object, serialized UTF-8 <= 2048 bytes | NULL |
| terminal_at | Finite TIMESTAMPTZ iff dead_letter | Migration observation time for legacy dead_letter |
| terminal_reason | Static reason iff dead_letter | legacy_dead_letter for legacy terminal rows |

Pending and processed rows gain no terminal metadata. Existing dead_letter stays terminal: `terminal_at=statement_timestamp()` records migration observation, **not reconstructed historical failure time**. Legacy last_error is preserved verbatim as restricted historical data; new methods do not expose, erase or copy it into structured evidence. Historical acquisitions count from baseline zero: an existing count >= 10 has already exhausted this cycle. Live final owners retain completion authority until expiry; rows are not silently renewed or reactivated.

Named checks `ck_outbox_events_retry_history`, `ck_outbox_events_diagnostic` and `ck_outbox_events_terminal` complement the existing ownership/processed checks. INTEGER/BIGINT enforce representability. Diagnostic code/time/evidence are all absent or all present; JSON null is rejected using `IS TRUE`, avoiding SQL CHECK's nullable-result loophole. Terminal new decisions require matching diagnostic reason/time and budget/storage evidence when those reasons apply. All terminal rows have no owner and processed_at NULL. Checks enforce shape, not authorization or complete transition history.

No new index: exact-ID transitions use the primary key; acquisition uses the existing pending availability index with residual cycle/storage/revision predicates. Maintenance targets one observed exact ID, so no speculative scanner index is needed.

Down removes only these nine columns/checks, retaining envelopes, lifetime counters, status, last_error and availability. It destroys cycle/revision/failure/terminal audit information and is tested only on isolated fixtures. It is not a recovery strategy or an authorized shared rollback.

## Models, ports and authority

[Domain models](../../../backend/src/domain/retry.rs) own checked history, delay and evidence. [Focused ports](../../../backend/src/persistence/retry.rs) use native Send futures/static dispatch; [PostgreSQL adapter](../../../backend/src/infrastructure/postgres/retry.rs) owns row locks, driver mapping and transactions. No generic CRUD, replay API or dependency was added.

`RetryState` validates cycle/baseline/maximum/revision against the lifetime count. Consumption uses checked subtraction; revisions use checked addition. `OwnedOutboxEvent::retry_state()` returns the committed acquisition metadata. Its existing constructor remains usable for synthetic callers; production acquisition attaches validated persisted state with `with_retry_state`. The pending reader remains an observation API, not retry authority or a full raw-row maintenance view.

`RetryDelay` accepts positive whole microseconds up to one day and rejects sub-microsecond Duration values. Checked addition rejects Chrono overflow and PostgreSQL's finite lower bound; Chrono's upper bound is below PostgreSQL's. Production uses only post-lock database time; caller timestamps cannot authorize a transition. Equal-jitter randomness and the default 1 s/300 s settings remain application decisions.

`FailureEvidence` contains fixed policy_version=adr008-v1, typed phase, optional fixed validator_version=envelope-v1 and optional non-nil UUID evidence_reference. It accepts no free-form strings, payload, credentials, connection strings or driver source chains. Evidence is a caller's authorized diagnostic snapshot, not proof of broker nonacceptance or consumer nonprocessing. The raw retained row remains restricted evidence. Terminal timestamp/reason and retained cycle/count metadata preserve the decision without an extra quarantine table; durable replay-request audit remains 3.5.

`RetryOwner` combines event ID/token with expected cycle/revision. `ScheduleRetry` and `PreserveFailure` require live pending authority. Result is `RetryTransition::Applied(value)` or `Conflict`, independent of infrastructure failure. Wrong/missing identity, stale token/cycle/revision, nonpending state or invalid lease conflicts. A repeat after successful mutation conflicts; zero rows never imply idempotent success. The fresh token fences database writes, not RabbitMQ sends.

All relevant mutations increment revision once: acquisition, completion, ordinary release, schedule and terminal preservation. Acquisition returns the advanced revision only after acknowledged COMMIT. Completion/release keep their existing key/token contracts and OwnershipLost result: under row lock their token identifies the current acquisition; they do not require callers to adopt a new request. A valid final owner can complete or release without resetting budget or erasing diagnostic history. All external writers must follow this revision contract; direct unaudited SQL is not a supported concurrent recovery path.

## Exact transitions

Every operation starts a short Read Committed transaction. Owned transitions lock exact ID without time predicates, then sample clock_timestamp() in a separate statement, requiring acquired_at <= now < expires_at. All stored times/predicates in that operation use this sample. No broker I/O occurs in the transaction.

- `schedule_retry`: recheck ownership/cycle/revision and remaining cycle **and storage** allowance; persist available_at=database_now+validated_delay, diagnostic/evidence and advanced revision; atomically clear lease; retain pending state, lifetime count, cycle history and envelope. The final permitted acquisition cannot schedule another attempt. Only definitive contextual failure authorizes the application to call this port; unknown publication acceptance does not.
- `preserve_failure`: persist authorized static reason/evidence, terminal_at=diagnostic_at=database_now, dead_letter and advanced revision; clear ownership; leave processed_at NULL and preserve original fields/history. Budget/storage reasons require corresponding durable exhaustion. The repository does not infer permanent invalidity from driver/publisher categories.
- Acquisition: due pending, absent/expired lease, lifetime < i32::MAX, lifetime-baseline < maximum and revision < i64::MAX. Lock one candidate with FOR UPDATE SKIP LOCKED, restore its envelope, sample post-lock time and revalidate before checked increment/new lease/revision. Count 9 may acquire slot 10; count 10 cannot acquire slot 11. None means no acquirable candidate, not no backlog.
- `preserve_exhausted`: one exact ID with expected cycle/revision, FOR UPDATE SKIP LOCKED, no owned-envelope restoration. Recheck due pending and absent/expired lease using post-lock time; preserve as storage_counter_exhausted when lifetime=i32::MAX, otherwise acquisition_budget_exhausted when consumption >= maximum. It never acquires/sends, repairs envelopes or overwrites a live lease. A locked row conflicts, allowing completion/other maintainers to proceed; callers must observe again rather than presume fairness. Two maintainers cannot apply the same revision twice.

Maintenance intentionally requires an independently observed ID/cycle/revision and has no scan/list API, batch setting or polling loop. It may preserve a malformed envelope **only because the raw metadata demonstrates exhaustion**; it does not validate poison. General invalid-envelope diagnosis/scanning remains 3.4. Invalid ownership shapes need separately authorized repair, never invented authority.

Revision overflow is OperationFailed with no partial mutation; acquisition excludes nonincrementable revisions. Such exceptional rows require an explicit operational repair/widening design, not wrap/reset. Storage-counter exhaustion preserves history and cannot be solved by ordinary replay.

## Failure and cancellation

| Boundary | Result and durable meaning |
| --- | --- |
| Validation/arithmetic or statement failure before COMMIT | OperationFailed/InvalidStoredData or classified driver failure; no partial mutation; no hidden retry |
| Explicit ordinary server rejection of COMMIT | Known failure under existing mapping; transaction rejected |
| Lost COMMIT acknowledgement, connection-class/completion-unknown/shutdown error during COMMIT | CommitUncertain; schedule/terminal state may already be durable |
| Ownership/cycle/revision/eligibility conflict | Conflict, no mutation; repeat is not success |
| Dropped/cancelled future | May suppress a durable result; dropping does not prove immediate rollback |

Observe exact durable state independently after uncertain effects; no new generic reconciliation capability is claimed. An uncertain schedule may already have selected timing and cleared authority; recomputing and blindly repeating may overwrite history or duplicate publication. Uncertain terminalization may already have isolated the row. Failed/uncertain completion still permits neither compensating release nor blind republication.

## Rollout, tests and remaining milestones

Shared migrations and shared relay startup were **not** performed. Reuse the existing Docker/PostgreSQL/broker; no volume resets or credential changes are part of 3.2. The prior explicit credential task is separate. The binary's read-only preflight now requires retry columns before publisher/polling setup. Supervisor source directories now match `/app/backend`; no shared restart/rebuild was needed for validation.

Before a separately authorized rollout, stop admission/old writers, inspect migration history and current mounts, count existing exhausted pending rows (including live/expired leases), review legacy diagnostics/raw access, then apply migrations in order and deploy compatible binaries/writers. Record that baseline zero grants no fresh allowance. Do not use down or reset as recovery. No automatic activation or production readiness claim is made.

[Tests](../../../backend/tests/postgres_retry.rs) run only in UUID-named harness-owned databases with exact cleanup: backfill/down, constraint NULL/overflow/evidence checks, post-lock clock/expiry, wrong/stale authority, repeat conflicts, exact budgets, concurrent final acquisition, final completion, maintenance/live owners/concurrency/raw malformed envelopes, storage/revision limits, statement/deferred COMMIT rejection, lost COMMIT acknowledgements with independent observation, and cancellation both during a lock wait and after server COMMIT while acknowledgement is withheld. [Pure model tests](../../../backend/tests/retry_state.rs) cover input/precision/time bounds. Existing owner/repository/schema and integrated recovery regressions remain separate. See [executed results](../testing/validation-results.md).

```bash
docker compose exec -T app cargo test --locked --test postgres_retry -- --ignored --test-threads=4
```

3.3 wires contextual definitive failure decisions, persisted cycle allowance and explicit unbiased jitter into deliver_once/worker while preserving uncertain leases and completion semantics; it must define bounded maintenance admission through the exact-row port. 3.4 adds general poison diagnosis/scanning. 3.5 adds authenticated audited replay with original identity. 3.6 adds integrated milestone acceptance/closure. Frontend remains illustrative and unchanged; 3.2 alone does not claim a deployed retry pipeline.

Primary references: [PostgreSQL CHECK semantics](https://www.postgresql.org/docs/18/ddl-constraints.html), [time sampling](https://www.postgresql.org/docs/18/functions-datetime.html), [timestamp bounds](https://www.postgresql.org/docs/18/datatype-datetime.html).

## Milestone 3.3 — current integration

Application delivery/worker integration is implemented; earlier sections retain their dated historical scope. [Current contracts, jitter, contextual outcome table, maintenance and cancellation limits](milestone-3-3.md). General poison scanning remains 3.4, audited replay 3.5 and full acceptance 3.6; shared rollout remains pending.
