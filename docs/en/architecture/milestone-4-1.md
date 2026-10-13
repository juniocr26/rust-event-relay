[Other language](../../pt-BR/architecture/milestone-4-1.md) | [Documentation catalog](../index.md)

# Milestone 4.1 — Inventory consumer contracts

Implemented on 2026-10-13: pure validated models, application ports and policy tests. Planned: PostgreSQL adapter/schema, consumer runtime and quarantine topology. Frontend remains illustrative. This is a local inventory demonstration, not a commercial inventory integration or a producer/business outbox transaction.

## Effect and identity

`inventory.units_received`, schema 1, has exactly warehouse_id, sku and quantity. Identifiers (including aggregate type/id and logical consumer name) are 1–128 ASCII bytes, letters/digits/period/underscore/hyphen; no trimming or case folding. Quantity is a JSON integer 1..9223372036854775807, not 5.0, exponent notation, strings or floating point. Stock is nonnegative signed BIGINT, absent row starts at zero. Checked addition rejects overflow and rolls back the whole operation; preserve it for operator review rather than wrap or retry forever.

Use stable logical name `inventory.stock.v1` across replicas. The unique key is (consumer_name,event_id), never delivery tag, process/connection/channel, redelivered flag, acquisition count or replay cycle. Different consumers can apply independent effects. Different event IDs with equal payload increment independently; business-operation identity across different IDs is outside scope. Require nonnil envelope UUID and parseable matching AMQP message_id; missing/malformed/mismatch is preserved, never repaired with a generated ID.

## Immutable checked representation v1

Store and compare the complete validated envelope: ID, type, aggregate type/id, schema version, occurrence instant, correlation/causation IDs and payload. JSON object order and JSON whitespace do not matter. Identifier strings and array order remain exact; timestamps compare UTC instants; absent/null optional IDs both restore None. Supported integer quantity spelling is retained by serde_json arbitrary_precision; decimal/exponent spellings are unsupported. No digest, Debug formatting or string equality is used. Unknown payload keys reject. Unknown top-level envelope keys follow existing envelope restoration (ignored); they are not canonical event content. Transport headers/redelivery information are excluded.

For an existing key, equal checked representation gives Duplicate; changed immutable fields give IdentityConflict. A distinct ID is a new event. Persist representation version 1 and lossless content; the future adapter must not rely on JSONB numeric equality alone, which could erase number spelling. A duplicate returns only the original committed receipt (stock_after at that event), not current stock, processing attempts or a second effect. Receipt lookup/content comparison must establish committed data before returning Duplicate.

[Models](../../../backend/src/domain/consumer.rs), [ports and policy](../../../backend/src/application/consumer.rs), [pure tests](../../../backend/tests/consumer_contract.rs).

## Proposed database operation — 4.2, no migration in 4.1

Separate consumer schema: consumer.receipts with PRIMARY KEY (consumer_name,event_id), representation_version, immutable checked content, warehouse_id, sku, quantity, stock_after and committed processing time; consumer.stock with PRIMARY KEY (warehouse_id,sku), units BIGINT CHECK (units >= 0). Never use relay.outbox_events.processed as a receipt. Retain receipts indefinitely initially; removing receipts can enable old events to apply again.

One explicit transaction at READ COMMITTED: INSERT receipt ownership ON CONFLICT DO NOTHING RETURNING key. The unique index serializes concurrent same-key inserts; a contender waits for owner commit/rollback. If no insertion, a fresh statement reads the committed receipt and compares complete content; absent/unreadable state fails rather than claiming Duplicate. Owner atomically INSERTs missing stock from zero or ON CONFLICT UPDATEs stock with checked positive addition and RETURNING units, then fills the receipt. Proposed receipt has a finalized boolean initially false and nullable result fields during ownership. A DEFERRABLE INITIALLY DEFERRED constraint trigger rereads the current receipt at commit and requires finalized=true, stock_after nonnull/nonnegative and processing time nonnull. The stock update and receipt finalization happen before COMMIT; a committed placeholder is forbidden. The consumer role cannot delete receipts; all adapter writes use this one transaction. Test the deferred trigger and permissions in 4.2 before shipping.

Different events on the same stock key serialize through the row update, avoiding lost updates. Overflow, known failure between writes or transaction abort commits neither receipt nor effect. Process termination releases uncommitted work; dropping a Rust future alone does not prove abort, especially during COMMIT. Lost commit response is CommitUncertain; recover by delivering the same identity and comparing durable content in the atomic operation. Do not independently compensate/increment or infer rollback from timeout. This transaction covers only local stock/receipt, not HTTP, email or notifications.

## Manual acknowledgement matrix

| Evidence | Action |
| --- | --- |
| Applied with acknowledged commit | ACK original receiving channel/tag, single delivery |
| Verified identical Duplicate | ACK, return historical receipt |
| Known pre-commit transient failure | Keep recovery possible, pause admission, bounded backoff/reconnect; no immediate unlimited nack/requeue |
| CommitUncertain or cancellation during commit | No ACK; recover same identity via durable operation, without assuming rollback |
| IdentityConflict / invalid / unsupported / stock overflow | Preserve through the quarantine protocol below before source ACK |
| Quarantine confirmed and routed | ACK original once; failed/uncertain ACK can redeliver |
| Quarantine return / nack / unavailable / timeout / cancellation | No source ACK; pace recovery and retire uncertain publication channel |

Successful effect followed by failed ACK can redeliver; durable deduplication prevents another increment. ACK only applies to its receiving channel/tag. No batch ACK may accidentally retire an unpreserved message. Cancellation before admission can prove no call began; after operation/send it proves neither rollback nor preservation. Drain bounded in-flight work at shutdown, then close channel for recovery; no runtime is implemented here.

