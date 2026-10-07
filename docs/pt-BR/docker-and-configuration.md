[English](../en/docker-and-configuration.md) | [README](../../README.pt-BR.md)

# Docker e configuração

O Dockerfile é uma imagem de ferramentas de desenvolvimento, não de implantação em produção. Fixa Rust estável 1.95.0 sobre Debian Bookworm, inclui Bash, rustfmt e Clippy e executa como `developer`. A tag de versão é fixa; o digest da imagem base não é, portanto revisões upstream ainda podem mudar pacotes de sistema. Compose usa um processo init e montagem do código do host. `sleep infinity` mantém o workspace utilizável antes do download das dependências.

```bash
cp .env.example .env
# No Linux: ajuste no .env os valores LOCAL_UID=$(id -u) e LOCAL_GID=$(id -g).
docker compose up -d --build
docker compose exec app bash
# Dentro do shell:
cargo fetch --locked
cargo build --locked
cargo test --locked
cargo fmt
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo run --locked
```

Use Ctrl-C para parar a aplicação em primeiro plano e `docker compose down` para parar o ambiente. Mudanças no código exigem nova execução; mudanças de UID/GID ou Dockerfile exigem reconstrução da imagem. No Linux, a raiz montada precisa permitir escrita pelo UID/GID configurado. Não execute os primeiros comandos Cargo como root. IDs numéricos padrão são 1000; escolha ID sem privilégios. IDs existentes na imagem base podem impedir a criação de usuário/grupo: escolha IDs compatíveis ou adapte a criação para seu ambiente. Docker Desktop traduz montagens macOS/Windows de maneira diferente; o comportamento de propriedade Linux não foi testado nesses sistemas. No Windows, use um shell compatível com os comandos (por exemplo WSL). Bash está disponível via `docker compose exec app bash`.

| Variável | Padrão | Uso |
| --- | --- | --- |
| APP_ENV | development | Rótulo nos logs de partida; não muda entrega |
| RUST_LOG | info | Filtro tracing; filtros inválidos impedem partida |
| HTTP_ADDR | 0.0.0.0:8080 | IP e porta; endereços inválidos impedem partida |
| HOST_HTTP_PORT | 8080 | Porta loopback do host no Compose |
| LOCAL_UID / LOCAL_GID | 1000 / 1000 | Identidade developer durante build |
| CARGO_HOME | /app/.cargo-cache | Cache de fontes/downloads Cargo no container |
| CARGO_TARGET_DIR | /app/target | Artefatos compilados no container |

As três primeiras são configurações da aplicação. As demais pertencem às ferramentas de desenvolvimento. Não há strings de conexão obrigatórias. Compose fornece DATABASE_URL do PostgreSQL, mas o binário Rust não a consome. URLs de RabbitMQ/Redis, concorrência de workers, tentativas e batches continuam planejados.

O binário carrega `.env` do diretório de trabalho ou ancestrais com dotenvy; variáveis já existentes no processo têm precedência. Compose lê separadamente o `.env` da raiz para interpolação (portas/IDs); não injeta todos os valores no container. A montagem do código expõe `.env` para leitura pelo binário em execução. Para sobrescrever explicitamente: `docker compose exec -e RUST_LOG=debug app cargo run --locked`. A ausência de `.env` é válida; `.env` malformado, APP_ENV vazio, HTTP_ADDR ou RUST_LOG inválidos e sockets ocupados causam falha.

Mantenha `.env` e credenciais de cache fora do Git e do contexto de build Docker. Não coloque segredos em `.env.example`. A porta é publicada apenas no loopback do host; mudar HTTP_ADDR para loopback do container impede acesso pelo host. Mantenha 8080 no container ou ajuste também a porta interna no Compose. `/health` indica apenas liveness, sem verificações de readiness das dependências.

## Reconstruindo o ambiente local

```bash
git clone https://github.com/juniocr26/rust-event-relay.git
cd rust-event-relay
cp .env.example .env
# Linux: alinhe LOCAL_UID e LOCAL_GID antes do build.
docker compose up -d --build --wait --wait-timeout 120
docker compose ps
docker compose logs postgres
docker compose exec postgres pg_isready -h 127.0.0.1 -p 5432 -U relay -d reliable_event_relay
docker compose exec postgres psql -U relay -d reliable_event_relay -c "SELECT 1;"
docker compose exec app cargo fetch --locked
docker compose exec app cargo build --locked
docker compose exec app cargo run --locked
```

Compose cria uma rede do projeto. `app` resolve `postgres` pelo DNS do Docker. PostgreSQL escuta internamente em 5432; apenas o mapeamento no host usa 5433, evitando a porta 5432 do outro projeto. `app` aguarda o healthcheck sem sleeps fixos. O workspace executa `sleep infinity` até você chamar Cargo; a aplicação ainda não exige conexão ao banco. A dependência de partida não é readiness da aplicação em execução.

```mermaid
flowchart LR
    DEV[Desenvolvedor / DBeaver]
    APP[Container Rust]
    DB[(Container PostgreSQL)]
    DISK[.dockerized-postgres/]
    DEV -->|localhost:5433| DB
    APP -.->|postgres:5432 - disponível, ainda sem uso| DB
    DB -->|bind mount| DISK
```

