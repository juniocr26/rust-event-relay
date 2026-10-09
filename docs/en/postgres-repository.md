[Português brasileiro](../pt-BR/postgres-repository.md) | [README](../../README.md)


## Current extension — Milestone 2.2

[Delivery state and ownership](milestone-2-2.md) and [ADR 007](adr/007-durable-delivery-ownership.md) now define durable lease recovery and separate acquisition/completion/release contracts. New migration `20261009000000_add_delivery_ownership` adds nullable token/acquired_at/expires_at with coherent pending-only leases. Earlier milestone sections below describe their original scope; earlier claims that ownership/attempt semantics are undecided are superseded by ADR 007. Production mutation adapters remain deferred to 2.3; reader SELECT and publisher behavior remain unchanged.
# PostgreSQL repository — Milestone 1.6

`PostgresOutboxRepository::new(pool)` is the usable infrastructure construction boundary. The caller creates/configures a `PgPool` (for example with `PgPoolOptions::connect_with` and `PgConnectOptions`) and owns shutdown. Query methods never read environment variables or create global connections. Existing HTTP startup remains HTTP-only. Infrastructure implements the existing `OutboxReader` and depends on persistence/domain; SQLx types never enter those modules. Private `StoredEvent` data and decoding belong exclusively to infrastructure. Models contain validation/conversion, no queries. ADR 005 remains unchanged.

SQLx is pinned to 0.8.6, matching the Docker CLI. Defaults are disabled; only postgres, runtime-tokio, uuid, chrono and json are enabled. Rust 1.95.0 is the checked Docker baseline. No macros, offline metadata, application migration feature or TLS backend is enabled. The local Docker connection works without TLS; a future deployment requiring TLS must deliberately enable/configure it. See [SQLx runtime/TLS documentation](https://docs.rs/sqlx/0.8.6/sqlx/).

## Read semantics

One parameterized runtime query selects explicit envelope columns plus attempt_count/available_at from `relay.outbox_events`. It filters `status = 'pending' AND available_at <= $1`, orders by `available_at, created_at, id` to match the pending index, and uses `LIMIT $2`. Each SELECT observes a consistent snapshot under the connection's effective isolation; at Read Committed it begins at statement start. Selection order is an implementation detail, never a business delivery guarantee. No row ownership locks (FOR UPDATE/SHARE), claims, reservations, lifecycle writes or second query occur. Normal SELECT table locks still apply; see [failure/transaction semantics](failure-and-transaction-semantics.md). Other readers can receive the same events, and returned state may immediately become stale.

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
| Other driver/database failures, including missing table, permission denial, reported server-side query cancellation or transaction failures | OperationFailed |
| Request limit exceeds i64 | OperationFailed before I/O |

The fallback is conservative OperationFailed. Categories do not promise successful retry; policy belongs to callers. Typed SQLx/domain/conversion sources are retained. Public Display/Debug expose static classification only; repository methods emit no logs. Never log raw pools/options, credentials, payloads or source chains; diagnostic inspection requires redaction.

Unit tests run without PostgreSQL. The [Milestone 1.7 integration suite](../../tests/postgres_repository.rs) exercises the public OutboxReader against real PostgreSQL in independent isolated databases. It covers eligibility/bounds/adapter tie-breakers, every restored field, exact stored JSON values, read-only observation by two readers, selected invalid data and recovery, closed pools and missing-table failures. Display/Debug stay sanitized and typed sources remain available. See [configuration, cleanup and limitations](testing.md). Milestone 1.8 [documents failure/transaction semantics](failure-and-transaction-semantics.md); crash/recovery injection remains future validation; producer writes, claims, leases and delivery remain unimplemented.
