[English](README.md) | [Português brasileiro](README.pt-BR.md)

# Reliable Event Relay

[Preparação frontend/backend — estrutura e comandos atuais](docs/pt-BR/guides/frontend-backend.md). Comandos Cargo nativos rodam em `backend/`; comandos frontend em `frontend/`.


[Marco 2.6 — Recuperação integrada](docs/pt-BR/architecture/milestone-2-6.md): experimentos reais de crash, incerteza, concorrência e shutdown concluídos. [Marco 2 fechado sob premissas explícitas](docs/pt-BR/operations/milestone-2-review.md). Migrations locais de ownership/retry instaladas; http e relay com autostart. [Rollout atual](docs/pt-BR/operations/local-relay-rollout.md).

Um serviço Rust que explora entrega confiável de eventos, tentativas, idempotência, contrapressão, isolamento de mensagens problemáticas e recuperação de falhas em sistemas distribuídos.

Este projeto de código aberto, estudo e portfólio investiga entrega distribuída de eventos sob falhas. Rust é a ferramenta de implementação; o problema de engenharia é a razão do repositório. Inglês é o idioma canônico da documentação; a documentação em português cobre o mesmo escopo.

## Catálogo de documentação e percurso de leitura

- [Catálogo completo por categoria](docs/pt-BR/index.md)
- [Checklist da revisão e lacunas](docs/pt-BR/operations/completion-checklist.md)

Leia propósito/setup, arquitetura e contratos, segurança/falhas, testes/evidência e implantação/recuperação local. Este projeto é independente no portfólio; procedimentos runtime documentados não foram executados na revisão de 2026-10-10.

## Estado atual e escopo

**Implementado hoje:** worker relay sequencial com pacing/reconexão explícitos e shutdown limitado, configuração por ambiente e `.env` opcional, tracing estruturado em JSON, servidor HTTP Axum, `GET /health` retornando `200` e `ok`, encerramento por SIGINT/SIGTERM, testes de configuração e ciclo de vida, envelope canônico validado com UUID v7, timestamps UTC e testes de round-trip JSON, desenvolvimento Docker com infraestrutura PostgreSQL local e migrações SQL versionadas e schema outbox durável inicial e contratos de persistência da aplicação e repositório PostgreSQL de observação e operações produtivas de posse, testes opt-in isolados de integração PostgreSQL, adapter publisher RabbitMQ confirmado, management local do broker e gestão Supervisor, tipos validados de estado/posse e contratos focados de mutação, migração de lease durável (sem aplicação automática), testes reais de concorrência/expiração de posse, verificações de CI e documentação bilíngue.

**Planejado / exploração futura:** gravações de produtor, orquestração de retries, idempotência, isolamento em dead-letter, pools de workers, concorrência limitada e contrapressão, webhooks HTTP, Redis Streams, readiness, métricas Prometheus e experimentos de falha. O binário HTTP não executa worker outbox; o publisher é chamável separadamente. O roteiro provisório está em [arquitetura](docs/pt-BR/architecture/overview.md).

[Marco 2.2](docs/pt-BR/architecture/milestone-2-2.md) implementa decisão de posse, modelos, contratos e schema. Eventos com lease permanecem pending; donos expirados não podem concluir/liberar. Tokens protegem transições no banco, mas não impedem publicação nem estabelecem entrega ponta a ponta.

[Marco 2.3](docs/pt-BR/architecture/milestone-2-3.md) implementa operações PostgreSQL com transações curtas e relógio pós-lock. Migrations locais de ownership/retry instaladas explicitamente em 2026-10-13. Relatório de continuidade (relatório 2.5 ausente neste checkout); handoff atual (historical handoff unavailable in this checkout). 2.6 valida uma fundação pelo menos uma vez sob premissas explícitas, sem efeitos exactly-once.

## Arquitetura

A composição separada `relay` constrói pool/repositório PostgreSQL e lifecycle RabbitMQ; scheduling admite uma entrega por vez com pacing/reconexão explícitos e shutdown limitado. Veja [operação do worker](docs/pt-BR/architecture/milestone-2-5.md).

Um `main.rs` pequeno carrega configuração, configura tracing, abre um socket e executa a aplicação. `application.rs` controla o ciclo de vida HTTP; `config.rs` interpreta configuração; `telemetry.rs` configura logs. `domain/event.rs` define o envelope canônico de eventos; o contrato publisher da aplicação é implementado pelo adapter RabbitMQ Lapin 4.12.0 da infraestrutura. SQLx 0.8.6 executa leituras e transações de posse PostgreSQL. Compose fornece PostgreSQL e RabbitMQ separados; o binário HTTP não se conecta a nenhum deles. O módulo persistence expõe snapshots pending limitados e erros classificados para futuros chamadores genéricos; `infrastructure/postgres` implementa OutboxReader e ports de aquisição/conclusão/liberação com PgPool injetado.

