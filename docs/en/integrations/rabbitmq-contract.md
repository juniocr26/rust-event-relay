# Current RabbitMQ contract

The relay publishes original stable-ID envelopes to durable direct `relay.events`, routing key `event`, source queue `relay.local`, vhost `relay`. Mandatory routing/returns and publisher confirms classify acceptance, known rejection/unroutability and uncertainty. Database leases protect state transitions, not external sends. Reconnection is not proof of prior nonacceptance; retry can duplicate transport.

The inventory consumer is implemented with manual ACK after its atomic stock/receipt transaction. Invalid/unsupported/conflicting input requires confirmed application-managed quarantine before source ACK; preservation failure retains source. Defaults are `relay.quarantine`, key `inventory`, queue `relay.quarantine.inventory`. Duplicate actions use their own durable `relay.demo.<action UUID>` queue/key and bounded temporary consumers. SQL terminal dead_letter differs from broker quarantine. Broker credentials are separate from demo/operator tokens; current AMQP is not a verified production TLS or payload-encryption path.

[Complete event/ACK/uncertainty trace](../guides/implementation-study.md). Earlier adapter-only and missing-consumer statements below retain their original milestone scope, not the current contract.

## Historical reference (original dates and milestone scope)

Current resource admission and instrumentation: publisher/quarantine reject saturation immediately before serialization; no internal admission waiters. Earlier dated admission descriptions below are historical. [Milestone 5.1](../operations/milestone-5-1.md).

#
Current worker behavior, configuration, cancellation and authorized rollout: [Milestone 2.5](../architecture/milestone-2-5.md). Earlier sections retain their historical scope.

 Verified publisher integration

## Milestone 2.4 — 2026-10-12

The callable delivery use case now performs one acknowledged acquisition → original envelope publication → completion only after confirmed acceptance. Definitive publication failures release once; uncertainty retains the lease. Results preserve primary and cleanup failures separately. HTTP remains independent; no worker is composed. See [Milestone 2.4](../architecture/milestone-2-4.md) for outcomes, cancellation and recovery limitations. Next: 2.5 worker composition/operation; 2.6 end-to-end failure experiments.

Earlier sections retain their recorded milestone/review scope.

[English](rabbitmq-contract.md) | [Português brasileiro](../../pt-BR/integrations/rabbitmq-contract.md)

Static source review: 2026-10-10. Implemented facts, general theory and hypothetical changes are distinguished below. Runtime commands were not executed.

The callable RabbitMQ adapter implements the application-owned `EventPublisher` port. It opens an owned connection/channel, declares a durable direct exchange and durable queue, binds with the configured routing key and enables publisher confirms. Defaults are `relay.events`, `relay.local` and `event`. It serializes the event envelope to JSON, uses stable event identity as message ID, sends persistent messages with mandatory routing and waits for confirmation. A successful Ack without return proves broker acceptance under this contract; it does not prove consumer processing, business effects or an end-to-end relay guarantee. No consumer is implemented.

`RabbitMqConfig` constructs an escaped AMQP URI from separate credential/vhost fields, validates names and deadlines, and redacts Debug. The current configuration accepts `amqp`, not `amqps`; there is no verified production TLS path. Authentication uses broker credentials/vhost, not HTTP application authentication or payload encryption. Local Compose supplies broker storage and a management endpoint; management login is a separate operational boundary from publishing.

The publisher mutex serializes one in-flight publication. Admission waiting and publication waiting have separate deadlines; setup and close are also bounded. Serialization/unavailable, rejected, unroutable and uncertain are distinct outcomes. Timeout/error/cancellation after sending begins can leave broker acceptance unknown; the adapter retires the channel and requires explicit reconnection. It performs no automatic retry. A database lease token is not sent as a broker fencing mechanism and cannot stop a stale process's external send.

The PostgreSQL reader, ownership ports and this publisher are separate library paths. `main` only composes config/tracing/HTTP liveness. There is no producer transaction, polling worker, acquisition SQL, completion SQL, consumer effect or payment integration connecting them. Proposed acquisition → publish → complete remains hypothetical orchestration. A diagram that connects these adapters as a running worker would be inaccurate.

## Milestone 3.3 — current integration

Application delivery/worker integration is implemented; earlier sections retain their dated historical scope. [Current contracts, jitter, contextual outcome table, maintenance and cancellation limits](../architecture/milestone-3-3.md). General poison scanning remains 3.4, audited replay 3.5 and full acceptance 3.6; shared rollout remains pending.
