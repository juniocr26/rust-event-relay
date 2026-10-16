[Português brasileiro](../../pt-BR/architecture/milestone-4-3.md) | [Catalog](../index.md)

# Milestone 4.3 — RabbitMQ consumer and confirmed quarantine

Implemented 2026-10-16 on Rust base `652bbd21e1cac5dbd4fe587fcf03dcd73a906625`. The uncommitted 4.2 diagnostic/test/documentation corrections were preserved. Library base `a25591eaf04a7f95b1769b7dbc9d6b7b403945a5` also had two pre-existing documentation edits. [4.2](milestone-4-2.md) is the real atomic adapter, not a fake: this slice adds delivery orchestration, raw preservation, explicit AMQP lifecycle and a third compiled binary. It does not close Milestone 4 or activate shared consumption.

## Boundaries and concrete flow

[consume::handle_delivery](../../../backend/src/application/consume.rs) validates one bounded delivery and sequences the existing processing/preservation ports with an IncomingDelivery contract. Domain/application remain independent of Lapin and SQLx. [PostgresInventoryProcessor](../../../backend/src/infrastructure/postgres/consumer.rs) still owns its short READ COMMITTED transaction; there is no SQL transaction open during quarantine confirmation or ACK. [AMQP adapters/runtime](../../../backend/src/infrastructure/rabbitmq/consumer.rs) own protocol objects and the original delivery's acker. [consumer binary](../../../backend/src/bin/consumer.rs) owns configuration, pool, signal handling and schema preflight; it never migrates.

| Step | consumer.stock(local, SKU-1) | Receipt / broker action |
| --- | ---: | --- |
| Fixture setup | 10 | No receipt |
| Receive event A, quantity 5 | 10 | Source remains unacknowledged |
| Atomic acknowledged commit | 15 | A records before 10 / after 15 / completion timestamp |
| Individual source ACK | 15 | Broker can retire original |
| Redelivered identical A | 15 | Durable comparison returns historical Duplicate(15), then ACK |
| Conflicting A | 15 | Preserve conflicting bytes in quarantine, confirm routing, then ACK |
| Invalid JSON / unsupported event | 15 | No inventory call; preserve bytes, confirm, then ACK |

`relay.outbox_events.processed` means relay publication completion. `consumer.receipts` records the local business effect. Broker ACK retires a delivery and is not a business receipt. SQL `dead_letter` preserves relay failures; RabbitMQ quarantine preserves consumer-invalid/conflicting messages. Quarantine success is preservation, not successful inventory processing. Replay/purge for quarantine, producer endpoints, external effects and frontend integration remain absent.

## Source and quarantine topology

Read-only shared inspection found durable classic `relay.local`, `x-queue-type=classic`, non-auto-delete, zero consumers, binding `relay.events` → `relay.local` with key `event`, and no effective DLX/TTL policy. Nothing was consumed or changed. Consumer source declarations explicitly require classic; mismatched type/durability/exchange declarations fail with sanitized diagnostics and operator guidance. No delete/recreate fallback exists.

| Setting | Default |
| --- | --- |
| CONSUMER_SOURCE_EXCHANGE | relay.events (durable direct) |
| CONSUMER_SOURCE_QUEUE | relay.local (durable classic, non-exclusive, non-auto-delete) |
| CONSUMER_SOURCE_ROUTING_KEY | event |
| CONSUMER_QUARANTINE_EXCHANGE | relay.quarantine (durable direct) |
| CONSUMER_QUARANTINE_QUEUE | relay.quarantine.inventory (durable, non-exclusive, non-auto-delete) |
| CONSUMER_QUARANTINE_ROUTING_KEY | inventory |
| CONSUMER_NAME | inventory.stock.v1 |

Source and quarantine exchange/queue names must differ and follow relay.* conventions; names/routing keys fit AMQP short strings. These source settings are explicit and independent of the relay publisher's RABBITMQ_EXCHANGE/QUEUE/ROUTING_KEY. All replicas of one logical responsibility must share CONSUMER_NAME; a different name has independent deduplication and can apply another effect to the same stock. A consumer tag is broker subscription identity only.

