[Português brasileiro](../../pt-BR/adr/006-rabbitmq-publisher-and-supervisor.md)

# ADR 006 — Confirmed RabbitMQ publisher and local Supervisor

Accepted — 2026-10-08, Milestone 2.1.

The relay has a read-only outbox observation port, with no ownership or transition authority. A focused destination adapter can establish publication semantics without inventing a polling worker. Local browser management and child process control are explicit development requirements.

Use an application-facing native Send-future `EventPublisher`, implemented by an infrastructure-owned Lapin 4.12.0 Tokio connection/channel. Declare a minimal durable direct exchange/queue/binding in infrastructure configuration, publish the unchanged JSON envelope persistently with stable message identity, require mandatory routing and confirms, and reject ack-with-return. Serialize in-flight sends to one per adapter. Bound connection/setup/admission/send-confirm/close waits, preserve sanitized errors with typed sources, retire the adapter after uncertain outcomes or post-send cancellation, and disable automatic retries/recovery. Explicit reconnect cannot reconcile a previous unknown acceptance.

Use pinned RabbitMQ 4.3.6 management as a separate Compose service, persistent volume, loopback management HTTP on 15672, project vhost and management-only user with `relay.*` permissions. Real secrets live only in ignored `.env`. Initialization variables do not reconcile existing volume credentials. No host AMQP publication is needed for container tests. TLS/HA remain separate production decisions.

Run foreground Supervisor under Compose init as developer, with repository config installed at its standard discovery path, private Unix socket and one program `http` executing the real compiled binary. Provide `supervisor` executable forwarding to supervisorctl. Explicit stop keeps the manager alive; unexpected nonzero child exits restart. SIGTERM drains with a 30s child budget and 35s Compose budget; logs stream without child rotation.

Alternatives: direct AMQP types in application couple business orchestration to transport; fire-and-forget cannot establish acceptance; automatic retry after loss can duplicate; a pool/reconnection framework adds speculative ownership. `cargo run` as child obscures binary lifecycle; a fake worker misrepresents scope; supervising databases inside app duplicates Compose responsibilities. Serialized sends trade throughput for bounded correlation and a simpler cancellation boundary. Declaring topology requires configure privileges; future deployments may preprovision it with stricter roles. A single local queue/broker is educational infrastructure, not production HA or exactly-once processing. Outbox claims, state updates, retries and idempotency remain prerequisites for later delivery guarantees.

See [implementation, official sources and validation](../milestone-2-1.md).
