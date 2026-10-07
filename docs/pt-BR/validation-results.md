[English](../en/validation-results.md) | [README](../../README.pt-BR.md)

# Resultados de validação

## Marco 1.2 — 2026-10-07

Ambiente: macOS com Docker Desktop; imagem Rust de desenvolvimento existente reconstruída e imagem oficial PostgreSQL baixada. Nenhuma fonte Rust, dependência Cargo, migração ou persistência da aplicação foi adicionada.

| Validação | Resultado observado |
| --- | --- |
| Build/partida Compose com espera limitada por readiness | Aprovados; app em execução e PostgreSQL healthy |
| Configuração Compose e logs PostgreSQL | Aprovados; servidor pronto para aceitar conexões |
| pg_isready | Aprovado; 127.0.0.1:5432 aceitando conexões |
| SQL TCP autenticado por senha via DNS do serviço postgres | Aprovado; SELECT 1 retornou 1 |
| Servidor selecionado | 18.6 (Debian 18.6-1.pgdg12+2) |
| Diretório de dados / montagem | /var/lib/postgresql/18/docker; um bind mount do host .dockerized-postgres em /var/lib/postgresql, sem volume de dados |
| Porta publicada no host | 127.0.0.1:5433 -> container 5432 |
| Socket TCP no host | Aprovado via Python; tentativa inicial negada pelo sandbox foi repetida com acesso local à rede aprovado |
| Arquivos físicos no host | PG_VERSION existe em .dockerized-postgres/18/docker e contém 18 |
| Acesso TCP pelo container app | Aprovado; conexão Bash /dev/tcp com prazo em postgres:5432 |
| Sobrevivência à recriação do container | Aprovada; marcador em tabela comum descartável sobreviveu à mudança de ID com mesmo bind mount |
| Limpeza da validação | Aprovada; tabela removida e consulta user_tables retornou 0 |
| Docker cargo fmt --check | Aprovado |
| Docker Clippy em todos os targets/features, sem warnings | Aprovado |
| Docker cargo test --locked | Aprovado; 10 testes (2 configuração, 6 envelope, 2 ciclo de vida) |
| Docker cargo build --locked | Aprovado |
| Git ignores / links / git diff --check | Aprovados; todos os links relativos resolvem |

A tabela `milestone12_validation_20261007` continha apenas `survives-recreation`. O ID do container PostgreSQL mudou de `601cd8ad9a28...` para `ee3342e7a231...`; o marcador permaneceu e a tabela foi removida em seguida. Nenhum schema da aplicação ou artefato de validação permanece. O banco nunca foi resetado ou apagado. Ambos os containers de desenvolvimento continuam ativos; o workspace app não executa o binário HTTP automaticamente.

Não validado: sessão SQL no host ou interface DBeaver (psql ausente no host); a sondagem no host comprova apenas alcance TCP. SQL autenticado foi validado no Docker. Checks Rust nativos não foram repetidos porque Cargo continua indisponível no host; os quatro checks exigidos rodaram no Docker. Portabilidade de permissões em host Linux, backups/operação de produção e integração de persistência da aplicação continuam não validados/fora do escopo. Sem desvios de implementação; nenhuma configuração Rust de conexão sem uso foi introduzida.

Comandos exatos executados (IDs/montagens foram verificados antes e após recriação; a sondagem de socket foi repetida após aprovação do acesso pelo sandbox):