No DLX, TTL, queue expiry, purge or replay is configured. Declaration establishes the required binding but AMQP confirms/mandatory do not name the destination queue. Deployment must prevent topology/policy drift and extra/missing routes, ensure the expected durable destination and restrict configuration authority. Effective broker policies can override declaration assumptions and cannot be inspected through basic.queue.declare: before rollout inspect policies/operator policies, bindings and permissions, including absence of silent drop-head/TTL/DLX. Do not combine broker DLX and application preservation for this path. Durable configuration alone is not backup or a power-loss acceptance test.

## Preservation format v1 and bounds

[QuarantineMessage](../../../backend/src/application/consumer.rs) carries the exact original body (up to 1 MiB), original message_id including absence/malformed text (up to 255 bytes), and a JSON sidecar (up to 16 KiB including metadata). The actual publication body is unchanged; its properties use delivery_mode=2, content_type=application/octet-stream, type=relay.quarantine.v1 and one header `relay-preservation-v1` containing UTF-8 JSON. Original expiration, delivery mode, user_id and other properties are evidence only; they are not reapplied as quarantine TTL/authority.

Sidecar fields: format_version=1, logical consumer, actual source_exchange/source_routing_key, UTC observed_at, closed reason enum, original_properties. Observation time is a client observation, not database commit evidence. Reasons distinguish MalformedJson, InvalidContent, InvalidIdentity, Unsupported, IdentityConflict and StockOverflow. Malformed envelope shape is InvalidContent; syntactically invalid JSON is MalformedJson. Require a nonnil canonical UUID and matching parseable message_id for business processing. Never generate a replacement identity for invalid input.

[Properties encoder/checked decoder](../../../backend/src/infrastructure/rabbitmq/properties.rs) preserves all BasicProperties fields and header types exposed by installed Lapin 4.12.0. Header values retain enum tags, nested arrays/tables, signed/unsigned widths, decimal scale/value and timestamp. LongString and ByteArray bytes use base64. FloatBits/DoubleBits store IEEE integer bits, including NaN, infinities and negative zero; plain JSON float serialization would lose them. Header traversal is limited to 512 charged nodes and depth 16, with an early byte budget and final serialized-size check. Decoder checks bounds and canonical re-encoding; it grants no replay authority. Lapin already maps wire aliases into its client model and materializes bodies/properties: this is not a capture of original AMQP frames, duplicate table keys or distinctions the client erased.

Original headers, including a forged `relay-preservation-v1`, remain nested inside original_properties; they are never merged into internal metadata. Quarantine queue permissions and origin must be authenticated before trusting a record: a producer able to write there could forge an entire record. Bounds are checked before JSON parsing/body copying; serde also has a recursion limit. Lapin's already allocated body and buffers mean a 1 MiB application limit is not a hard process-memory cap. Broker admission/max-message-size, frame size, positive prefetch and capacity monitoring remain deployment prerequisites.

Oversize/unsupported exact preservation is never truncated. The runtime cancels/closes and exits cleanly with an operator-recovery error log, leaving the source recoverable; Supervisor will not repeatedly restart an exit 0 after RUNNING. Inspect retained source, provide an explicit lossless external preservation protocol or supported limit change, then explicitly restart. No automatic discard is authorized.

## Decision table and uncertainty

| Primary result | Preservation | Source action / recovery |
| --- | --- | --- |
| Applied after acknowledged COMMIT | none | Individual ACK |
| Verified identical Duplicate | none | Individual ACK; no new effect |
| IdentityConflict | persistent mandatory publish, confirm without return | ACK only after preservation succeeds |
| Invalid / unsupported / overflow | same preservation | ACK only after preservation succeeds |
| Known DB rejection / Unavailable / InvalidStoredData | none | No ACK; close session, paced reconnect |
| CommitUncertain or whole-processing timeout | none | No ACK/quarantine; later durable identity comparison |
| Quarantine nack / return / unavailable / timeout / disconnect | failed or uncertain | No ACK; retire uncertain publisher, close, pace |
| Source ACK error / timeout / poisoned acker | retain primary result and preservation evidence | Close, pace; redelivery may occur |
| Consumer cancellation / connection loss | no fabricated result | Close/reconnect/resubscribe with pacing |
| Preservation outside bounds | blocked | Retain source; stop for operator recovery |