## Selected application-managed quarantine

Preserve existing classic `relay.local`; do not redefine it as quorum, delete/recreate it, attach destructive policies or consume shared messages for tests. Future demo consumer uses this existing queue explicitly; another logical responsibility later needs its own bound queue because consumers on one queue compete. Future durable direct `relay.quarantine` exchange, durable classic `relay.inventory.quarantine` queue, exclusive binding key `inventory.stock.v1`; publication must be persistent, mandatory and publisher-confirmed with no basic.return for that attempt. Provision and verify the exact binding/target before consumption; mandatory success only proves routing to some queue unless this binding invariant is established. No topology created in 4.1.

Publish unchanged body bytes with bounded diagnostic reason from QuarantineReason, plus original properties in a versioned lossless type-tagged JSON sidecar (AMQP property names and field-table wire type tags, binary values base64, nested values typed, message_id presence preserved). New publication properties use persistent delivery mode and preservation format version 1; sidecar stores original delivery mode/expiration without applying its TTL to quarantine. Broker delivery channel/tag are recovery context, never event IDs. Do not serialize exceptions, credentials or payload excerpts as diagnostics or log raw preserved messages. The sidecar encoding and Lapin conversion are mandatory 4.3 adapter acceptance work; the pure model checks object shape/size, not lossless AMQP encoding.

Model bounds: body 1 MiB, properties serialized UTF-8 JSON 16 KiB, message_id 255 bytes, closed diagnostic enum. No truncation. Reject preservation construction for oversize; retain source unacknowledged, pause consumption and alert operator for explicit external preservation protocol. Align source admission/prefetch, broker maximum message size, memory and quarantine capacity before rollout; existing producer payloads have no new limit imposed by 4.1. No original ACK if exact preservation cannot complete.

Publisher success precedes source ACK. Crash after quarantine confirmation but before ACK, or uncertain source ACK, can produce two identical quarantined records. Lost confirm may also lead to a second copy after recovery. RabbitMQ publisher confirms and source ACK are not one transaction. Keep both copies as evidence; logical grouping by valid consumer/event ID is informational, not deletion authority. Missing/malformed IDs cannot safely deduplicate by event; never fabricate one. Pure fake test explicitly republishes bytes twice; actual crash/broker proof remains 4.4.

Target missing/unavailable/full, return, nack or unknown confirm: retain source, pause admission with bounded recovery pacing; no requeue=false assumption. Future app role needs read on source, write on quarantine exchange; provisioner needs configure on the new relay.* resources and binding permissions. Current initial development permissions match relay.*; inspect effective existing role permissions/policies in 4.3 rather than assume initialization re-applies them.

No automatic purge, replay, TTL expiration, queue expiry or drop-head limits. If setting capacity limits later, use reject-publish with alert/pacing and verify existing policies, never silent discard. No automatic deletion does not solve disk growth: operators monitor bytes/disk alarms, capacity and backup/recovery, then explicitly authorize audited removal/replay. Quarantine replay/removal needs a separate protocol, preserving identity/content and recording audit; malformed identity/content repair is separate. The SQL replay API does not control broker messages and cannot undo a prior stock effect.

## Remaining slices and acceptance

| Slice | Deliverables and required evidence |
| --- | --- |
| 4.2 | Final consumer schema/constraints and focused atomic adapter; lossless versioned comparison storage, stock/receipt rollback and classified commit errors. Real isolated PostgreSQL tests: concurrent same-key copies, changed content conflict, distinct events on same stock, overflow, failure between writes, process termination and lost COMMIT response with independent durable observation. |
| 4.3 | Owned RabbitMQ runtime, manual single ACK, bounded prefetch/admission/shutdown, exact durable topology/permissions verification, typed properties sidecar encoder/decoder, persistent mandatory confirms/returns, resource limits and paced failures. Isolated broker tests for target missing/unavailable, invalid identity/content preservation and no early ACK. No shared topology mutation without separate rollout scope. |
| 4.4 | Real integrated concurrent duplicates, failure before commit, crash after commit before ACK, distinct concurrent events, CommitUncertain, invalid preservation, crash after quarantine confirm before ACK, lost source ACK, replay with already processed ID. Independent database/broker observations and exact fixture cleanup; bilingual closure and remaining limits. |

Exact 4.2 prerequisites: approve/finalize representation v1 persistence (retain numeric distinctions), deferred receipt-completeness trigger and schema privileges, define atomic error mapping including deterministic overflow, retain dedup receipts, reuse isolated CREATE DATABASE harness and fault proxy. No new shared migration is a prerequisite for isolated validation.

## Study example: 10 + 5 = 15

Implemented relay: canonical event is stored in relay.outbox_events. Publication means broker acceptance, not inventory change. Relay processed records publication completion only. Planned RabbitMQ manual delivery reaches inventory.stock.v1, which atomically stores its receipt and adds five to consumer.stock: 10 + 5 = 15. A second delivery of event A finds the same receipt/content: stock stays 15. Different event B adds five: 20. A lost ACK changes deliveries, not units. Distinct concurrent events serialize on stock.

SQL dead_letter preserves relay failures before publication completion; consumer quarantine preserves messages the consumer cannot apply safely. SQL replay opens another relay attempt cycle while preserving event identity/content; it cannot undo stock or make an already processed ID add five again. Broker quarantine replay is planned separately. Read this alongside the existing [guided study](../guides/project-guide.md) and [Milestone 3 review](../operations/milestone-3-review.md). Contracts/pure tests are implemented; all database effects and runtime scenarios in this example are planned.
