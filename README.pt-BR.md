[English](README.md) | [Português brasileiro](README.pt-BR.md)

# Reliable Event Relay

Um serviço Rust que explora entrega confiável de eventos, tentativas, idempotência, contrapressão, isolamento de mensagens problemáticas e recuperação de falhas em sistemas distribuídos.

Este projeto de código aberto, estudo e portfólio investiga entrega distribuída de eventos sob falhas. Rust é a ferramenta de implementação; o problema de engenharia é a razão do repositório. Inglês é o idioma canônico da documentação; a documentação em português cobre o mesmo escopo.

## Estado atual e escopo

**Implementado hoje:** configuração por ambiente e `.env` opcional, tracing estruturado em JSON, servidor HTTP Axum, `GET /health` retornando `200` e `ok`, encerramento por SIGINT/SIGTERM, testes de configuração e ciclo de vida, envelope canônico validado com UUID v7, timestamps UTC e testes de round-trip JSON, desenvolvimento Docker com infraestrutura PostgreSQL local e migrações SQL versionadas e schema outbox durável inicial e contratos de persistência da aplicação e repositório PostgreSQL somente leitura (sem gravações pela aplicação), testes opt-in isolados de integração PostgreSQL, verificações de CI e documentação bilíngue.

**Planejado / exploração futura:** gravação e processamento outbox, entrega RabbitMQ, tentativas, idempotência, isolamento em dead-letter, pools de workers, concorrência limitada e contrapressão, webhooks HTTP, Redis Streams, readiness, métricas Prometheus e experimentos de falha. A aplicação Rust não persiste nem entrega eventos hoje. O roteiro provisório está em [arquitetura](docs/pt-BR/architecture.md).

## Arquitetura

Um `main.rs` pequeno carrega configuração, configura tracing, abre um socket e executa a aplicação. `application.rs` controla o ciclo de vida HTTP; `config.rs` interpreta configuração; `telemetry.rs` configura logs. `domain/event.rs` define o envelope canônico de eventos; módulos de entrega continuam planejados. SQLx 0.8.6 executa leituras PostgreSQL; dependências de broker ficam adiadas. Compose fornece PostgreSQL local; o binário não se conecta a ele. O módulo persistence expõe snapshots pending limitados e erros classificados para futuros chamadores genéricos; `infrastructure/postgres` implementa OutboxReader com PgPool injetado.

## Desenvolvimento

Pré-requisitos: Docker Engine/Desktop com Compose v2. Desenvolvimento nativo também exige Rust estável com rustfmt e Clippy; Docker e CI usam Rust 1.95.0 como base reproduzível. A imagem de desenvolvimento inclui Bash e Cargo.

```bash
cp .env.example .env
# Linux: ajuste LOCAL_UID e LOCAL_GID no .env conforme id -u e id -g.
docker compose up -d --build
docker compose exec app cargo fetch --locked
docker compose exec app cargo build --locked
docker compose exec app cargo run --locked
```

Em outro terminal: `curl --fail http://localhost:8080/health`. Encerre o serviço em primeiro plano com Ctrl-C. O container de desenvolvimento permanece disponível até `docker compose down`. O código é montado do host; mudanças exigem reiniciar `cargo run` (sem recarga automática).

```bash
docker compose exec app bash
docker compose exec app cargo test --locked
docker compose exec app cargo fmt --check
docker compose exec app cargo clippy --locked --all-targets --all-features -- -D warnings
```

Equivalentes nativos: `cargo run --locked`, `cargo test --locked`, `cargo fmt --check` e `cargo clippy --locked --all-targets --all-features -- -D warnings`.

## Configuração e recuperação

`.env` é ignorado. Variáveis existentes no processo têm precedência sobre `.env`; sua ausência é aceita, mas arquivos malformados impedem a partida. Padrões: `APP_ENV=development`, `RUST_LOG=info`, `HTTP_ADDR=0.0.0.0:8080`. Compose publica apenas no loopback do host; `HOST_HTTP_PORT` muda a porta do host. Mantenha a porta 8080 no `HTTP_ADDR` do container, a menos que também ajuste o mapeamento Compose.