## Desenvolvimento

Pré-requisitos: Docker Engine/Desktop com Compose v2. Desenvolvimento nativo também exige Rust estável com rustfmt e Clippy; Docker e CI usam Rust 1.95.0 como base reproduzível. A imagem de desenvolvimento inclui Bash e Cargo.

```bash
cp .env.example .env
# Linux: ajuste LOCAL_UID e LOCAL_GID no .env conforme id -u e id -g.
# Defina credenciais RabbitMQ no .env antes de iniciar.
docker compose build app
docker compose run --rm --no-deps app cargo build --locked --bins
docker compose up -d --wait postgres rabbitmq
# Inspecione SQL das migrations e dados atuais antes desta ação explícita:
docker compose run --rm --no-deps app sqlx migrate run
python3 scripts/start-local.py
```

Em outro terminal: `curl --fail http://localhost:8080/health`. Supervisor inicia `http` e `relay` automaticamente; ambos mantêm controle manual. Abra **http://localhost:15672/** e entre com `RABBITMQ_DEFAULT_USER` / `RABBITMQ_DEFAULT_PASS` do `.env` ignorado. Veja [Marco 2.1](docs/pt-BR/architecture/milestone-2-1.md) para comandos coletivos/individuais/interativos, build/start e credenciais em volume existente. Mudanças exigem parar, compilar e iniciar; sem recarga automática.

```bash
docker compose exec app bash
docker compose exec app cargo test --locked
docker compose exec app cargo fmt --check
docker compose exec app cargo clippy --locked --all-targets --all-features -- -D warnings
```

Equivalentes nativos (em `backend/`): `cargo run --locked`, `cargo test --locked`, `cargo fmt --check` e `cargo clippy --locked --all-targets --all-features -- -D warnings`.

## Configuração e recuperação

`.env` é ignorado. Variáveis existentes no processo têm precedência sobre `.env`; o binário HTTP aceita sua ausência, enquanto Compose exige credenciais do broker; arquivos malformados impedem a partida do binário. Padrões: `APP_ENV=development`, `RUST_LOG=info`, `HTTP_ADDR=0.0.0.0:8080`. Compose publica apenas no loopback do host; `HOST_HTTP_PORT` muda a porta do host. Mantenha a porta 8080 no `HTTP_ADDR` do container, a menos que também ajuste o mapeamento Compose.

`CARGO_HOME=/app/.cargo-cache` armazena dependências de registry/git; `CARGO_TARGET_DIR=/app/target` armazena artefatos compilados. Ambos são diretórios físicos no host dentro da montagem do código, ignorados pelo Git. O entrypoint os cria, sem volumes Cargo nomeados; RabbitMQ usa volume persistente nomeado separado. Restaure downloads com `cargo fetch --locked` e artefatos com `cargo build --locked` no container. Recuperação completa:

```bash
docker compose down
rm -rf .cargo-cache target
docker compose run --rm --no-deps app cargo build --locked
python3 scripts/start-local.py
docker compose exec app cargo test --locked
```

## PostgreSQL local

Clientes do host usam `127.0.0.1:5433`; containers usam `postgres:5432`. DBeaver usa POSTGRES_DB/USER/PASSWORD da configuração local, correspondendo às credenciais persistidas. Mudar variáveis de inicialização não atualiza cluster existente. `.dockerized-postgres/` persiste após Compose down. SQLx CLI 0.8.6 fornece migrações explícitas criando namespace relay e relay.outbox_events; gravação pela aplicação permanece futura; entrega relay exige a migração de posse. Veja [PostgreSQL e reset destrutivo](docs/pt-BR/database/postgresql.md) e [comandos de migração](docs/pt-BR/database/migrations.md).

## RabbitMQ e Supervisor

```bash
docker compose exec app bash
supervisorctl status
supervisorctl stop all
# Após rollout de schema autorizado:
supervisorctl stop http
supervisorctl start http
supervisorctl restart http
# Após rollout de schema autorizado:
# supervisorctl start all
supervisorctl start relay
supervisorctl stop relay
supervisorctl restart relay
```

Os mesmos argumentos funcionam com `supervisor`; execute qualquer comando sem argumentos para console interativo. `all` controla só programas do app, `http` e `relay` (start all inicia relay; confira o pré-requisito de schema). Login e inicialização estão no [Marco 2.1](docs/pt-BR/architecture/milestone-2-1.md).

