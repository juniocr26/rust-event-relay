[English](../en/validation-results.md) | [README](../../README.pt-BR.md)

# Resultados de validação

## Marco 1.3 — Validação da infraestrutura de migrações

Data: 2026-10-07. Docker Desktop no macOS, container app Linux ARM64. SQLx informa `sqlx-cli 0.8.6`; instalação usou `--locked --no-default-features --features rustls,postgres`. Nenhuma dependência da aplicação ou código de domínio mudou.

| Verificação | Resultado observado |
| --- | --- |
| Build da imagem e startup condicionado ao health | Passou |
| Cluster existente do desenvolvedor | Saudável, publicado em 127.0.0.1:5433; credenciais atuais falham na autenticação TCP e sqlx migrate info |
| Preservação de dados | Sem reset, alteração de usuário/senha ou aplicação de migrações ao cluster do desenvolvedor |
| Geração isolada | CLI produziu par up/down com timestamp em /tmp/milestone13-generated do container |
| Migração isolada info/run/revert/run | Passou; pending → installed → pending → installed |
| Inspeção namespace | relay presente após aplicar, ausente após reverter e presente após reaplicar; zero tabelas relay |
| Histórico SQLx | Versão 20261007000000, success true; execução repetida não aplicou nada |
| Encoding de credenciais | Conexão SQLx autenticada passou com senha de teste contendo : @ / % ? # |
| Autenticação TCP interna e pela publicação no host | SELECT 1 passou; host.docker.internal:15433 mapeou para PostgreSQL de teste |
| Ambiente de inicialização mudado com dados persistidos de teste | Novas credenciais falharam, originais do app conectaram; restauração preservou migração instalada |
| PostgreSQL indisponível | SQLx encerrou com erro DNS/conexão; reinício saudável restaurou status installed |
| cargo fmt --check | Passou no Docker |
| Clippy todos targets/features, warnings negados | Passou no Docker |
| cargo test --locked | Passou: 2 testes unitários + 6 envelope + 2 ciclo de vida (10) |
| cargo build --locked | Passou no Docker |
| git diff --check e links Markdown locais relativos | Passou |