```bash
docker compose up -d --build --wait --wait-timeout 120
docker compose config --quiet
docker compose ps
docker compose logs --tail=20 postgres
docker compose exec -T postgres pg_isready -h 127.0.0.1 -p 5432 -U relay -d reliable_event_relay
docker compose exec -T postgres sh -c 'PGPASSWORD="$POSTGRES_PASSWORD" psql -h postgres -p 5432 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT 1; SHOW server_version; SHOW data_directory;"'
docker compose port postgres 5432
docker compose exec -T app bash -c 'timeout 5 bash -c "</dev/tcp/postgres/5432"'
docker compose exec -T app cargo fmt --check
docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings
docker compose exec -T app cargo test --locked
docker compose exec -T app cargo build --locked
command -v psql
python3 - <<'PYTHON'
import socket
from pathlib import Path
with socket.create_connection(('127.0.0.1',5433), timeout=5):
 print('Host TCP 127.0.0.1:5433 reachable')
p=Path('.dockerized-postgres/18/docker/PG_VERSION')
print('Host PG_VERSION:',p.read_text().strip())
PYTHON
docker compose ps -q postgres
docker inspect rust-event-relay-postgres-1 --format '{{json .Mounts}}'
docker compose exec -T postgres psql -U relay -d reliable_event_relay -v ON_ERROR_STOP=1 -c "CREATE TABLE milestone12_validation_20261007 (marker text); INSERT INTO milestone12_validation_20261007 VALUES ('survives-recreation');"
docker compose up -d --force-recreate --no-deps --wait --wait-timeout 120 postgres
docker compose ps -q postgres
docker inspect rust-event-relay-postgres-1 --format '{{json .Mounts}}'
docker compose exec -T postgres psql -U relay -d reliable_event_relay -v ON_ERROR_STOP=1 -c "SELECT marker FROM milestone12_validation_20261007;"
docker compose exec -T postgres psql -U relay -d reliable_event_relay -v ON_ERROR_STOP=1 -c "DROP TABLE milestone12_validation_20261007;"
docker compose exec -T postgres psql -U relay -d reliable_event_relay -v ON_ERROR_STOP=1 -c "SELECT count(*) AS user_tables FROM pg_tables WHERE schemaname NOT IN ('pg_catalog', 'information_schema');"
git check-ignore .dockerized-postgres/ .cargo-cache/ target/
python3 - <<'PYTHON'
from pathlib import Path
import re
errors=[]; count=0
for p in [Path('README.md'),Path('README.pt-BR.md'),*Path('docs').rglob('*.md')]:
 for target in re.findall(r'\]\(([^)]+)\)',p.read_text()):
  if '://' in target or target.startswith('#'): continue
  path=target.split('#')[0]
  if not (p.parent/path).exists(): errors.append(f'{p}: {target}')
  count+=1
print(f'Checked {count} relative documentation links')
if errors: raise SystemExit('\n'.join(errors))
print('All relative documentation links resolve')
PYTHON
git diff --check
```

## Marco 1.1 — 2026-10-07

O container Docker de desenvolvimento existente estava disponível. Os checks nativos foram tentados individualmente, mas não puderam executar porque Cargo não está instalado/no PATH do host macOS. Os checks Docker usaram o código montado; não foi necessário reconstruir a imagem. `cargo check` resolveu as novas dependências e atualizou Cargo.lock antes dos checks com lockfile.

| Check | Resultado observado |
| --- | --- |
| fmt, Clippy, test e build nativos | Indisponíveis: os quatro retornaram `command not found: cargo` |
| Docker `cargo fmt --check` | Aprovado |
| Docker `cargo clippy --locked --all-targets --all-features -- -D warnings` | Aprovado |
| Docker `cargo test --locked` | Aprovado: 2 testes unitários de configuração, 6 testes de integração do envelope e 2 de ciclo de vida (10 no total) |
| Docker `cargo build --locked` | Aprovado |
| `git diff --check` | Aprovado |

Os seis novos testes de integração cobrem criação, IDs UUID v7 únicos gerados, timestamps UTC atuais, as quatro regras de validação (incluindo identificadores compostos apenas por espaços), formato JSON explícito com nove campos, restauração do ID/instante originais, round-trips com correlação e causalidade ausentes/individuais/combinadas, valores JSON inválidos e normalização de offsets para UTC. Nenhum componente de persistência ou entrega foi implementado ou validado. Os checks nativos são a única divergência de ambiente em relação à validação solicitada para o marco.

Comandos exatos executados neste marco:

```bash
# Tentativas no host: cada comando retornou command not found: cargo
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked
cargo build --locked

# Container Docker de desenvolvimento
docker compose ps
docker compose exec -T app cargo fmt
docker compose exec -T app cargo check
docker compose exec -T app cargo fmt --check
docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings
docker compose exec -T app cargo test --locked
docker compose exec -T app cargo build --locked

# Revisão do workspace
git diff --check
```