## Documentação

- [Arquitetura, escolhas e roteiro](docs/pt-BR/architecture/overview.md)
- [Decisão PostgreSQL e trade-offs](docs/pt-BR/database/postgresql.md)
- [Migrações de banco](docs/pt-BR/database/migrations.md)
- [Schema outbox](docs/pt-BR/database/outbox-schema.md)
- [Abstração de persistência](docs/pt-BR/architecture/persistence-contracts.md)
- [Recuperação de dependências](docs/pt-BR/guides/dependencies.md)
- [Docker e configuração](docs/pt-BR/docker/configuration.md)
- [Guia do projeto e dependências](docs/pt-BR/guides/project-guide.md)
- [Testes](docs/pt-BR/testing/strategy.md)
- [Resultados de validação](docs/pt-BR/testing/validation-results.md)
- [ADR 001: Rust](docs/pt-BR/adr/001-use-rust-for-the-relay.md)
- [ADR 002: PostgreSQL](docs/pt-BR/adr/002-use-postgresql-for-durable-event-storage.md)
- [ADR 003: Migrações SQL versionadas](docs/pt-BR/adr/003-use-versioned-sql-migrations.md)
- [ADR 004: Schema outbox transacional](docs/pt-BR/adr/004-use-postgresql-transactional-outbox-schema.md)
- [ADR 005: Fronteira de persistência](docs/pt-BR/adr/005-separate-persistence-contracts-from-postgresql.md)
- [ADR 006: Publisher RabbitMQ e Supervisor](docs/pt-BR/adr/006-rabbitmq-publisher-and-supervisor.md)
- [ADR 007: Ownership durável](docs/pt-BR/adr/007-durable-delivery-ownership.md)
- [Marco 2.2: Tipos, contratos e schema de ownership](docs/pt-BR/architecture/milestone-2-2.md)

## Limites e filosofia

`/health` comprova apenas que o HTTP responde, sem readiness de infraestrutura nem garantia de entrega. O binário encerra HTTP graciosamente; Supervisor limita parada do filho a 30 segundos. Não há autenticação na aplicação HTTP, gravações do produtor, medição de desempenho ou imagem de produção. A futura entrega pelo menos uma vez exige idempotência dos consumidores; nenhuma garantia de exatamente uma vez é alegada. As decisões evoluirão com testes e experimentos de falha documentados. Priorizar semântica clara de falhas, controle de recursos e recuperação acima de complexidade ou afirmações sem medição.

Licença MIT; consulte [LICENSE](LICENSE).

[Marco 1 fechado](docs/pt-BR/operations/milestone-1-review.md): envelope, PostgreSQL, migrações, schema, contratos, repositório, integração e [semântica de falhas/transações](docs/pt-BR/architecture/failure-and-transactions.md). [Marco 2.1](docs/pt-BR/architecture/milestone-2-1.md) implementa publisher; [Marco 2.2](docs/pt-BR/architecture/milestone-2-2.md) implementa contratos de estado/posse, com orquestração implementada em [2.4](docs/pt-BR/architecture/milestone-2-4.md). [Fonte do handoff histórico](docs/pt-BR/operations/handoff-marco-1.md).

## Marco 3.2 — persistência atual

Schema durável de retry, agenda/preservação terminal protegidas por dono/ciclo/revisão, orçamento de aquisição e manutenção exata de esgotados estão implementados. Integração implementada em 3.3; scanner geral fica em 3.4, replay em 3.5 e fechamento integrado em 3.6. Seções anteriores preservam contexto histórico de cada marco. [Implementação e rollout](docs/pt-BR/architecture/milestone-3-2.md).

[Marco 3.3 — integração atual](docs/pt-BR/architecture/milestone-3-3.md).

[Startup local, pré-requisitos de migrations e controles Supervisor](docs/pt-BR/operations/local-relay-rollout.md).


## Marco 3.4 — descoberta/isolamento atuais

Descoberta limitada e isolamento com autoridade atual implementados; replay permanece 3.5 e fechamento integrado 3.6. Rollout local preservado; esta alteração não tem migration ou reinício compartilhado. [Marco 3.4 — descoberta/isolamento atuais](docs/pt-BR/architecture/milestone-3-4.md).

[Marco 3.5 — replay autorizado](docs/pt-BR/architecture/milestone-3-5.md) | [API operacional](docs/pt-BR/operations/replay.md). Fonte implementada; migration 5 e ativação da API privilegiada continuam pendentes no ambiente compartilhado.
