[Português brasileiro](../pt-BR/milestone-2-2.md) | [README](../../README.md)

# Milestone 2.2 — Delivery state and event ownership

Implemented 2026-10-09 from clean `5eac9ff`. No applicable project AGENTS.md found (dependency-cache instructions do not govern project files). No commit or push. Milestone 1 and 2.1 validation records are historical; see [new validation](validation-results.md#milestone-22--2026-10-09).

[ADR 007](adr/007-durable-delivery-ownership.md) selects durable leases and short transactions over holding connections/row locks during broker I/O. Snapshot data is separate from authority; tokens identify acquisitions, event IDs identify immutable envelopes. Database token checks cannot fence RabbitMQ or provide exactly-once delivery.

## Implemented types and ports

| File | Purpose |
| --- | --- |
| src/domain/delivery.rs | Checked duration/token/lease, pending/processed/dead_letter state metadata, pure acquire/complete/release rules |
| src/persistence/ownership.rs | Single-use AcquireRequest, OwnedEventKey, OwnedOutboxEvent and separate OutboxAcquirer/Completer/Releaser contracts |
| src/persistence/error.rs | CommitUncertain classification for future mutation adapters; existing reader mapping unchanged |
| migrations/20261009000000_add_delivery_ownership.*.sql | Nullable durable ownership fields and coherent row-shape constraint; reversible without dropping events |
| tests/delivery_ownership.rs | Invariant, boundary, stale-owner, attempt-limit, identity and Send/static-port checks |
| tests/delivery_ownership_schema.rs | Isolated real PostgreSQL migration, preservation, constraints, reader compatibility and down migration |
| .github/workflows/ci.yaml | Include isolated ownership schema test in PostgreSQL job; remote execution not asserted |
| tests/lifecycle.rs | Bounded asynchronous listener-unavailability assertion after two failures of the immediate synchronous shutdown probe |
| tests/persistence_contract.rs | Extend sanitized error/source coverage to CommitUncertain |

Private fields and checked restoration preserve meaningful invariants without SQLx/Lapin/row types inward. AcquireRequest generates a fresh token and is not Clone/Copy; callers can retain its token for diagnosis before passing the request by value. OwnedEventKey carries ID plus expected token; possession does not promise a still-active lease. OwnedOutboxEvent validates positive bounded committed acquisition count and availability no later than acquisition. The pure DeliveryState has no event envelope and cannot regenerate its identity. It models rules, not a production database repository or orchestration use case.

## Lifecycle

| Operation | Predicate using authoritative database instant after row lock | Atomic result |
| --- | --- | --- |
| Acquire | pending, availability due, lease absent or expires_at <= time, count < i32::MAX | Fresh token, acquired_at=time, expires_at=time+duration, count+1; status remains pending |
| Complete | pending, matching ID/token, acquired_at <= time < expires_at | processed, processed_at=time, all ownership fields NULL |
| Release | Same ownership predicate | pending with all ownership fields NULL; availability/count unchanged |

Ownership begins at known committed acquisition. At the exact expiry boundary old owners lose transition authority even if nobody acquired again. Expired pending work is acquired directly, with token replacement; no reaper or extra state. Terminal states have no lease; only processed has processed_at. Terminal transitions/replay policy remain absent. Shape constraints do not prove legal transition history.

Attempt_count counts committed acquisitions, including crashes before publication. Existing historical values remain unchanged. Overflow is explicit AttemptLimit in the model; future acquisition excludes exhausted candidates instead of wrapping/saturating. No eligible result does not prove an empty backlog. Available_at is earliest scheduling eligibility; release does not schedule retries. Acquired_at and expires_at describe each acquisition, not event creation. Processed_at records persistence completion after confirmed broker acceptance, not consumer work. PostgreSQL is authoritative, with one clock_timestamp() sample after obtaining the row lock; microsecond durations/times and checked addition avoid silent precision changes. Caller clocks cannot authorize writes.

OwnershipLost is an ordinary transition result, separate from sanitized infrastructure failure. Repeated completion/release returns OwnershipLost. Unknown COMMIT uses CommitUncertain, not known rollback; cancellation after dispatch can also leave uncertainty. Unconfirmed acquisition must not authorize publication; expire/recover it. Unknown completion/release requires reconciliation or recovery rather than assuming success/retained ownership. Confirmed broker acceptance alone may authorize the future completion call; every PublishErrorKind prevents successful completion. Broker acceptance followed by failed/uncertain database completion may duplicate on recovery, preserving original event ID.

## Schema compatibility and scope

Existing migration files remain unchanged. Existing rows get three NULL ownership fields without losing envelope identity or counter values. All fields must be absent or coherent: non-nil token, finite lease times, pending, positive attempts, available_at <= acquired_at < expires_at. Existing terminal timestamp constraint remains. The availability partial index remains sufficient for the planned due-candidate scan with residual expiry/count predicates; no speculative index is added. Milestone 2.3 must measure performance and test locked-row behavior.

The reader deliberately continues selecting pending rows by availability, including leased/exhausted rows. It neither filters for exclusive acquisition nor mutates ownership. Existing publisher and HTTP runtime remain unchanged. Shared/development data is not migrated for validation. The isolated test applies original schema, seeds an old row, snapshots every old column, applies the new migration, verifies constraints/reader immutability and rollback, then the existing harness closes pools, drops its exact generated database and verifies absence. Other PostgreSQL tests retain the original two-migration fixtures to test backward reader compatibility.

Remaining milestones: **2.3** production PostgreSQL acquisition/completion/release with bounded transactions, post-lock authoritative clock predicates, SKIP LOCKED selection, checked counters, committed return values, conflict/error/commit-uncertainty classification, real competing owners, lock-wait expiration and stale-owner tests. Pure/fake tests establish none of those database concurrency guarantees. **2.4** claim → publish → persist use case; **2.5** polling/composition/reconnection/Supervisor relay program; **2.6** end-to-end crash/recovery experiments. Retry schedules, backoff, quarantine and dead-letter policy are not selected here. No producer endpoint, CDC or end-to-end delivery claim.

## Validation commands

Check existing services first with docker compose ps. All Cargo commands below use the existing app container. Never apply migrations to shared data merely to run tests.

```bash
docker compose exec -T app cargo fmt --check
docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings
docker compose exec -T app cargo test --locked
docker compose exec -T app cargo test --locked --test delivery_ownership_schema -- --ignored
docker compose exec -T app cargo test --locked --test postgres_repository -- --ignored --test-threads=4
docker compose exec -T app cargo test --locked --test rabbitmq_publisher -- --ignored
docker compose exec -T app cargo test --locked --lib infrastructure::rabbitmq::tests::closed_owned_connection_is_unavailable -- --ignored
docker compose exec -T app cargo build --locked
docker compose exec -T -e RUSTDOCFLAGS='-D warnings' app cargo doc --locked --no-deps
git diff --check
```

The PostgreSQL harness needs explicit configured credentials and CREATE DATABASE; it fails rather than silently skipping requested integration. Abrupt harness death may leave its printed isolated database; clean only the exact owned name after inspection. See [testing](testing.md) for existing harness details and [results](validation-results.md) for checks actually run.
