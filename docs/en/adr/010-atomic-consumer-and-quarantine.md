[Other language](../../pt-BR/adr/010-atomic-consumer-and-quarantine.md) | [Documentation catalog](../index.md)

# ADR 010 — Atomic inventory effect and application-managed quarantine

Status: accepted for later adapters; 4.1 implements contracts only.

Use (consumer_name,event_id) durable deduplication with immutable content comparison, local stock increment and receipt in one PostgreSQL transaction. Select application-managed persistent mandatory quarantine publication with publisher confirms and no return, then source ACK. Preserve the existing classic relay.local.

Broker-managed DLX with at-least-once forwarding is an alternative on a suitably configured quorum source: requires dead-letter-strategy at-least-once, overflow reject-publish, DLX/policy/permissions and supported feature configuration. It retains source messages until target confirmation, but changes source queue requirements. Default classic DLX forwarding does not establish lossless preservation. Never redefine the classic queue as quorum in place or delete/recreate shared resources. A separate quorum consumer queue would require an explicit later topology/rollout decision.

Application-managed preservation fits the existing classic queue, gives explicit bounded diagnostics/original-byte preservation and supports missing identity. It costs a separate publisher, exact binding verification, capacity monitoring and recovery pacing; duplicate quarantine publications are possible after confirmation before ACK or uncertain ACK/confirm. This is at-least-once preservation, not exactly-once quarantine. No purge/replay/TTL/discard automation. Target failure never authorizes source ACK.

[Full contracts, ACK matrix, limits, study and 4.2–4.4 acceptance](../architecture/milestone-4-1.md).

Primary references checked 2026-10-13: [RabbitMQ confirms](https://www.rabbitmq.com/docs/confirms), [DLX](https://www.rabbitmq.com/docs/dlx), [quorum queues](https://www.rabbitmq.com/docs/quorum-queues), [PostgreSQL INSERT](https://www.postgresql.org/docs/18/sql-insert.html). Confirms and consumer acknowledgements are independent; mandatory returns must be checked even with a positive confirm. ON CONFLICT coordination belongs inside the complete local transaction.

2026-10-13: [4.2 implemented database protocol and deferred-trigger rationale](../architecture/milestone-4-2.md). BYTEA representation v1; receipt reservation/completion and stock share one commit. Quarantine/runtime remain 4.3.

2026-10-16: [4.3 implements the opt-in consumer runtime and confirmed quarantine](../architecture/milestone-4-3.md). Shared rollout and full 4.4 crash acceptance remain pending.
