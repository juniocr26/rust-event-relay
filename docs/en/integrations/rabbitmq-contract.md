# Verified publisher integration

[English](rabbitmq-contract.md) | [Português brasileiro](../../pt-BR/integrations/rabbitmq-contract.md)

Static source review: 2026-10-10. Implemented facts, general theory and hypothetical changes are distinguished below. Runtime commands were not executed.

The callable RabbitMQ adapter implements the application-owned `EventPublisher` port. It opens an owned connection/channel, declares a durable direct exchange and durable queue, binds with the configured routing key and enables publisher confirms. Defaults are `relay.events`, `relay.local` and `event`. It serializes the event envelope to JSON, uses stable event identity as message ID, sends persistent messages with mandatory routing and waits for confirmation. A successful Ack without return proves broker acceptance under this contract; it does not prove consumer processing, business effects or an end-to-end relay guarantee. No consumer is implemented.

`RabbitMqConfig` constructs an escaped AMQP URI from separate credential/vhost fields, validates names and deadlines, and redacts Debug. The current configuration accepts `amqp`, not `amqps`; there is no verified production TLS path. Authentication uses broker credentials/vhost, not HTTP application authentication or payload encryption. Local Compose supplies broker storage and a management endpoint; management login is a separate operational boundary from publishing.

The publisher mutex serializes one in-flight publication. Admission waiting and publication waiting have separate deadlines; setup and close are also bounded. Serialization/unavailable, rejected, unroutable and uncertain are distinct outcomes. Timeout/error/cancellation after sending begins can leave broker acceptance unknown; the adapter retires the channel and requires explicit reconnection. It performs no automatic retry. A database lease token is not sent as a broker fencing mechanism and cannot stop a stale process's external send.

The PostgreSQL reader, ownership ports and this publisher are separate library paths. `main` only composes config/tracing/HTTP liveness. There is no producer transaction, polling worker, acquisition SQL, completion SQL, consumer effect or payment integration connecting them. Proposed acquisition → publish → complete remains hypothetical orchestration. A diagram that connects these adapters as a running worker would be inaccurate.
