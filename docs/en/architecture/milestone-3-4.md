[Other language](../../pt-BR/architecture/milestone-3-4.md) | [ADR 008](../adr/008-durable-failure-policy.md)

# Milestone 3.4 — Bounded poison discovery and safe isolation

Implemented on 2026-10-13 against Rust a756797bbdd64d50671d536ae7de697a3705ce8b. The current local rollout already installed ownership/retry migrations and enabled both processes; earlier stopped/pending reports are historical. This change adds no migration and performs no shared activation. Replay is 3.5; full acceptance is 3.6.

## Detection and authority matrix

All automatic mutations require current pending state, due availability, usable non-nil identity, coherent ownership and retry history, incrementable revision, expected cycle/revision and absent/expired ownership, checked after locking. A scan is only an observation. Past broker acceptance remains possible for any row with acquisition history; neither exhaustion nor invalidity proves that publication never occurred. Even attempt_count zero is only stored history, not a universal external-publication attestation.

| Class / evidence | Automatic action | Operator / preservation |
| --- | --- | --- |
| Restorable, due, allowance remaining | Observe Valid; eligible for fresh exact-ID acquisition | No isolation; ordinary ownership/publication rules |
| Restorable, consumed cycle >= maximum | acquisition_budget_exhausted after fresh checks | Preserve raw envelope, identity, lifetime/cycle history; replay needs audit |
| Restorable, lifetime count = i32::MAX | storage_counter_exhausted; precedence over cycle budget | Capacity resolution requires explicit policy; never wrap/reset lifetime history |
| Checked envelope restoration returns typed EventError | invalid_stored_envelope, only with safe metadata and matching fresh typed evidence | Whitespace event/aggregate names and IDs violate envelope-v1; preserve original content |
| Unsupported version/representation, JSONB/timestamp decoder failure | UnsupportedRepresentation; no isolation | Diagnose compatibility/implementation; no permanence inferred from a decoder category |
| Nil identity, invalid ownership tuple/time, cycle/history/revision, exhausted revision capacity | RepairRequired; no invented ownership or overwrite | Separately authorized audited repair; preserve all fields |
| Query/pool/transport failure | Sanitized persistence error; no candidate or isolation | Infrastructure diagnosis; future bounded read only |
| Live lease, including final-budget owner | LiveOwner; no isolation | Owner can complete before expiry; expiry cannot revoke an already underway send |
| Not due | NotDue; no mutation | Revisit after wrap; availability is not a terminal decision |

Schema version is a positive u32 contract, not a business event catalog. Versions within that range are supported envelope metadata without asserting a known business payload schema. Out-of-range values/unsupported representations are not classified as permanent corruption. Concrete domain EventError is distinguished from private decoding failures through the existing checked StoredEvent restoration path used by reader/acquirer. Delivery serialization failure alone is still insufficient evidence of poison.

## Bounded traversal

OutboxCandidateDiscovery returns zero or one observation. PostgreSQL visits every pending row, including future/live/unsafe metadata, in UUID primary-key order. DiscoveryCursor contains after and through: first read observes a maximum table UUID as high-watermark; subsequent reads use id > after AND id <= through. No OFFSET. Terminal rows are filtered. No discovery locks: held rows can be observed from committed data; exact isolation uses SKIP LOCKED. Concurrent scanners can observe the same row, but fresh mutation predicates resolve competition.

After every returned row, application maintain_once advances the cursor before isolation, including unmanageable rows and terminal conflicts/errors/uncertain outcomes. Empty means no pending row in this suffix snapshot, resets the cursor and ends this traversal. It does not mean an empty backlog. New higher UUIDs wait for the next traversal; insertions/changes behind the cursor and lock-skipped mutations are revisited after wrap. Restart resets process-local progress; neither cursor nor observation queue is durable. The fixed watermark prevents newly arriving larger UUIDs from extending the current range indefinitely. Infinite contention, continuous changes or repeated restarts do not establish fairness or full coverage.

One read statement, at most one raw row/payload materialized, then at most one short isolation transaction. Only sanitized evidence and UUID/cycle/revision cross the persistence boundary; payloads are not retained to list candidates. JSON envelope validation still materializes one complete JSON value, with no payload-byte bound. LIMIT bounds returned rows and retained row count, not index pages, examined tuples, bytes or latency; large JSON and many filtered terminal rows remain resource risks. Dedicated pool acquisition/statement/lock deadlines remain in force. No new speculative index or payload-size milestone is claimed.

## Fresh isolation and evidence