## Validação da fundação (anterior)

Data: 2026-10-07. Ambiente: host macOS com Docker Desktop, container de desenvolvimento Linux ARM64, `rustc 1.95.0 (59807616e 2026-04-14)`, UID/GID 1000. Rust nativo não estava instalado; as verificações Rust ocorreram no Docker sobre o repositório montado.

| Validação | Resultado observado |
| --- | --- |
| Build da imagem e partida Compose | Aprovados |
| Bash e ferramentas sem root | Aprovados; developer UID/GID 1000 |
| Cargo fetch e geração do lockfile | Aprovados; Cargo.lock gerado e incluído nas fontes |
| cargo build --locked | Aprovado |
| cargo test --locked e cargo test | Aprovados; 2 testes unitários e 2 de integração |
| cargo fmt --check | Aprovado após aplicar cargo fmt |
| Clippy em todos os targets/features, sem warnings | Aprovado |
| Partida e tracing JSON | Aprovados; ambiente development e endereço registrados |
| Requisição HTTP pelo host | Aprovada; /health retornou HTTP 200 e `ok` |
| Carregamento de .env local | Aprovado via teste de subprocesso isolado |
| Saída graciosa SIGINT e SIGTERM | Aprovada via teste de subprocesso para cada sinal |
| Recuperação completa de cache e artefatos | Aprovada após apagar ambos os diretórios do host e reiniciar Compose |
| Visibilidade dos dados gerados no host | Aprovada; .cargo-cache/registry e target/debug recriados |
| Links relativos e diretórios ignorados | Verificados localmente |

## Comandos da fundação (validação anterior)

O primeiro fetch omitiu intencionalmente `--locked` para gerar o lockfile inicial; as verificações posteriores de fetch/build usaram o lockfile. A remoção Python abaixo apagou apenas diretórios gerados, equivalente ao `rm -rf .cargo-cache target` documentado para recuperação.

```bash
docker compose up -d --build
docker compose exec -T app bash -c 'cargo fmt && cargo fetch && cargo build --locked && cargo test --locked && cargo fmt --check && cargo clippy --locked --all-targets --all-features -- -D warnings'
docker compose exec -T -d app bash -c 'exec /app/target/debug/reliable-event-relay >/tmp/relay-live.log 2>&1'
curl --fail --include http://127.0.0.1:8080/health
docker compose exec -T app bash -c 'id && rustc --version && cat /tmp/relay-live.log'
docker compose down
python3 - <<'PYTHON'
from pathlib import Path
import shutil
for name in ['.cargo-cache', 'target']:
    p = Path(name)
    assert p.is_dir() and not p.is_symlink()
    shutil.rmtree(p)
PYTHON
docker compose up -d
docker compose exec -T app bash -c 'cargo fetch --locked && cargo build --locked && cargo test --locked && cargo fmt --check && cargo clippy --locked --all-targets --all-features -- -D warnings'
docker compose exec -T app cargo test
```

Os testes unitários cobrem configuração padrão e rejeições. O teste HTTP usa socket real e encerramento injetado. O teste de subprocesso fornece `.env` temporário fora do repositório e valida os dois sinais Unix. As regras Git foram verificadas com `git check-ignore .env .cargo-cache/ target/ .vscode/`; links Markdown relativos foram resolvidos contra o sistema de arquivos. A revisão confirmou que relay/durabilidade estão marcados como planejados.

## Limitações da fundação (validação anterior)

Rust nativo no host, sinais no Windows, portabilidade UID/GID em host Linux, execução de CI hospedada no GitHub e implantação de produção não foram validados. PostgreSQL, RabbitMQ, Redis, entrega de webhooks, tentativas, idempotência, contrapressão e recuperação de crashes não foram implementados nem testados. Não há benchmarks ou alegações de vazão/latência. Health indica apenas liveness HTTP. O container de desenvolvimento permanece ativo após a validação; o processo HTTP temporário terminou quando Compose foi parado para recuperação. Inicie o serviço com `docker compose exec app cargo run --locked`.
