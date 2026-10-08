[Português brasileiro](../pt-BR/postgres-repository.md) | [README](../../README.md)

# PostgreSQL repository — Milestone 1.6

`PostgresOutboxRepository::new(pool)` is the usable infrastructure construction boundary. The caller creates/configures a `PgPool` (for example with `PgPoolOptions::connect_with` and `PgConnectOptions`) and owns shutdown. Query methods never read environment variables or create global connections. Existing HTTP startup remains HTTP-only. Infrastructure implements the existing `OutboxReader` and depends on persistence/domain; SQLx types never enter those modules. Private `StoredEvent` data and decoding belong exclusively to infrastructure. Models contain validation/conversion, no queries. ADR 005 remains unchanged.

SQLx is pinned to 0.8.6, matching the Docker CLI. Defaults are disabled; only postgres, runtime-tokio, uuid, chrono and json are enabled. Rust 1.95.0 is the checked Docker baseline. No macros, offline metadata, application migration feature or TLS backend is enabled. The local Docker connection works without TLS; a future deployment requiring TLS must deliberately enable/configure it. See [SQLx runtime/TLS documentation](https://docs.rs/sqlx/0.8.6/sqlx/).

## Read semantics

One parameterized runtime query selects explicit envelope columns plus attempt_count/available_at from `relay.outbox_events`. It filters `status = 'pending' AND available_at <= $1`, orders by `available_at, created_at, id` to match the pending index, and uses `LIMIT $2`. Each statement observes a PostgreSQL statement snapshot. Selection order is an implementation detail, never a business delivery guarantee. No locks, claims, reservations, writes or second query occur. Other readers can receive the same events, and returned state may immediately become stale.

`usize` limits use `i64::try_from`. Values above PostgreSQL's signed BIGINT range return OperationFailed with the typed conversion source before database I/O. They are neither truncated nor saturated; no arbitrary application cap is imposed. The implementation allocates according to actual returned rows, without reserving the requested maximum. Count limits do not bound payload bytes or aggregate concurrency; callers must choose sensible operational limits.

## Faithful restoration

A private row is restored through `EventEnvelope::restore(EventEnvelopeParts)`. The same TryFrom implementation serves Serde restoration and checks aggregate fields/version; EventType validates its own name. This minimal storage-independent API preserves IDs, times, payload and optional UUIDs without calling `new`. Whitespace around nonempty strings is preserved. BIGINT version is checked as u32 and zero is rejected by domain validation; signed INTEGER attempts use checked u32 conversion. Every selected row must decode/validate, or the whole batch fails with InvalidStoredData; partial results are never returned.

The private timestamp decoder reads PostgreSQL binary microseconds from its 2000 epoch with checked Chrono arithmetic. Infinity sentinels and finite dates outside Chrono's range fail; the text fallback parses timestamps with offsets. This avoids SQLx 0.8.6's unchecked Chrono arithmetic, which the initial smoke test showed can panic on infinity. A private JSONB decoder checks binary version 1 and rejects invalid JSON/unknown versions. serde_json arbitrary_precision preserves large integers, long decimals and large exponents without float rounding; JSON null is valid. JSONB already normalizes stored JSON formatting/key ordering; restoration preserves the stored JSON value, not original producer text. TIMESTAMPTZ preserves instants at database microsecond precision, not original timezone spelling or producer nanoseconds.

## Error mapping and diagnostics

| Failure | Category |
| --- | --- |
| I/O, TLS, closed/timed-out pool, crashed worker | Unavailable |
| SQLSTATE class 08 (connection), 28 (authentication), 53300 (connection capacity), 57P01/02/03 (shutdown/unready) | Unavailable |
| Selected row decoding, invalid model fields, unsupported timestamps/JSON formats and checked stored-number conversion | InvalidStoredData |
| Other driver/database failures, including missing table, permission denial, cancellation or transaction failures | OperationFailed |
| Request limit exceeds i64 | OperationFailed before I/O |

The fallback is conservative OperationFailed. Categories do not promise successful retry; policy belongs to callers. Typed SQLx/domain/conversion sources are retained. Public Display/Debug expose static classification only; repository methods emit no logs. Never log raw pools/options, credentials, payloads or source chains; diagnostic inspection requires redaction.

Unit tests run without PostgreSQL. The opt-in [smoke test](../../tests/postgres_read_smoke.rs) uses an isolated database and fixture SQL outside the production API. It covers empty reads, cutoff/status filtering, bounds, identity/time/JSON/UUID fidelity, repeated read-only observation and malformed timestamp/model rejection and exact numeric JSON preservation. See [execution and cleanup](testing.md). This is not the full Milestone 1.7 integration suite or Milestone 1.8 concurrency/failure/transaction analysis. Writes, leases, lifecycle changes, delivery and exactly-once guarantees remain out of scope.
