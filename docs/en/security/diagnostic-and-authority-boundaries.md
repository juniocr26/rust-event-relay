# Current security and authority boundaries

The local HTTP application includes authenticated producer/actions/evidence, not only health. Demo Bearer authentication is independent of privileged replay credentials. Demo middleware checks token, origin/client header, body size, rate, concurrency and deadlines; operator replay derives actor/allowance from trusted server configuration and is disabled when REPLAY_OPERATORS is empty. Browser token stays in memory, identifiers in session storage. No public-user account system is implied.

PostgreSQL token/revision/lease checks authorize relay transitions. Consumer immutable-content/receipt constraints protect supported local effects against ordinary duplicate processing; privileged database owners can bypass safeguards. Neither mechanism fences external AMQP sends. Credentials, obfuscation of diagnostics and durable storage are not application encryption or TLS. Current local AMQP/HTTP must not be described as production transport security. Inspect [the final implementation boundaries](../guides/implementation-study.md) and [token operation](../operations/demo-token-connectivity.md). Earlier health-only/missing-consumer descriptions below are historical.

## Historical reference (original dates and milestone scope)

# Authority, diagnostics and transport

[English](diagnostic-and-authority-boundaries.md) | [Português brasileiro](../../pt-BR/security/diagnostic-and-authority-boundaries.md)

Static source review: 2026-10-10. Implemented facts, general theory and hypothetical changes are distinguished below. Runtime commands were not executed.

## Current update — 2026-10-12

[Milestone 2.3](../architecture/milestone-2-3.md) implements conditional ownership transitions with real contention/expiration evidence. Ownership migration remains pending in development, verified by read-only metadata; only isolated test databases were migrated. No broker fencing, end-to-end orchestration, production performance or lost-COMMIT-acknowledgement experiment is established. The source review below is historical (2026-10-10).

## Historical review


The HTTP API is only `GET /health`, a fixed liveness response without dependency checks or login. Structured JSON tracing is configured by `RUST_LOG`. Persistence and publication errors expose sanitized Display/Debug categories while retaining typed source chains. Printing those sources may disclose connection details, SQL or payload data; safe outer formatting is not full log redaction. The review never read live credentials or application rows.

Ownership tokens authorize future conditional database transitions, not data confidentiality. Event UUID identifies a stable event across retries; ownership token identifies one acquisition. Pure delivery models reject zero/invalid tokens, incoherent times, stale owners and exact-expiry transitions. Nullable migration fields must be absent together or form a coherent pending lease. SQL constraints validate row shape; they do not prove transition history, correct clock sampling or concurrent ownership enforcement. Those require production mutation implementations and PostgreSQL concurrency tests, currently absent.

Short transactions and publication outside locks are the recorded ADR 007 design. A future completion/release must match event ID, token, pending state and an unexpired lease using database time after lock acquisition. Broker publication remains a separate boundary: the token cannot revoke a send already underway. End-to-end duplicate-tolerant effects would need consumer deduplication coupled with business changes. This is theory for future work, not an implemented consumer. Current AMQP is plaintext transport; durable storage and UUIDs do not supply application encryption, authentication signatures or production TLS.

[Milestone 3.5 — replay](../architecture/milestone-3-5.md) | [ADR 009](../adr/009-authorized-idempotent-replay.md) | [Operator replay](../operations/replay.md)