`CARGO_HOME=/app/.cargo-cache` armazena dependências de registry/git; `CARGO_TARGET_DIR=/app/target` armazena artefatos compilados. Ambos são diretórios físicos no host dentro da montagem do código, ignorados pelo Git. O entrypoint os cria, sem volumes nomeados. Restaure downloads com `cargo fetch --locked` e artefatos com `cargo build --locked` no container. Recuperação completa:

```bash
docker compose down
rm -rf .cargo-cache target
docker compose up -d
docker compose exec app cargo fetch --locked
docker compose exec app cargo build --locked
docker compose exec app cargo test --locked
```

## PostgreSQL local

Clientes do host usam `127.0.0.1:5433`; containers usam `postgres:5432`. DBeaver usa POSTGRES_DB/USER/PASSWORD da configuração local, correspondendo às credenciais persistidas. Mudar variáveis de inicialização não atualiza cluster existente. `.dockerized-postgres/` persiste após Compose down. SQLx CLI 0.8.6 fornece migrações explícitas criando namespace relay e relay.outbox_events; gravação pela aplicação e entrega permanecem futuras. Veja [PostgreSQL e reset destrutivo](docs/pt-BR/postgresql.md) e [comandos de migração](docs/pt-BR/database-migrations.md).

## Documentação

- [Arquitetura, escolhas e roteiro](docs/pt-BR/architecture.md)
- [Decisão PostgreSQL e trade-offs](docs/pt-BR/postgresql.md)
- [Migrações de banco](docs/pt-BR/database-migrations.md)
- [Schema outbox](docs/pt-BR/outbox-schema.md)
- [Abstração de persistência](docs/pt-BR/persistence-abstraction.md)
- [Recuperação de dependências](docs/pt-BR/development-dependencies.md)
- [Docker e configuração](docs/pt-BR/docker-and-configuration.md)
- [Guia do projeto e dependências](docs/pt-BR/project-guide.md)
- [Testes](docs/pt-BR/testing.md)
- [Resultados de validação](docs/pt-BR/validation-results.md)
- [ADR 001: Rust](docs/pt-BR/adr/001-use-rust-for-the-relay.md)
- [ADR 002: PostgreSQL](docs/pt-BR/adr/002-use-postgresql-for-durable-event-storage.md)
- [ADR 003: Migrações SQL versionadas](docs/pt-BR/adr/003-use-versioned-sql-migrations.md)
- [ADR 004: Schema outbox transacional](docs/pt-BR/adr/004-use-postgresql-transactional-outbox-schema.md)
- [ADR 005: Fronteira de persistência](docs/pt-BR/adr/005-separate-persistence-contracts-from-postgresql.md)

## Limites e filosofia

`/health` comprova apenas que o HTTP responde, sem readiness de infraestrutura nem garantia de entrega. O encerramento HTTP ainda não tem timeout forçado; requisições prolongadas podem atrasá-lo. Não há autenticação, gravações do produtor, processamento relay, medição de desempenho ou imagem de produção. A futura entrega pelo menos uma vez exige idempotência dos consumidores; nenhuma garantia de exatamente uma vez é alegada. As decisões evoluirão com testes e experimentos de falha documentados. Priorizar semântica clara de falhas, controle de recursos e recuperação acima de complexidade ou afirmações sem medição.

Licença MIT; consulte [LICENSE](LICENSE).

[Marco 1 fechado](docs/pt-BR/milestone-1-review.md): envelope, PostgreSQL, migrações, schema, contratos, repositório, integração e [semântica de falhas/transações](docs/pt-BR/failure-and-transaction-semantics.md). Marco 2 não iniciado. [PDF de handoff](HANDOFF_MARCO_1.pdf) | [Fonte Markdown](docs/pt-BR/handoff-marco-1.md).