HandlingOutcome retains primary processing/validation, preservation and acknowledgement results independently; sanitized logs never include bodies, credentials, properties or source chains. A phase timeout during processing can race COMMIT, so it is conservatively CommitUncertain. There is no adapter retry or blind quarantine on a database failure. Later identical redelivery either discovers the committed historical receipt or safely applies after rollback. Negative observation cannot exclude an in-flight commit.

The quarantine publisher reuses only the confirmed raw transport extracted from RabbitMqPublisher, not EventPublisher's valid-envelope contract. One gate bounds publication; mandatory Ack(None) succeeds, Ack(return) is Unroutable, Nack is Rejected, and missing/lost confirmation is Uncertain. Cancellation after send retires that publisher. Source ACK uses BasicAckOptions{multiple:false} and the original acker; Lapin 4.12.0 returns false for used/poisoned ackers, which is treated as failure. Client ACK completion is not a server acknowledgement of that ACK. Crash after preservation confirm before source ACK, or ACK uncertainty, can create another usable quarantine copy. No global quarantine deduplication is promised.

## Lifecycle, admission and shutdown

Lapin 4.12.0 defaults have auto_recover=false and zero connection retries; installed source was inspected for confirms, consumer Stream/Drop, acker and connection properties. Explicit outer lifecycle connects the preservation publisher before subscribing to source, declares compatible topology, sets per-consumer QoS (global=false), and uses manual acknowledgement. One sequential operation is admitted; no task is spawned per delivery. Prefetch limits outstanding deliveries, not active operations; additional prefetched messages stay unprocessed in client/broker buffers.

After a failed/uncertain operation the subscription/connection is closed, then the runtime waits an interruptible reconnect delay before new admission. Closing causes unacknowledged deliveries to become eligible for broker redelivery; no immediate nack/requeue loop is used. BasicCancel stops future subscription delivery but does not requeue outstanding work by itself. Server cancellation/stream termination triggers the same replacement. Startup setup failures are paced and produce sanitized guidance; missing database schema fails before subscription. Publisher retirement replaces the whole session.

SIGINT/SIGTERM stops application admission, cancels subscription and drains the current future within the configured drain budget. On expiry it drops that operation without fabricating ACK/preservation success. Explicit bounded close recovers current and prefetched unacknowledged deliveries through broker semantics; pool cleanup shares the absolute shutdown deadline. Close errors/deadlines are logged. No owned asynchronous Drop hooks or detached cleanup tasks are introduced; Lapin itself has internal Drop behavior, which is not relied on as proof of rollback or retirement. Broker detection/redelivery can lag transport failure. A shutdown while setup is pending may exhaust its drain before connection setup finishes; explicit deadlines bound application wait, not instantaneous broker cleanup.

| Configuration (ms unless count) | Default | Accepted range |
| --- | ---: | --- |
| CONSUMER_PREFETCH | 1 | 1..32 |
| CONSUMER_OPERATION_MS (each DB / preservation / ACK phase) | 10000 | 1..60000 |
| CONSUMER_RECONNECT_MS | 5000 | 1..60000 |
| CONSUMER_DRAIN_MS | 15000 | 1..20000, less than shutdown |
| CONSUMER_SHUTDOWN_MS | 25000 | 1..25000 |
| CONSUMER_POOL_MAX | 2 | 1..16 |
| CONSUMER_POOL_ACQUIRE_MS | 5000 | 1..60000 |
| CONSUMER_STATEMENT_MS | 5000 | 1..60000 |
| CONSUMER_LOCK_MS | 3000 | 1..60000 |
| RABBITMQ_CONNECT_TIMEOUT_MS / PUBLISH_TIMEOUT_MS | 10000 | 1..300000 |