A divergência de credenciais foi observada diretamente; usuário/senha reais foram omitidos. A validação usou projeto separado `relay-milestone13-validation`, dados em /tmp e loopback 15433. Configuração apenas ilustrativa e função Compose exata estão em [testes](testing.md#ambiente-isolado-de-validação). Nenhum diretório de dados do desenvolvedor foi excluído. Containers/rede do teste foram removidos; dados temporários permanecem em /tmp. Ambiente original continua executando.

Publicação de porta e credenciais foram validadas no ambiente isolado, mas a interface DBeaver não foi testada diretamente. Publicação original foi confirmada; credenciais configuradas falharam, portanto não se afirma conexão com elas. Integração outbox, rollback em produção, Rust nativo, portabilidade Linux e execução CI GitHub não foram validados.

### Comandos exatos executados

No projeto original (falhas de autenticação abaixo são constatações esperadas, não verificações bem-sucedidas):

```bash
docker compose ps
docker compose up -d --build --wait --wait-timeout 120
docker compose exec -T app sqlx --version
docker compose exec -T app sqlx migrate info
docker compose exec -T postgres sh -c 'PGPASSWORD="$POSTGRES_PASSWORD" psql -h postgres -p 5432 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT 1;"'
docker compose port postgres 5432
docker compose exec -T app cargo fmt --check
docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings
docker compose exec -T app cargo test --locked
docker compose exec -T app cargo build --locked
```

Após criar arquivos do ambiente isolado conforme testes:

```bash
docker compose -p relay-milestone13-validation --env-file /tmp/relay-milestone13-validation/test.env -f compose.yaml -f /tmp/relay-milestone13-validation/compose.yaml up -d --wait --wait-timeout 120
dc() {
  docker compose -p relay-milestone13-validation --env-file /tmp/relay-milestone13-validation/test.env -f compose.yaml -f /tmp/relay-milestone13-validation/compose.yaml "$@"
}
dc ps
dc exec -T app sqlx --version
dc exec -T app sqlx migrate add -r --source /tmp/milestone13-generated create_relay_schema
dc exec -T app sqlx migrate info
dc exec -T app sqlx migrate run
dc exec -T app sqlx migrate info
dc exec -T postgres sh -c 'PGPASSWORD="$POSTGRES_PASSWORD" psql -h postgres -p 5432 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT nspname FROM pg_namespace WHERE nspname = '\''relay'\''; SELECT version, success FROM _sqlx_migrations; SELECT tablename FROM pg_tables WHERE schemaname = '\''relay'\'';"'
dc exec -T app sqlx migrate revert
dc exec -T app sqlx migrate info
dc exec -T postgres sh -c 'psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT count(*) AS relay_schema_count FROM pg_namespace WHERE nspname = '\''relay'\'';"'
dc exec -T app sqlx migrate run
dc exec -T app sqlx migrate run
dc exec -T app sqlx migrate info
dc port postgres 5432
dc exec -T postgres sh -c 'PGPASSWORD="$POSTGRES_PASSWORD" psql -h host.docker.internal -p 15433 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT 1;"'
```

Para reproduzir inicialização de credenciais, foi escrito override adicional apenas de teste:

```yaml
# /tmp/relay-milestone13-validation/changed-environment.yaml
services:
  postgres:
    environment:
      POSTGRES_USER: validation_changed_user
      POSTGRES_PASSWORD: validation_changed_password
```

Depois executado (com a mesma função dc):

```bash
dc -f /tmp/relay-milestone13-validation/changed-environment.yaml up -d --force-recreate --no-deps --wait --wait-timeout 120 postgres
if dc exec -T postgres sh -c 'PGPASSWORD="$POSTGRES_PASSWORD" psql -h postgres -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT 1;"'; then
  echo 'Unexpected success with changed initialization credentials' >&2
  exit 1
fi
# app retains the original isolated test credentials, proving they still work.
dc exec -T app sqlx migrate info
dc up -d --force-recreate --no-deps --wait --wait-timeout 120 postgres
dc exec -T postgres sh -c 'PGPASSWORD="$POSTGRES_PASSWORD" psql -h postgres -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT 1;"'
dc exec -T app sqlx migrate info
```

Falha com servidor indisponível e encerramento:

```bash
dc stop postgres
if dc exec -T app sqlx migrate info --connect-timeout 2; then
  echo 'Unexpected success with unavailable PostgreSQL' >&2
  exit 1
fi
dc up -d --wait --wait-timeout 120 postgres
dc exec -T app sqlx migrate info
dc down
```

Os três grupos rodaram como scripts temporários com `set -eu`; falhas esperadas usaram if para verificar saída diferente de zero. `dc exec -T app sqlx migrate info --help` também foi executado para consultar timeout de conexão. Verificação do workspace usou `git diff --check`, `git check-ignore .env .dockerized-postgres/ .cargo-cache/ target/` e varredura Python pathlib/re de links e âncoras locais. Sem commit ou push.

Asserções de encoding de URL e topologia também passaram no Docker. O comando adicional exato usou apenas credenciais sintéticas:

```bash
docker compose exec -T app python3 - <<'PY'
import importlib.util
from urllib.parse import unquote, urlsplit
spec = importlib.util.spec_from_file_location('sqlx_wrapper', '/app/docker/sqlx.py')
wrapper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wrapper)
environment = dict(POSTGRES_USER='user:@/雪', POSTGRES_PASSWORD='password:@/%?#雪', POSTGRES_DB='db/@?#雪', POSTGRES_HOST='postgres', POSTGRES_PORT='5432')
url = urlsplit(wrapper.database_url(environment))
assert unquote(url.username) == environment['POSTGRES_USER']
assert unquote(url.password) == environment['POSTGRES_PASSWORD']
assert unquote(url.path[1:]) == environment['POSTGRES_DB']
assert url.hostname == 'postgres' and url.port == 5432
assert not url.query and not url.fragment
for key, value in [('POSTGRES_HOST', 'localhost'), ('POSTGRES_PORT', '5433')]:
    try:
        wrapper.database_url({**environment, key: value})
    except ValueError:
        pass
    else:
        raise AssertionError('invalid Docker topology accepted')
print('URL encoding and topology validation passed')
PY
```

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
python3 - <<'PY'
import socket
from pathlib import Path
with socket.create_connection(('127.0.0.1',5433), timeout=5):
 print('Host TCP 127.0.0.1:5433 reachable')
p=Path('.dockerized-postgres/18/docker/PG_VERSION')
print('Host PG_VERSION:',p.read_text().strip())
PY
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
python3 - <<'PY'
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
PY
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
python3 - <<'PY'
from pathlib import Path
import shutil
for name in ['.cargo-cache', 'target']:
    p = Path(name)
    assert p.is_dir() and not p.is_symlink()
    shutil.rmtree(p)
PY
docker compose up -d
docker compose exec -T app bash -c 'cargo fetch --locked && cargo build --locked && cargo test --locked && cargo fmt --check && cargo clippy --locked --all-targets --all-features -- -D warnings'
docker compose exec -T app cargo test
```

Os testes unitários cobrem configuração padrão e rejeições. O teste HTTP usa socket real e encerramento injetado. O teste de subprocesso fornece `.env` temporário fora do repositório e valida os dois sinais Unix. As regras Git foram verificadas com `git check-ignore .env .cargo-cache/ target/ .vscode/`; links Markdown relativos foram resolvidos contra o sistema de arquivos. A revisão confirmou que relay/durabilidade estão marcados como planejados.

## Limitações da fundação (validação anterior)

Rust nativo no host, sinais no Windows, portabilidade UID/GID em host Linux, execução de CI hospedada no GitHub e implantação de produção não foram validados. PostgreSQL, RabbitMQ, Redis, entrega de webhooks, tentativas, idempotência, contrapressão e recuperação de crashes não foram implementados nem testados. Não há benchmarks ou alegações de vazão/latência. Health indica apenas liveness HTTP. O container de desenvolvimento permanece ativo após a validação; o processo HTTP temporário terminou quando Compose foi parado para recuperação. Inicie o serviço com `docker compose exec app cargo run --locked`.
