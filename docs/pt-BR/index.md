# Catálogo de documentação: rust-event-relay

[Introdução do projeto](../../README.pt-BR.md) | [Outro idioma](../en/index.md)

Percurso: propósito/setup → arquitetura/contratos → segurança/falhas → testes/evidência → operações. Na biblioteca: catálogo/dossiê → entrevista por projeto → fundamentos → falhas. Marcos/refactors/handoff históricos descrevem datas originais; revisão/checklist explica escopo atual.

## architecture

Componentes, fluxos e fronteiras implementadas.

- [Semântica de falhas e transações - Marco 1.8](architecture/failure-and-transactions.md)
- [Marco 2.1 — Publisher RabbitMQ e gestão de processos](architecture/milestone-2-1.md)
- [Marco 2.2 — Estado de entrega e posse de eventos](architecture/milestone-2-2.md)
- [Arquitetura](architecture/overview.md)
- [Abstração de persistência — Marco 1.5](architecture/persistence-contracts.md)

## adr

Decisões e consequências; IDs/histórico preservados.

- [ADR 001 — Usar Rust para o relay](adr/001-use-rust-for-the-relay.md)
- [ADR 002 — Usar PostgreSQL para armazenamento durável de eventos](adr/002-use-postgresql-for-durable-event-storage.md)
- [ADR 003 — Usar Migrações SQL Versionadas para Evolução do Schema PostgreSQL](adr/003-use-versioned-sql-migrations.md)
- [ADR 004 — Usar Tabela Outbox Transacional PostgreSQL para Handoff Durável de Eventos](adr/004-use-postgresql-transactional-outbox-schema.md)
- [ADR 005 — Separar Contratos de Persistência do Relay da Infraestrutura PostgreSQL](adr/005-separate-persistence-contracts-from-postgresql.md)
- [ADR 006 — Publisher RabbitMQ confirmado e Supervisor local](adr/006-rabbitmq-publisher-and-supervisor.md)
- [ADR 007 — Posse durável da entrega com transações curtas](adr/007-durable-delivery-ownership.md)

## guides

Setup, pré-requisitos e procedimentos de leitura.

- [Dependências de desenvolvimento e recuperação](guides/dependencies.md)
- [Guia do projeto](guides/project-guide.md)

## testing

Estratégia, inventários e evidências datadas.

- [Testes](testing/strategy.md)
- [Resultados de validação](testing/validation-results.md)

## docker

Imagens, serviços, mounts e configuração local.

- [Docker e configuração](docker/configuration.md)

## database

Modelos, constraints, migrações e consistência.

- [Migrações de banco — Marco 1.3](database/migrations.md)
- [Schema outbox — Marco 1.4](database/outbox-schema.md)
- [Repositório PostgreSQL — Marco 1.6](database/postgres-repository.md)
- [Decisão PostgreSQL e infraestrutura local](database/postgresql.md)

## integrations

Fronteiras verificadas e ausência de integrações.

- [Integração verificada do publisher](integrations/rabbitmq-contract.md)

## operations

Diagnóstico, recuperação, checklists e registros históricos.

- [rust-event-relay: Checklist de conclusão do repositório](operations/completion-checklist.md)
- [Implantação, observabilidade e lacunas de evidência](operations/deployment-and-evidence.md)
- [Handoff do Marco 1](operations/handoff-marco-1.md)
- [Revisão de fechamento do Marco 1](operations/milestone-1-review.md)

## security

Autenticação, autorização, dados e recursos.

- [Autoridade, diagnóstico e transporte](security/diagnostic-and-authority-boundaries.md)

Aplicabilidade e omissões estão justificadas no [checklist](operations/completion-checklist.md). Não há categorias vazias.