Operation is a per-phase deadline, not a single total processing duration; shutdown remains an overall absolute budget. Configure database and publish limits coherently with that budget. PostgreSQL roles need the [4.2 restricted grants](milestone-4-2.md). RabbitMQ runtime needs source read and quarantine write; declarations/bindings also need appropriate configure/read/write permissions under the existing relay.* naming scope. Initialization credentials do not update existing volumes.

## Supervisor and pending shared rollout

[Versioned launcher](../../../docker/supervisor/relay-consumer) executes the compiled consumer. [normal.conf](../../../docker/supervisor/normal.conf) adds named consumer with autostart=false, autorestart=unexpected, developer execution, process groups, TERM/30s stop bound and stdout/stderr. HTTP/relay autostart remains unchanged. Compose grace becomes 95s for three sequential 30s children plus margin; queue count is unrelated to program count. **start all and restart all include consumer even with autostart=false: do not use them on shared state.** Existing worker smoke now exercises named HTTP/relay controls.

Rollout is explicitly pending. The running app was not rebuilt/recreated/reloaded, so its manager still knows only HTTP/relay. Its four shared migrations stop at 20261012000000; replay and consumer are pending. Future authorized rollout must:

1. Inspect/backup shared state and apply pending migrations in supported order; confirm consumer schema, constraints, triggers and restricted privileges. Never reset volumes.
2. Review real source contents and logical consumer identity; consuming relay.local will change actual deliveries. Verify source/quarantine topology, policies, capacity, maximum message/frame sizes and effective permissions. No source classic-to-quorum replacement.
3. Build all three binaries and rebuild/recreate only the app when authorized to install the new launcher/config; consumer remains autostart=false. Environment changes require container recreation. Do not invoke start all.
4. Check named status, then explicitly start only `consumer` after authorization. Monitor sanitized outcomes, stock/receipts, quarantine capacity and unacknowledged deliveries. Stop only the named consumer for rollback; business effects/receipts and preserved messages must remain.

## Evidence and exact 4.4 work

[Application tests](../../../backend/tests/consumer_application.rs) cover ordering, validation without persistence, all failed preservation classes, primary/secondary errors, timeout/cancellation and unsupported preservation. Property tests cover binary/type fidelity, float bits and bounds; paused-time tests establish interruptible reconnect pacing. [Real runtime tests](../../../backend/tests/consumer_runtime.rs) use exact UUID broker resources and isolated PostgreSQL: 10→15, duplicate, conflict, malformed/unsupported bytes and forged-header isolation; mandatory return; known DB rejection/unavailability; commit-loss then Duplicate; quarantine-confirm loss with two copies and no early ACK; poisoned source acker and recovery; compiled binary prefetch/blocked-processing TERM/restart; server cancellation/resubscription; idle shutdown; incompatible topology retained; isolated named Supervisor opt-in operation. Cleanup deletes only exact test queues/exchanges and verifies absence; the existing database harness verifies exact database removal. [Dated executed checks](../testing/validation-results.md) distinguish local results from configured CI.

4.4 must add and close the expanded real process-kill matrix: before DB commit; after server commit before source ACK; after quarantine confirm before source ACK; lost source ACK; concurrent consumers/duplicates/distinct events; identity-preserving SQL replay after a completed consumer effect; shutdown/connection loss during preservation and broker cancellation; full/unavailable quarantine capacity/policies and oversize operator recovery. Use independent database/broker observations, exact cleanup, bounded coordination, bilingual acceptance matrix and documented liveness/availability assumptions. Do not infer power-loss durability, production readiness, ordering, fairness or full Milestone 4 closure from these finite schedules.

Primary references checked 2026-10-16: [RabbitMQ confirms and same-channel ACK](https://www.rabbitmq.com/docs/confirms), [positive prefetch and zero meaning unlimited](https://www.rabbitmq.com/docs/consumer-prefetch), [consumer lifecycle](https://www.rabbitmq.com/docs/consumers), [DLX safety/alternatives](https://www.rabbitmq.com/docs/dlx). Installed Lapin 4.12.0 source governs actual client behavior.
