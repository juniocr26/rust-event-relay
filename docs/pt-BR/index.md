# Catálogo de documentação: Rust Event Relay

[README](../../README.pt-BR.md) | [English / Português brasileiro](../en/index.md)

Comece pela [instalação](../../README.pt-BR.md), depois [acompanhe um evento](guides/implementation-study.md), [operação atual](operations/application-scenarios.md), [testes](testing/strategy.md) e a prática de entrevista do guia. Aceite final é [evidência datada](operations/final-audit.md), não execução nova. Páginas de marcos abaixo são históricas; “atual” nos títulos refere-se ao marco registrado.

## adr

- [ADR 001 — Usar Rust para o relay](adr/001-use-rust-for-the-relay.md)
- [ADR 002 — Usar PostgreSQL para armazenamento durável de eventos](adr/002-use-postgresql-for-durable-event-storage.md)
- [ADR 003 — Usar Migrações SQL Versionadas para Evolução do Schema PostgreSQL](adr/003-use-versioned-sql-migrations.md)
- [ADR 004 — Usar Tabela Outbox Transacional PostgreSQL para Handoff Durável de Eventos](adr/004-use-postgresql-transactional-outbox-schema.md)
- [ADR 005 — Separar Contratos de Persistência do Relay da Infraestrutura PostgreSQL](adr/005-separate-persistence-contracts-from-postgresql.md)
- [ADR 006 — Publisher RabbitMQ confirmado e Supervisor local](adr/006-rabbitmq-publisher-and-supervisor.md)
- [ADR 007 — Posse durável da entrega com transações curtas](adr/007-durable-delivery-ownership.md)
- [ADR 008 — Política durável de falhas e contratos de recuperação](adr/008-durable-failure-policy.md)
- [ADR 009 — Replay autorizado, idempotente e auditável](adr/009-authorized-idempotent-replay.md)
- [ADR 010 — Efeito atômico de estoque e quarentena pela aplicação](adr/010-atomic-consumer-and-quarantine.md)
- [ADR 011: execuções locais com evidência persistente](adr/011-persistent-demo-runs.md)
- [ADR 012 — ambiente único e cenários reais limitados](adr/012-one-application-real-scenarios.md)

## architecture

- [Semântica de falhas e transações - Marco 1.8](architecture/failure-and-transactions.md)
- [Marco 2.1 — Publisher RabbitMQ e gestão de processos](architecture/milestone-2-1.md)
- [Marco 2.2 — Estado de entrega e posse de eventos](architecture/milestone-2-2.md)
- [Marco 2.3 — Operações PostgreSQL de posse da entrega](architecture/milestone-2-3.md)
- [Marco 2.4 — Orquestração do caso de uso de entrega](architecture/milestone-2-4.md)
- [Marco 2.5 — Composição e operação do worker](architecture/milestone-2-5.md)
- [Marco 2.6 — Experimentos integrados de recuperação](architecture/milestone-2-6.md)
- [Marco 3.1 — Política durável de falhas e contratos de recuperação](architecture/milestone-3-1.md)
- [Marco 3.2 — Estado durável de retry e transições PostgreSQL](architecture/milestone-3-2.md)
- [Marco 3.3 — Integração da política durável de falhas](architecture/milestone-3-3.md)
- [Marco 3.4 — Descoberta limitada e isolamento seguro](architecture/milestone-3-4.md)
- [Marco 3.5 — Replay autorizado, idempotente e auditável](architecture/milestone-3-5.md)
- [Marco 4.1 — Contratos do consumidor de estoque](architecture/milestone-4-1.md)
- [Marco 4.2 — persistência atômica de estoque no PostgreSQL](architecture/milestone-4-2.md)
- [Marco 4.3 — Consumidor RabbitMQ e quarentena confirmada](architecture/milestone-4-3.md)
- [Arquitetura](architecture/overview.md)
- [Abstração de persistência — Marco 1.5](architecture/persistence-contracts.md)

## database

- [Migrações de banco — Marco 1.3](database/migrations.md)
- [Schema outbox — Marco 1.4](database/outbox-schema.md)
- [Repositório PostgreSQL — Marco 1.6](database/postgres-repository.md)
- [Decisão PostgreSQL e infraestrutura local](database/postgresql.md)

## docker

- [Docker e configuração](docker/configuration.md)
- [Inventário de ambiente — Marco 4.4](docker/environment-inventory.md)

## guides

- [Dependências de desenvolvimento e recuperação](guides/dependencies.md)
- [Preparação frontend/backend — 2026-10-12](guides/frontend-backend.md)
- [Implementação local final: um evento, cinco fronteiras de evidência](guides/implementation-study.md)
- [Guia do projeto](guides/project-guide.md)

## integrations

- [rabbitmq-contract](integrations/rabbitmq-contract.md)

## operations

- [Startup e verificação de token](operations/application-scenarios.md)
- [Manutenção documental — 2026-10-18](operations/completion-checklist.md)
- [Startup e verificação de token](operations/demo-token-connectivity.md)
- [deployment-and-evidence](operations/deployment-and-evidence.md)
- [Auditoria final de implementação - 17/10/2026](operations/final-audit.md)
- [Handoff do Marco 1](operations/handoff-marco-1.md)
- [Rollout local e inicialização automática do relay](operations/local-relay-rollout.md)
- [Revisão de fechamento do Marco 1](operations/milestone-1-review.md)
- [Revisão de aceitação do Marco 2 — 2026-10-12](operations/milestone-2-review.md)
- [Marco 3.6 — Aceitação integrada e auditoria completa do projeto](operations/milestone-3-review.md)
- [Marco 4.4 — verificação completa e fechamento do Marco 4](operations/milestone-4-4-review.md)
- [Marco 4.5 — rollout local controlado e verificação operacional](operations/milestone-4-5-rollout.md)
- [Marco 5.1 — limites de recursos e fundamentos de observabilidade](operations/milestone-5-1.md)
- [Marco 5.2 — prontidão e atualidade operacional](operations/milestone-5-2.md)
- [Marco 5.3 — medições reproduzíveis e aceitação](operations/milestone-5-3.md)
- [Marco 6.1 — entrega real com evidência persistente](operations/milestone-6-1.md)
- [Marco 6.2 — entrega duplicada com evidência durável](operations/milestone-6-2.md)
- [Milestone 6.3 — aceitação final](operations/milestone-6-3.md)
- [Preparação do Marco 6 — comparação e escopo da interface real](operations/milestone-6-preparation.md)
- [Procedimento de replay do operador](operations/replay.md)

## security

- [Autoridade, diagnóstico e transporte](security/diagnostic-and-authority-boundaries.md)

## testing

- [Verificação de rotina](testing/strategy.md)
- [Resultados de validação](testing/validation-results.md)
