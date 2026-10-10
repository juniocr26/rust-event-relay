# Authority, diagnostics and transport

[English](diagnostic-and-authority-boundaries.md) | [Português brasileiro](../../pt-BR/security/diagnostic-and-authority-boundaries.md)

Static source review: 2026-10-10. Implemented facts, general theory and hypothetical changes are distinguished below. Runtime commands were not executed.

The HTTP API is only `GET /health`, a fixed liveness response without dependency checks or login. Structured JSON tracing is configured by `RUST_LOG`. Persistence and publication errors expose sanitized Display/Debug categories while retaining typed source chains. Printing those sources may disclose connection details, SQL or payload data; safe outer formatting is not full log redaction. The review never read live credentials or application rows.

Ownership tokens authorize future conditional database transitions, not data confidentiality. Event UUID identifies a stable event across retries; ownership token identifies one acquisition. Pure delivery models reject zero/invalid tokens, incoherent times, stale owners and exact-expiry transitions. Nullable migration fields must be absent together or form a coherent pending lease. SQL constraints validate row shape; they do not prove transition history, correct clock sampling or concurrent ownership enforcement. Those require production mutation implementations and PostgreSQL concurrency tests, currently absent.

Short transactions and publication outside locks are the recorded ADR 007 design. A future completion/release must match event ID, token, pending state and an unexpired lease using database time after lock acquisition. Broker publication remains a separate boundary: the token cannot revoke a send already underway. End-to-end duplicate-tolerant effects would need consumer deduplication coupled with business changes. This is theory for future work, not an implemented consumer. Current AMQP is plaintext transport; durable storage and UUIDs do not supply application encryption, authentication signatures or production TLS.
