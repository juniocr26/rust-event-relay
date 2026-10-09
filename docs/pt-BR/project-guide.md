[English](../en/project-guide.md) | [README](../../README.pt-BR.md)


## Extensão atual — Marco 2.2

[Estado de entrega e posse](milestone-2-2.md) e [ADR 007](adr/007-durable-delivery-ownership.md) definem recuperação por lease durável e contratos separados de adquirir/concluir/liberar. Migração nova `20261009000000_add_delivery_ownership` adiciona token/acquired_at/expires_at nullable com lease coerente apenas em pending. Seções de marcos anteriores abaixo descrevem escopo original; afirmações antigas de posse/contador indefinidos são substituídas pelo ADR 007. Adapters produtivos de mutação ficam para 2.3; SELECT do reader e publisher preservados.
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
│   ├── infrastructure/postgres/mod.rs
│   └── persistence/ (mod.rs, model.rs, error.rs)
├── tests/ (lifecycle.rs, event_envelope.rs, persistence_contract.rs, postgres_repository.rs, support/, sql/)
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

Serde e serde_json fornecem serialização JSON do envelope; UUID gera IDs v7; Chrono fornece timestamps UTC. Erros usam a biblioteca padrão; não há dependência direta de thiserror ou broker; SQLx fornece driver PostgreSQL. `domain/event.rs` contém comportamento real do envelope, não uma camada vazia. Compose fornece PostgreSQL local com armazenamento físico; migrações SQLx definem namespace relay e tabela outbox; contratos Rust de persistência existem separadamente; adapter PostgreSQL somente leitura implementado; gravações pela aplicação ficam adiadas. Consulte [PostgreSQL](postgresql.md) e [ADR 002](adr/002-use-postgresql-for-durable-event-storage.md). Não são necessários Makefile nem override Compose separado para os comandos atuais.

O nome pretendido do repositório público é `reliable-event-relay`; a pasta local existente pode manter seu nome atual. O pacote Cargo e o título usam o nome pretendido. A descrição GitHub é a primeira frase do README inglês e a descrição Cargo; este scaffold não altera configurações de repositórios remotos.

## Responsabilidades do módulo persistence

- `persistence/mod.rs`: expõe tipos e OutboxReader somente leitura. Chamador fornece corte UTC limitado explícito; resultado não dá propriedade.
- `persistence/model.rs`: validação BatchSize, EligibleRead e PendingOutboxEvent (envelope e metadados de tentativa/disponibilidade). Não espelha todas colunas nem acrescenta infraestrutura ao envelope.
- `persistence/error.rs`: três classificações sem driver, preservação de fonte e Display/Debug sanitizados.
- `tests/persistence_contract.rs`: cinco testes Rust e fake de uma resposta; sem banco, leitura de ambiente ou repositório alternativo de produção.

EventEnvelope é evento canônico; migração outbox é representação SQL durável; contratos definem expectativa de futuros chamadores; adapter PostgreSQL e mapeamentos de largura assinada/linhas/erros pertencem ao Marco 1.6. Sem placeholder vazio de adapter. Configuração/runtime HTTP permanecem independentes. Veja [detalhes](persistence-abstraction.md) e [ADR 005](adr/005-separate-persistence-contracts-from-postgresql.md).

## Repositório PostgreSQL — Marco 1.6 implementado

`src/infrastructure/postgres/` implementa `OutboxReader` com `PgPool` injetado. Infraestrutura depende dos contratos de persistência e domínio; SQL, SQLx, linhas privadas e classificação de erros ficam na infraestrutura. Modelos validam/convertem dados sem consultas. ADR 005 permanece suficiente: sem interface duplicada, camadas vazias, nova migração ou ADR. Futures Send nativas e dispatch estático permanecem. Bootstrap HTTP continua independente do banco.

Veja [consulta, restauração, limites e erros](postgres-repository.md), [testes de integração](testing.md) e [validação executada](validation-results.md). SQLx agora também é dependência da aplicação. Escrita, claims e processamento ficam adiados; Marcos 1.7 e 1.8 concluídos: integração e [semântica de falhas/transações](failure-and-transaction-semantics.md). Marco 1 fechado; entrega futura não iniciada.

Marco 1.7 adiciona job CI dedicado PostgreSQL 18.6 com credenciais descartáveis, mantendo verificações sem banco.

## Handoff do Marco 1

Marco 1 fechado; [Marco 2.1](milestone-2-1.md) agora implementa o recorte publisher. Leia [revisão e critérios](milestone-1-review.md), [semântica de falhas/transações](failure-and-transaction-semantics.md), [fonte Markdown do handoff](handoff-marco-1.md). Base histórica do fechamento: 7cba38d; base atual verificada: 5eac9ff.

Controllers delegam a casos de uso; casos de uso orquestram aplicação/negócio; repositórios controlam consultas e contratos focados; API/mensageria externa fica em adapters; services têm comportamento de negócio reutilizável; helpers têm utilidades genéricas; modelos têm dados, invariantes e conversões próprias. Infraestrutura depende para dentro. Aplicação proporcional: resposta fixa de health não exige camadas vazias de caso de uso/service. Sem classes de encaminhamento nem CRUD genérico.

Para regenerar PDF, instale ReportLab em ambiente Python separado e execute `python scripts/generate_handoff.py`. Fontes são Arial ou DejaVu do sistema (ou `HANDOFF_FONT_DIR`); gerador lê Markdown e grava PDF na raiz. Renderize/inspecione todas as páginas e confira extração textual antes de aceitar nova versão. Dependências Rust não mudam.
