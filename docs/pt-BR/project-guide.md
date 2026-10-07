[English](../en/project-guide.md) | [README](../../README.pt-BR.md)

# Guia do projeto

```text
.
├── .github/workflows/ci.yaml
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── config.rs
│   ├── application.rs
│   ├── telemetry.rs
│   ├── domain/event.rs
│   └── persistence/ (mod.rs, model.rs, error.rs)
├── tests/ (lifecycle.rs, event_envelope.rs, persistence_contract.rs, sql/)
├── docs/
│   ├── en/ (architecture, dependencies, Docker, guide, testing, validation, adr/)
│   └── pt-BR/ (documentos equivalentes)
├── docker/entrypoint.sh
├── .cargo-cache/ (gerado, ignorado)
├── target/ (gerado, ignorado)
├── Cargo.toml
├── Cargo.lock
├── .env.example
├── .gitignore
├── .dockerignore
├── Dockerfile
├── compose.yaml
├── README.md
├── README.pt-BR.md
└── LICENSE
```

- `main.rs`: bootstrap e logs do ciclo de vida; `lib.rs`: expõe módulos testáveis.
- `config.rs`: carrega dotenv e valida configurações tipadas; `application.rs`: rota health, ciclo de vida e sinais; `telemetry.rs`: configuração tracing JSON.
- `tests/`: integração com socket real e testes de subprocesso Unix para dotenv/SIGINT/SIGTERM. Testes unitários de configuração ficam junto à implementação.
- `docs/`: orientações equivalentes em inglês/português e ADRs numerados. Inglês é canônico.
- `docker/entrypoint.sh`: cria diretórios físicos de cache/build e executa o comando como usuário sem privilégios de root.
- Manifesto/lock Cargo: dependências declaradas e resolução exata. `Cargo.lock` é versionado para este binário.
- Dockerfile/Compose: ferramentas de desenvolvimento, código montado, portas HTTP/PostgreSQL no loopback do host, bind mount do banco e argumentos UID/GID. `.dockerignore` exclui caches, segredos e metadados locais do build.
- `.env.example`: padrões seguros; copie para `.env` ignorado. `.gitignore` também exclui `.cargo-cache/`, `target/`, artefatos de editor e sistema. Esses diretórios gerados não são código e nunca devem ser versionados.
- CI: formatação, Clippy e testes em pushes/pull requests. A execução no GitHub é separada da validação local.
- READMEs: entradas da documentação; LICENSE: termos da licença MIT.

## Dependências de execução

| Crate | Motivo |
| --- | --- |
| Tokio | Runtime async, listener TCP, sinais; sync/time também apoiam testes de ciclo de vida com tempo limitado |
| Axum | Pequena rota HTTP health e encerramento gracioso do servidor |
| dotenvy | Carregamento local de `.env` com precedência do ambiente do processo |
| tracing | Eventos estruturados do ciclo de vida |
| tracing-subscriber | Formatação JSON e interpretação de filtros |

Serde e serde_json fornecem serialização JSON do envelope; UUID gera IDs v7; Chrono fornece timestamps UTC. Erros usam a biblioteca padrão; não há thiserror, driver Rust de banco ou dependências de broker. `domain/event.rs` contém comportamento real do envelope, não uma camada vazia. Compose fornece PostgreSQL local com armazenamento físico; migrações SQLx definem namespace relay e tabela outbox; contratos Rust de persistência existem separadamente; sem adapter PostgreSQL ou gravações pela aplicação. Consulte [PostgreSQL](postgresql.md) e [ADR 002](adr/002-use-postgresql-for-durable-event-storage.md). Não são necessários Makefile nem override Compose separado para os comandos atuais.

O nome pretendido do repositório público é `reliable-event-relay`; a pasta local existente pode manter seu nome atual. O pacote Cargo e o título usam o nome pretendido. A descrição GitHub é a primeira frase do README inglês e a descrição Cargo; este scaffold não altera configurações de repositórios remotos.

## Responsabilidades do módulo persistence

- `persistence/mod.rs`: expõe tipos e OutboxReader somente leitura. Chamador fornece corte UTC limitado explícito; resultado não dá propriedade.
- `persistence/model.rs`: validação BatchSize, EligibleRead e PendingOutboxEvent (envelope e metadados de tentativa/disponibilidade). Não espelha todas colunas nem acrescenta infraestrutura ao envelope.
- `persistence/error.rs`: três classificações sem driver, preservação de fonte e Display/Debug sanitizados.
- `tests/persistence_contract.rs`: cinco testes Rust e fake de uma resposta; sem banco, leitura de ambiente ou repositório alternativo de produção.

EventEnvelope é evento canônico; migração outbox é representação SQL durável; contratos definem expectativa de futuros chamadores; adapter PostgreSQL e mapeamentos de largura assinada/linhas/erros pertencem ao Marco 1.6. Sem placeholder vazio de adapter. Configuração/runtime HTTP permanecem independentes. Veja [detalhes](persistence-abstraction.md) e [ADR 005](adr/005-separate-persistence-contracts-from-postgresql.md).