OutboxCandidateIsolator locks exact ID FOR UPDATE SKIP LOCKED, then samples clock_timestamp in a separate statement. It rereads all envelope/ownership/retry fields, revalidates current pending/due/ownership eligibility and expected cycle/revision, and reruns the relevant envelope/budget validation. A changed typed evidence class conflicts. Current invalid content is preserved; no assumption that external writers advance revisions or keep envelopes immutable is needed to revalidate invalidity. Revision remains required for known transition history; an unaudited writer that changes valid history without revision updates is outside the protocol and cannot be reconstructed.

The existing atomic terminal UPDATE preserves the row as dead_letter, keeps processed_at NULL, clears authorized ownership and advances revision once with checked arithmetic. terminal_at/diagnostic_at use the same post-lock decision instant, not event occurrence or inferred historical failure time. Structured reason distinguishes invalid envelope/cycle/storage exhaustion. failure_evidence retains adr008-v1, maintenance phase, envelope-v1 attestation for invalid data, and original event UUID as evidence_reference to the retained row. The observed revision is the predecessor of the committed revision; retry cycle remains stored. Typed validator failure is available in bounded sanitized discovery logs and can be recomputed from the preserved raw row; unrestricted errors/payloads are never copied into evidence. Existing last_error and exact stored JSONB content are retained, without pretending to recover original producer JSON formatting.

Ordinary rejection rolls back with no partial mutation; stale/missing/locked/live/changed rows return Conflict, never prior-success inference. CommitUncertain may hide a committed terminal state. Cancellation can suppress a result after commit; dropping SQLx transactions queues rollback but does not prove immediate rollback. No blind mutation repeat, compensation, detached task or hidden reconciliation. Future traversal obtains a new observation. Constraints validate row shape, not all writers' historical authorization. Maintenance can terminalize an expired owner after possible publication; it cannot fence the broker.

## Worker scheduling and compatibility

Worker admits at most one maintenance unit per loop and at least one second after the previous unit finishes. A unit is one configured exact-ID attempt OR one discovery read plus at most one isolation. No catch-up bursts, unbounded queue, task spawning or additional concurrent maintenance. Maintenance uses the same shutdown/drain boundary as delivery. Connection attempts, delivery operations and interruptible idle/failure/reconnect pacing may delay the next unit; one second is an upper admission frequency, not a guaranteed deadline.

Database maintenance is explicitly before publisher connection, so broker outage permits a maintenance unit on each paced reconnect iteration. It does not run concurrently with a stuck bounded connect/delivery operation. Maintenance errors do not authorize delivery transitions and do not prevent an otherwise eligible delivery attempt. Delivery and maintenance logs have distinct sanitized outcome names.

A Valid discovery observation supplies at most one pending exact-ID selection hint. Worker consumes it before delivery; AcquireRequest::targeting changes selection only, never grants a lease or bypasses current eligibility/restoration/post-lock checks. This permits independent valid delivery beyond an unsafe/unsupported prefix without silently repairing it. Missing/stale/locked targets can yield NoEvent; later traversal reobserves them. Default acquisition keeps its existing availability ordering and malformed-selected-row error semantics. Only one UUID hint is retained, possibly overwritten during an outage; no delivery queue or fairness claim.

RELAY_MAINTENANCE_CANDIDATES remains compatible and optional: validated input <=4096 bytes, <=32 unique UUID:cycle:revision entries, consumed once per process run, now under the same one-second admission cap and before the broker gate. Configured entries temporarily take priority; discovery starts after these finite entries. Restart rereads exact-ID configuration, so remove stale observations. Legacy exhausted maintenance also checks coherent ownership metadata before clearing it. No new configuration is required; no shared worker restart was performed.

## Verification and next scope

[Executed results](../testing/validation-results.md) distinguish deterministic policy checks, isolated PostgreSQL schedules, and real broker/database observations. Tests do not establish universal fairness, power-loss durability, consumer effects or remote CI execution. Production constraints are untouched; malformed fixtures drop constraints only inside the exact disposable test database.

3.5 prerequisites: explicit authenticated/authorized replay intent, expected dead_letter revision, durable unique replay request/audit identity, concurrency-safe new retry cycle/baseline, preserved lifetime counter and original event ID/raw envelope, checked cycle/revision/storage capacity, validation/repair policy for invalid or unsupported envelopes, handling of replay COMMIT uncertainty/cancellation through independent observation, and tests rejecting live/nonterminal/stale/repeated unauthorized requests. Replay does not reconstruct or undo prior broker/consumer effects. Repair of unsafe identity/history needs a separately authorized audit protocol, not implicit replay. 3.6 owns integrated acceptance and closure; frontend simulation and consumer idempotency remain unchanged.

Reference: [PostgreSQL SELECT locking](https://www.postgresql.org/docs/current/sql-select.html) explains that SKIP LOCKED omits row-level lock contention but does not remove all table-lock waits or provide a consistent queue snapshot.