## Configuração do banco e clientes

| Variável | Padrão de desenvolvimento | Uso |
| --- | --- | --- |
| POSTGRES_DB | reliable_event_relay | Banco criado na primeira inicialização |
| POSTGRES_USER | relay | Usuário inicial (a imagem oficial cria um superusuário) |
| POSTGRES_PASSWORD | relay | Senha inicial apenas local |
| POSTGRES_HOST | postgres | Hostname usado na URL do container app |
| POSTGRES_PORT | 5432 | Porta usada na URL; manter o padrão do servidor |
| POSTGRES_HOST_PORT | 5433 | Publicação da porta no loopback do host |
| DATABASE_URL | postgresql://relay:relay@postgres:5432/reliable_event_relay | Configuração de ambiente montada pelo Compose, ainda não consumida pelo Rust |

Compose interpola as seis variáveis POSTGRES do `.env`, com padrões quando ausentes, e injeta explicitamente as variáveis de inicialização em `postgres`. Monta DATABASE_URL para `app`; definir DATABASE_URL no `.env` não substitui esse valor gerado pelo Compose. Mantenha POSTGRES_HOST=postgres e POSTGRES_PORT=5432 nesta topologia. Ao personalizar credenciais com caracteres reservados de URI, codifique sua representação na URL antes de um cliente futuro consumi-la; os padrões locais simples dispensam isso.

No DBeaver, crie uma conexão PostgreSQL:

```text
Tipo de banco: PostgreSQL
Host: localhost
Porta: 5433
Banco: reliable_event_relay
Usuário: relay
Senha: relay
```

São padrões de desenvolvimento, não credenciais de produção. A publicação usa `127.0.0.1`, evitando exposição na LAN; clientes podem usar IPv4 explicitamente se localhost resolver apenas para IPv6. DataGrip, TablePlus ou qualquer cliente compatível com PostgreSQL também funciona. Nenhuma interface gráfica é obrigatória:

```bash
psql -h localhost -p 5433 -U relay -d reliable_event_relay -W
docker compose exec postgres psql -U relay -d reliable_event_relay
```

Clientes no host usam localhost:5433; clientes em containers usam postgres:5432, nunca localhost para o outro container.

## Estado físico e encerramento

| Diretório no host | Conteúdo | Reconstrução |
| --- | --- | --- |
| `.cargo-cache/` | Downloads e cache de fontes Cargo | `cargo fetch --locked` no app |
| `target/` | Artefatos compilados | `cargo build --locked` no app |
| `.dockerized-postgres/` | Cluster PostgreSQL em `18/docker/` | A imagem oficial inicializa um banco de aplicação vazio quando ausente |

Todos são diretórios físicos ignorados com bind mount; não há volumes de dados nomeados. Tornam o estado local gerado visível e intencionalmente reconstruível, mas conteúdos do banco não são recuperados reconstruindo fontes: preserve-os ou faça backup quando necessário. `docker compose down` para/remove containers e rede, **não os conteúdos do banco** ou diretórios Cargo. Recriar containers também preserva dados. Sem migrações, um banco novo tem apenas estruturas internas do PostgreSQL e nenhuma tabela da aplicação.

## Reset apenas do banco local

**Destrutivo: os comandos abaixo apagam permanentemente todos os conteúdos do banco local. Faça backup do que precisar antes.** Cache Cargo e artefatos permanecem intactos:

```bash
docker compose down
rm -rf .dockerized-postgres/
docker compose up -d --wait --wait-timeout 120
```

A imagem inicializa um banco novo. Marcos futuros adicionarão criação reproduzível do schema da aplicação; nada disso existe hoje. Esse reset é documentado, não executado automaticamente.

## Solução de problemas do PostgreSQL

- Consulte `docker compose ps` e `docker compose logs postgres` para erros de inicialização ou health. `pg_isready` não testa autenticação; use também uma consulta SQL autenticada via TCP.
- Host 5433 ocupado impede publicação. Altere POSTGRES_HOST_PORT no `.env` e use essa porta nos clientes do host; mantenha 5432 interna.
- Alterar POSTGRES_DB/USER/PASSWORD afeta apenas a inicialização de um diretório vazio. Credenciais/estado existentes persistem; altere-os deliberadamente via SQL ou faça o reset destrutivo após backup.
- No Linux o entrypoint oficial inicializa a propriedade para seu usuário postgres, separado de LOCAL_UID/GID. Montagens somente leitura ou permissões restritas podem impedir partida. Não torne todos os arquivos do banco graváveis por qualquer usuário. O compartilhamento do Docker Desktop precisa permitir este checkout.
- PG_VERSION incompatível após mudar a versão principal exige upgrade suportado ou backup/restauração, não apagar arquivos para silenciar o erro.
- Compose não inicia app enquanto o banco estiver unhealthy; isso é orquestração de desenvolvimento. O binário Rust e seus testes continuam funcionando independentemente sem PostgreSQL.

Consulte [decisão PostgreSQL](postgresql.md), [testes](testing.md) e [resultados de validação](validation-results.md).
