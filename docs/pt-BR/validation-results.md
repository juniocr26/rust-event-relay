[English](../en/validation-results.md) | [README](../../README.pt-BR.md)

# Resultados de validação

## Marco 1.5 — Validação da abstração de persistência

Data: 2026-10-07. Container Docker de desenvolvimento, Rust 1.95.0. Novos testes Rust não usam banco, driver, configuração de ambiente, pools ou SQL. Sem migrações/alterações de schema ou chamadas PostgreSQL neste marco. Validação anterior de schema/credenciais permanece abaixo.

| Verificação | Resultado real |
| --- | --- |
| Primeira tentativa de teste específico e Clippy | Falhou: E0283/E0284, timezone ambígua no parse Chrono de uma nova asserção |
| Correção | parse::<DateTime<Utc>> explícito; sem alterações no envelope/schema |
| Docker cargo fmt --check | Passou após formatação |
| Clippy Docker todos targets/features, warnings negados | Passou |
| Docker cargo test --locked | Passou: 2 unitários, 6 envelope, 2 ciclo de vida, 5 persistência (15 total) |
| Docker cargo build --locked | Passou |
| Rustdoc -D warnings, --locked --no-deps | Passou; API pública documentada |
| git diff --check / links e âncoras locais | Passou |
| Revisão de fronteira/código | Sem imports/tipos SQLx/PostgreSQL, adapter concreto, mutações, worker ou append de produtor |
| Verificação de escopo | Cargo manifest/lock, EventEnvelope, migrações e configuração inalterados |

Cinco novos testes cobrem limites positivos/rejeição de zero/UTC explícito, envelope preservado com tentativas unsigned/disponibilidade separadas, categorias/fontes downcastable com Display/Debug sanitizados, contrato genérico em future Send Tokio e sucesso vazio/falha tipada. Fake de resultado preparado demonstra substituição, não filtros, locks, durabilidade, claims ou conformidade de adapter. Garantias exigem 1.6/1.7 e semântica futura de workers.

Comandos exatos executados (tentativa inicial específica falhou antes da correção UTC):

```bash
# Initial attempt
docker compose exec -T app cargo fmt
docker compose exec -T app cargo test --locked --test persistence_contract
docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings

# After fixing the test annotation
docker compose exec -T app cargo fmt
docker compose exec -T app cargo fmt --check
docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings
docker compose exec -T app cargo test --locked
docker compose exec -T app cargo build --locked
docker compose exec -T -e RUSTDOCFLAGS='-D warnings' app cargo doc --locked --no-deps
git diff --check
```

Varredura Python pathlib/re existente resolveu links/âncoras nos READMEs/docs. Revisão de diff/código confirmou dependências, migrações e envelope inalterados, sem acoplamento a driver no módulo. Sem commit/push.

Decisões: um contrato limitado somente leitura sem congelar APIs claim/conclusão/reagendamento/dead-letter; sem writer que deturpa atomicidade do produtor; sem enum terminal sem visão retornada. São escolhas permitidas de fronteira menor, não implementação do 1.6. Questões abertas: propriedade/recuperação, atomicidade/idempotência/conflitos, momento das tentativas, duplicatas, limites de payload/operação e ordem por agregado. Sem alegar produção/throughput/segurança concorrente. Configuração e banco permanecem inalterados.

## Marco 1.4 — Validação do schema outbox

Data: 2026-10-07. Cluster real PostgreSQL 18.6, SQLx CLI 0.8.6, Docker Desktop/Linux ARM64, Rust 1.95.0. Diagnóstico de autenticação passou antes do trabalho. Sem reset do cluster. Inspeção inicial mostrou ausência da tabela outbox e somente migração 20261007000000 instalada; rollback da nova tabela vazia era seguro.

| Validação | Resultado observado |
| --- | --- |
| Geração reversível | CLI criou par up/down 20261007175358_create_outbox_events |
| info/run/info real | Pending → applied → installed |
| Inspeção de catálogos | 15 colunas, tipos/defaults/nullability corretos, PK UUID, sete CHECKs nomeados, 11 constraints NOT NULL |
| Inspeção de índices | B-tree PK e parcial pending (available_at, created_at, id); sem índices payload/agregado |
| Fixture SQL | 25 casos passaram; envelope/defaults/JSONB válidos, ID duplicado, falhas NULL obrigatório, strings vazias, limites de versão/tentativas/status/conclusão inválidos e representação estendida do envelope/ciclo |
| Limpeza fixture | ROLLBACK concluído; count(*) = 0 |
| migrate revert real | Outbox absent=true, relay preserved=true, history preserved=true; nova migração pending |
| Reaplicação real | Outbox recriada, ambas versões installed com success=true |
| Inspeção final somente leitura | Tabela/índices/constraints presentes; outbox_rows=0 |
| Docker fmt / Clippy / test / build | Passou; warnings negados; 2 unitários + 6 envelope + 2 ciclo de vida (10) |
| git diff --check / links e âncoras Markdown locais | Passou |

Fixture SQL valida schema, não integração de persistência Rust. Falhas são capturadas/verificadas em subtransações PL/pgSQL, incluindo nomes esperados; linhas ficam em transação externa desfeita. Sem eventos de validação ou seeds restantes. Banco final mantém schema/migração aplicada. Migrações anteriores e Rust/dependências inalterados; sem repositório, publicador, worker, consulta claim ou retries executados.

### Comandos exatos executados

```bash
./scripts/check-postgres.sh
docker compose exec -T app sqlx migrate add -r create_outbox_events
# Generated version: 20261007175358; edited SQL before applying.
docker compose exec -T app sqlx migrate info
docker compose exec -T app sqlx migrate run
docker compose exec -T app sqlx migrate info
docker compose exec -T postgres sh -c 'psql -X -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1' < tests/sql/outbox_schema.sql
docker compose exec -T postgres sh -c 'psql -X -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT count(*) AS outbox_rows FROM relay.outbox_events;"'
docker compose exec -T app sqlx migrate revert
docker compose exec -T app sqlx migrate info
docker compose exec -T postgres sh -c 'psql -X -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT to_regclass('\''relay.outbox_events'\'') IS NULL AS outbox_absent, to_regnamespace('\''relay'\'') IS NOT NULL AS relay_preserved, to_regclass('\''public._sqlx_migrations'\'') IS NOT NULL AS history_preserved;"'
docker compose exec -T app sqlx migrate run
docker compose exec -T app sqlx migrate info
docker compose exec -T postgres sh -c 'psql -X -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT count(*) AS outbox_rows FROM relay.outbox_events;"'
docker compose exec -T postgres sh -c 'psql -X -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1' < tests/sql/inspect_outbox_schema.sql
docker compose exec -T app cargo fmt --check
docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings
docker compose exec -T app cargo test --locked
docker compose exec -T app cargo build --locked
git diff --check
```

Inspeção inicial SELECT to_regclass('relay.outbox_events') e SELECT version, description, success FROM public._sqlx_migrations confirmou ausência/histórico anterior. Antes do rollback, consultas equivalentes ao arquivo versionado foram executadas inline pelo psql para colunas, pg_constraint, pg_indexes e histórico. Arquivo final repetiu verificações somente leitura após reaplicar, com contagem explícita. Método Python pathlib/re resolveu links e âncoras locais.

Desvio do tipo sugerido: schema_version usa BIGINT CHECK BETWEEN 1 AND 4294967295 porque INTEGER assinado não representa todo NonZeroU32 Rust. Sem mudança no envelope. Sem benchmarks de produção, claims concorrentes, garantias de entrega ou integração de repositório; interface DBeaver não testada. Rollback validado somente com tabela vazia. Sem commit/push. Resultados anteriores abaixo foram preservados como histórico.

## Correção de autenticação PostgreSQL local

Data: 2026-10-07. Esta correção posterior preserva a falha original do Marco 1.3 e validação isolada abaixo.

**Causa raiz:** o usuário configurado atual não existia no cluster inicializado. TCP retornou falha de senha; detalhes dos logs e inspeção autorizada por socket confirmaram ausência do usuário. Foi divergência de estado inicializado, não senha antiga comprovada em usuário existente. Nenhum hash foi consultado; credenciais reais foram omitidas.

Antes do reset: PostgreSQL 18.6, diretório `/var/lib/postgresql/18/docker`, um superusuário antigo de login e usuários internos. Banco postgres e banco configurado tinham apenas public, zero relações/rotinas de usuário e nenhum histórico de migração ou dado da aplicação. A inspeção satisfez a autorização explícita de reset. Após parar Compose, apenas `.dockerized-postgres/` foi excluído. `.env`, cache Cargo e target foram preservados; sem exclusão automática.

| Validação | Resultado observado |
| --- | --- |
| PostgreSQL real reinicializado | Saudável; containers correspondem ao Compose/.env atual |
| Usuário/senha/banco atuais via TCP interno | SELECT current_user, current_database autenticado correspondeu aos valores configurados (identificadores omitidos) |
| SQLx real info/run/info | Pending → applied → installed; version 20261007000000, success true |
| Inspeção de schema | public e relay vazio; public._sqlx_migrations é a única tabela de usuário |
| Porta loopback macOS | nc conectou em 127.0.0.1:5433 |
| Autenticação SQL pela publicação | Passou via host.docker.internal:5433 com credenciais atuais |
| Diagnóstico somente leitura | Passou: health, igualdade de configuração, autenticação e histórico existente |
| Divergência sintética | Falha não-zero; sem recriação de container ou alteração de .env |
| Senha sintética incorreta | Trecho de autenticação falhou com erro genérico sem credenciais |
| Docker fmt, Clippy, test, build | Passou; warnings negados e 10 testes Rust passaram |
| Sintaxe shell, git diff --check, links/âncoras relativos | Passou |
| Interface DBeaver | Não testada diretamente |

DBeaver: PostgreSQL, 127.0.0.1:5433, banco POSTGRES_DB e usuário POSTGRES_USER, com POSTGRES_PASSWORD atual inalterado. Sem necessidade de adivinhar/substituir credenciais. O cluster real aceita esses valores. Sem Marco 1.4, tabelas da aplicação, repositório, workers ou brokers. Sem commit ou push.

### Comandos exatos da correção

Wrappers Python temporários capturaram e removeram credenciais de `docker compose ps`, `docker compose config --format json`, `docker compose logs --tail=100 postgres`, publicação e POSTGRES_USER/DB/HOST/PORT do container. Configuração bruta não foi impressa por conter segredos. SQL por socket autorizado leu versão/data_directory, pg_roles sem hashes, pg_database e schemas/relações/rotinas de cada banco não-template. Identificadores reais de login foram omitidos.

```bash
python3 /tmp/relay-postgres-diagnose.py
python3 /tmp/relay-postgres-inventory.py
docker compose down
```

A exclusão usou o comando com verificações abaixo em vez de rm sem proteção:

```bash
python3 - <<'PY'
from pathlib import Path
import json
import shutil
inventory=json.loads(Path('/tmp/relay-postgres-inventory-result.json').read_text())
assert len(inventory['inventory']) == 2
for database in inventory['inventory']:
    state=database['inventory']
    assert state['schemas'] == ['public']
    assert not state['relations']
    assert state['routines'] == 0
path=Path.cwd()/'.dockerized-postgres'
assert path.is_dir() and not path.is_symlink()
assert path.resolve() == path
shutil.rmtree(path)
print('Explicitly authorized empty-cluster reset completed; .env and Cargo directories preserved')
PY
docker compose up -d --build --wait --wait-timeout 120
docker compose exec -T app sqlx migrate info
docker compose exec -T app sqlx migrate run
docker compose exec -T app sqlx migrate info
./scripts/check-postgres.sh
sh -n scripts/check-postgres.sh
if POSTGRES_PASSWORD=deliberately-invalid-test ./scripts/check-postgres.sh; then exit 1; else echo 'Diagnostic rejects configuration drift as expected'; fi
```

O diagnóstico autentica via TCP e compara SELECT current_user, current_database à configuração sem imprimir identidade. Autenticação adicional pela publicação e inspeção de schema:

```bash
docker compose exec -T postgres sh -s <<'CONTAINER'
set -eu
export PGPASSWORD="$POSTGRES_PASSWORD"
result=$(psql -X -h host.docker.internal -p 5433 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -At -F '|' -c 'SELECT current_user, current_database();')
[ "$result" = "$POSTGRES_USER|$POSTGRES_DB" ]
echo 'Authenticated query through host publication: current_user and current_database match configuration (identifiers redacted).'
psql -X -h postgres -p "$POSTGRES_PORT" -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT nspname FROM pg_namespace WHERE nspname NOT LIKE 'pg_%' AND nspname <> 'information_schema'; SELECT schemaname, tablename FROM pg_tables WHERE schemaname NOT LIKE 'pg_%' AND schemaname <> 'information_schema'; SELECT version, success FROM public._sqlx_migrations;"
CONTAINER
```

Teste subprocess Python temporário extraiu o trecho de container do diagnóstico, executou com `docker compose exec -T -e POSTGRES_PASSWORD=deliberately-invalid-test postgres sh -s` e confirmou saída não-zero e mensagem genérica de falha. Credenciais armazenadas não mudaram. Comandos de qualidade:

```bash
docker compose exec -T app cargo fmt --check
docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings
docker compose exec -T app cargo test --locked
docker compose exec -T app cargo build --locked
git diff --check
git check-ignore .env
```

Links/âncoras usaram a mesma varredura Python pathlib/re do Marco 1.3. Limitações: interface DBeaver não testada manualmente; autenticação pela publicação usou gateway Docker Desktop em vez de psql no host. Portabilidade Linux e recuperação em produção não testadas. Desvios: exclusão Python com verificações substituiu rm bruto; saída de diagnóstico omitiu credenciais em vez de expor configuração. Sem outros desvios.

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

## Marco 1.6 — Repositório PostgreSQL (2026-10-08)

Checkout iniciou limpo em `223953f`, após `39eca7a`. Nenhum AGENTS.md aplicável encontrado. Docker Desktop app/PostgreSQL saudável acessíveis após acesso ao socket aprovado pelo sandbox; app usa Rust 1.95.0. SQLx aplicação 0.8.6 corresponde ao CLI, defaults desabilitados e features postgres/runtime-tokio/uuid/chrono/json. serde_json arbitrary_precision evita arredondar payload numérico. Resolução Cargo preservou versões de todos os pacotes previamente fixados e adicionou novas dependências transitivas.

As duas migrações existentes estavam pending. Wrapper existente `docker compose exec -T app sqlx migrate run` aplicou ambas explicitamente; info final mostra installed. Nenhuma migração reescrita, credencial alterada ou reset do banco do desenvolvedor.

| Verificação final executada | Resultado |
| --- | --- |
| cargo fmt --check | Passou |
| cargo clippy --locked --all-targets --all-features -- -D warnings | Passou |
| cargo test --locked | Passou: 22 testes; um smoke opt-in ignorado |
| cargo build --locked | Passou |
| RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps | Passou |
| cargo test --locked --test postgres_read_smoke -- --ignored | Passou: um smoke PostgreSQL isolado |
| git diff --check | Passou |

Comandos Cargo executados via `docker compose exec -T app`; rustdoc recebeu flags por `-e`. Sem macros SQL ou metadados de build dependentes do banco. Smoke usa banco temporário com nome único e DDL das migrações inalteradas: leitura vazia, corte/pending, seleção determinística limitada, versão máxima, strings/IDs/timestamps/UUIDs/JSON numérico exato, repetição e igualdade antes/depois de todas as colunas. Timestamps infinity/finitos fora do intervalo Chrono e campos whitespace inválidos falham lote inteiro com InvalidStoredData.

Smoke inicial revelou panic do decoder Chrono SQLx 0.8.6 em infinity. Decoder privado com aritmética verificada corrigiu isso. Primeiro panic deixou banco fixture, removido explicitamente pelo nome gerado exato. Teste agora executa verificação em task e limpa banco inclusive após panic. Comparação posterior revelou normalização PostgreSQL da grafia de expoentes; fixture final usa grafia numérica armazenada exata e testes unitários cobrem expoentes grandes. Erro temporário de compilação de teste com fonte boxed também foi corrigido antes das verificações finais. Falhas fixture exibem texto estático, sem fontes brutas/credenciais.

Inspeção final encontrou zero bancos relay_smoke_* e zero linhas na outbox do desenvolvedor. Fixtures não gravaram nessa outbox. Nenhuma validação bloqueada. Smoke não conclui matriz completa de integração/concorrência do 1.7 nem análise ampla de falhas/transações/recuperação do 1.8; sem processamento/entrega implementados. Seções históricas descrevem escopo dos marcos originais, não dependências atuais.

## Marco 1.7 — Integração do repositório PostgreSQL (2026-10-08)

Checkout inicialmente limpo em `c771f62`; nenhum AGENTS.md aplicável. App Docker existente informa Rust 1.95.0. Smoke histórico do Marco 1.6 substituído por `tests/postgres_repository.rs` e suporte `tests/support/mod.rs`: 14 casos PostgreSQL ignorados por padrão e um caso de falha pelo contrato público sem banco. Nenhum defeito produtivo encontrado; código produtivo, dependências, migrações, credenciais inicializadas e dados da aplicação permaneceram inalterados. Sem commit ou push.

Comandos finais via `docker compose exec -T app` (flags rustdoc por `-e`):

| Comando | Resultado real |
| --- | --- |
| `cargo fmt --check` | Passou |
| `cargo clippy --locked --all-targets --all-features -- -D warnings` | Passou |
| `cargo test --locked` | Passou: 23 testes; 14 casos de banco ignorados |
| `cargo test --locked --test postgres_repository -- --ignored --test-threads=1` | Passou: 14 casos |
| `cargo test --locked --test postgres_repository -- --ignored --test-threads=4` | Passou: 14 casos; isolamento paralelo verificado |
| `cargo build --locked` | Passou |
| `RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps` | Passou |
| `git diff --check` | Passou |

Probes deliberados selecionaram somente `eligibility_bounds_and_adapter_tie_breakers -- --ignored --exact`: `docker compose exec -T app env -u POSTGRES_USER cargo test --locked --test postgres_repository ...` saiu 101 com `missing required POSTGRES_USER`; sobrescrever somente POSTGRES_HOST=127.0.0.1 e POSTGRES_PORT=1 nesse comando saiu 101 após 10 segundos com falha administrativa sanitizada. Falhas esperadas comprovam que execução explícita não ignora configuração ausente/infraestrutura indisponível. Não criaram bancos nem alteraram configuração do container/servidor.

Cada caso bem-sucedido fecha pool, remove seu banco pelo nome controlado exato e verifica ausência por consulta parametrizada, incluindo casos com panic ou erro retornado intencionais. Consulta final somente leitura `SELECT count(*) FROM pg_database WHERE datname ~ '^relay_it_[0-9a-f]{32}$'` retornou 0 após todas execuções/probes. Nenhum banco não relacionado removido ou resetado. Término abrupto ou limpeza malsucedida ainda pode deixar fixture isolada; documentação explica limpeza manual pelo nome exato.

Compilação inicial rejeitou lifetime de closure SQLx raw_sql em after_connect; prazos de sessão foram movidos para opções de inicialização PgConnectOptions. Nenhuma correção produtiva necessária. Todas verificações finais passaram. Job CI PostgreSQL 18.6 adicionado com credenciais descartáveis; nenhuma execução remota GitHub realizada. Execução nativa no host e recuperação de término abrupto/crash não validadas. Marco 1.7 implementado; próximo é 1.8, análise ampla de falhas/recuperação e transações. Observação por dois leitores não comprova entrega concorrente segura nem snapshots sob toda sequência de escritas concorrentes.

## Marco 1.8 e fechamento do Marco 1 (2026-10-08)

Execução atual começou em `7cba38d` limpo, suíte 1.7 commitada. Sem AGENTS.md aplicável. Registros anteriores de 1.7 acima são históricos; todas verificações abaixo foram executadas novamente nesta árvore de fechamento. Marco 1.8 é documentação/revisão, sem implementação de recuperação de crash. Marco 1 fechado nos critérios 1.1-1.8, sem bloqueadores; Marco 2 não iniciado. Mudanças de fechamento não commitadas; sem commit, push ou execução remota de CI.

[Revisão](milestone-1-review.md) encontrou três inconsistências documentais médias (status antigo, sugestão de ausência de locks, SQL fonte versus metadados no banco) e duas melhorias baixas de cobertura/validação. Corrigidas nos dois idiomas. Nenhum defeito produtivo confirmado; sem alteração produtiva, dependências, Cargo.lock ou migrações. Caso JSON complexo agora compara fixture inteira explicitamente, normalizando somente representação do expoente. Novo caso ignorado executa fixture SQL existente de 25 assertions no harness isolado e verifica zero linhas após rollback.

### Comandos e resultados atuais

Cargo pelo app existente via `docker compose exec -T app`; rustdoc estrito com `docker compose exec -T -e RUSTDOCFLAGS='-D warnings' app cargo doc --locked --no-deps`. Formatação aplicada uma vez com `cargo fmt` antes das verificações.

| Comando executado | Resultado nesta execução |
| --- | --- |
| `docker compose exec -T app rustc --version` | Rust 1.95.0 |
| `./scripts/check-postgres.sh` | Passou: health, TCP 127.0.0.1:5433, igualdade de configuração atual/containers, identidade autenticada e duas entradas de histórico bem-sucedidas |
| `docker compose exec -T app sqlx migrate info` | 20261007000000 e 20261007175358 instaladas; nenhuma aplicação necessária |
| `cargo fmt --check` | Passou |
| `cargo clippy --locked --all-targets --all-features -- -D warnings` | Passou |
| `cargo test --locked` | Passou: 23 testes sem banco; 15 casos de banco ignorados |
| `cargo test --locked --test postgres_repository -- --ignored --test-threads=1` | Passou: 15 casos opt-in |
| `cargo test --locked --test postgres_repository -- --ignored --test-threads=4` | Passou: 15 casos com isolamento paralelo |
| `cargo build --locked` | Passou |
| `RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps` | Passou |
| `git diff --check` | Passou |

Script versionado `tests/sql/inspect_outbox_schema.sql` executado somente leitura por `docker compose exec -T postgres sh -c 'PGCONNECT_TIMEOUT=5 PGOPTIONS="-c statement_timeout=5000" psql -X -w -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1' < tests/sql/inspect_outbox_schema.sql`. Confirmou 15 colunas, 11 NOT NULL, chave primária/sete checks (PostgreSQL 18 também expõe NOT NULL como constraints do catálogo), índices de identidade/pending, ambas migrações e zero linhas outbox da aplicação. Todas gravações fixture ocorreram em banco gerado; nenhuma na outbox compartilhada. Sem reset/rollback compartilhado, mudança de credenciais ou restart PostgreSQL.

Consulta somente leitura com prazos observou PostgreSQL `18.6 (Debian 18.6-1.pgdg12+2)`, isolamento `read committed` e fsync/synchronous_commit/full_page_writes `on`. São observações da sessão/configuração, sem teste de energia/storage. `SELECT count(*) FROM pg_database WHERE datname ~ '^relay_it_[0-9a-f]{32}$'` retornou 0 após execuções finais. Cada harness também verifica ausência do nome exato parametrizado após fechar pools/remover banco, incluindo limpeza após assertion/erro. Nenhum banco não relacionado removido.

### Documentação e PDF

[Semântica de falhas/transações](failure-and-transaction-semantics.md) usa documentação oficial PostgreSQL major 18 e SQLx 0.8.6, distinguindo testes, código inspecionado, semântica publicada e inferência futura. ADRs 001-005 preservados sem mudar decisões históricas. READMEs/status, arquitetura, guias, armazenamento/contratos/repositório/testes e revisão reconciliados nos dois idiomas; registros históricos preservados.

`HANDOFF_MARCO_1.pdf` gerado de `docs/pt-BR/handoff-marco-1.md` por `scripts/generate_handoff.py`, com ReportLab em ambiente Python temporário isolado, sem dependências Rust/Dockerfile. Poppler 22.12.0 instalado somente no container de desenvolvimento para QA. Metadados e reabertura pypdf estrita passaram; oito páginas A4 têm texto selecionável e acentos. Todas renderizadas a 110 dpi via pdftoppm e inspecionadas visualmente. Parágrafo de referências inicialmente órfão gerou nona página; tabela condensada, PDF regenerado e layout final de oito páginas conferido. Extração textual, limites de página, numeração, links relativos e whitespace verificados. Base revisada e estado não commitado explícitos; sem credenciais, caminhos pessoais ou logs brutos no handoff.

### Limites da evidência

Job CI dedicado revisado frente ao comando testado e serviço PostgreSQL 18.6; nenhuma execução GitHub remota. Rust nativo no host, Windows, deploy produtivo, benchmark/desempenho, exercícios de backup/restore, limpeza/recuperação após término abrupto, injeção de falhas publisher/ack, prazo de cancelamento, pool esgotado na integração e toda sequência de escritas concorrentes não validados. Nenhum check de aceitação atual bloqueado ou pulado. Limitações futuras não transformam persistência/observação em garantia de entrega funcional; veja matriz de falhas e decisões abertas.

## Marco 2.1 — 2026-10-08

Executado no host Docker Desktop do usuário com Rust 1.95.0, Lapin 4.12.0, RabbitMQ 4.3.6 management e PostgreSQL 18.6 preservado. Implementação direta sem commit/push. Handoff do Marco 1 não foi editado. [Escopo e comandos do Marco 2.1](milestone-2-1.md) substituem descrições anteriores de setup/runtime.

| Verificação | Resultado real |
| --- | --- |
| Rebuild da imagem app e build do binário real | Passaram; usuário developer e montagens Cargo graváveis mantidos |
| Formatação / Clippy estrito / rustdoc estrito / whitespace do diff | Passaram |
| Suíte Rust padrão | 27 passaram, 18 casos opt-in ignorados; sem necessidade de PostgreSQL/RabbitMQ |
| Integração PostgreSQL existente | 15 casos ignorados passaram com 4 threads |
| Target integração RabbitMQ | 2 passaram: envelope/propriedades exatos, ack roteado, falha de ack com retorno, timeout de confirm e cancelamento pós-envio por proxy isolado, sem retry |
| Conexão RabbitMQ própria fechada | 1 caso opt-in de biblioteca passou |
| Handshake retido com prazo / transporte recusado / fontes tipadas sanitizadas | Passaram na suíte unitária padrão |
| Endpoint management no host | HTTP 200 com página de login exatamente em http://localhost:15672/ |
| Management autenticado | `/api/whoami` e vhost visível passaram com credenciais `.env` e tag management; configure/write/read escopados verificados via CLI do nó |
| Detecção de conflito de porta | Porta temporária ocupada informada; nenhum serviço compartilhado ou porta padrão alterados |
| Controle Supervisor | Status, stop/start coletivos e stop/start/restart por nome passaram com supervisorctl direto, alias executável supervisor e ambos consoles interativos |
| Ciclo do filho | HTTP indisponível quando parado e saudável após start/restart; PID manager mantido; modos/propriedade developer do socket privado verificados |
| Saída inesperada | SIGKILL somente no filho HTTP gerenciado; reinício automático e recuperação de health passaram |
| Encerramento gracioso | Testes diretos SIGTERM/SIGINT passaram; stops gerenciados registraram shutdown requested/application stopped e saída esperada 0; recriação app preservou dados de broker/banco |

Diagnóstico management inicial tentou listar permissões via API e recebeu 401, pois tag management não concede listagem administrativa. `/api/whoami` autenticou corretamente; diagnóstico foi corrigido para validar vhosts visíveis via API e regex efetivas pelo CLI do nó. Nenhuma senha, tag ou dado do broker foi resetado para resolver esse erro de diagnóstico.

**Não testados diretamente:** login pelo formulário do navegador (sem ferramenta de controle do navegador do host), CI remoto GitHub, TLS/HA produtivos, power loss do broker, injeção real de nack, drenagem HTTP além do orçamento Supervisor, carga esgotando admissão de publicação ou morte abrupta do harness. Falha de serialização é categoria defensiva, sem injeção deliberada no envelope validado atual. Verificação HTTP/API não é relatada como login de navegador. Entrega pelo menos uma vez por worker continua incompleta.


## Marco 2.2 — 2026-10-09

Checkout limpo verificado em `5eac9ff`; sem AGENTS.md aplicável ao projeto/ancestrais. Seções anteriores são históricas, não evidência atual. Containers app/PostgreSQL/RabbitMQ já estavam ativos; nenhum start, rebuild, reset ou restart. Sem alteração de credenciais, dependências/lockfile, migrações originais ou dados da aplicação; sem commit/push.

Cargo via `docker compose exec -T app` nas ferramentas Docker existentes (cargo nativo ausente no PATH do host). Rustdoc: `docker compose exec -T -e RUSTDOCFLAGS='-D warnings' app cargo doc --locked --no-deps`.

| Check final realmente executado | Resultado |
| --- | --- |
| cargo fmt --check | Passou após formatação |
| cargo clippy --locked --all-targets --all-features -- -D warnings | Passou, incluindo teste lifecycle ajustado |
| cargo test --locked | Passou: 32 testes; 19 casos opt-in ignorados na invocação padrão |
| cargo test --locked --test delivery_ownership_schema -- --ignored | Passou: 1 caso PostgreSQL isolado da migração |
| cargo test --locked --test postgres_repository -- --ignored --test-threads=4 | Passou: 15 casos existentes isolados |
| cargo test --locked --test rabbitmq_publisher -- --ignored | Passou: 2 casos existentes no broker |
| cargo test --locked --lib infrastructure::rabbitmq::tests::closed_owned_connection_is_unavailable -- --ignored | Passou: 1 caso existente no broker |
| cargo build --locked | Passou |
| cargo doc --locked --no-deps com rustdoc estrito | Passou |
| Links Markdown relativos e git diff --check | Passaram |
| sqlx migrate info somente leitura | Duas originais instaladas; migração de posse pending no banco de desenvolvimento |
| Consultas limitadas somente leitura de catálogo/outbox | 0 bancos relay_it_ restantes; 0 linhas outbox compartilhadas |

Novo caso isolado insere evento no schema antigo, compara todas colunas originais após migração, rejeita cada combinação parcial de posse e token/tempo/estado/contador inválidos, aceita lease coerente, verifica observação pelo reader sem mutação, limpa posse com processed válido, confirma exclusão pelo reader, aplica down e preserva identidade. Harness fecha pools, remove só banco gerado exato e verifica ausência. Migração nova nunca aplicada a dados compartilhados/de desenvolvimento. Suíte PostgreSQL anterior conserva fixtures do schema original. Job CI inclui novo caso isolado; sem execução remota.

Primeira e segunda execuções completas falharam apenas na assertion existente health_and_graceful_shutdown de que conexão TCP síncrona imediata deve falhar após retorno do servidor. Rerun do target passou sem alteração. Sondagem única substituída por verificação assíncrona limitada que exige listener indisponível dentro do mesmo orçamento de três segundos; sem mudança produtiva HTTP. Suíte completa final passou. Causa precisa da observação intermitente do socket não estabelecida; novo check não comprova mecanismo de shutdown não testado.

Cinco testes novos puros/fake verificam duração/token/timestamps, limite exato de expiração, forma terminal, elegibilidade, rejeição de dono antigo/expirado, substituição, repetição rejeitada, contador sem mutação parcial, ID estável e contratos Send/estáticos. Não são evidência de locks/concorrência/durabilidade PostgreSQL. Nenhuma operação produtiva de adquirir/concluir/liberar ou orquestração implementada. Operações, amostra do relógio após espera por lock, donos concorrentes, predicados contra dono antigo, commit incerto e integração concorrente pertencem a 2.3. Worker/recuperação ponta a ponta, desempenho produtivo, perda de energia, Rust nativo e CI remoto continuam não validados. Nenhum check obrigatório do escopo implementado ficou bloqueado.

Check Markdown inicial encontrou quatro links antigos para HANDOFF_MARCO_1.pdf, ausente do checkout/arquivos rastreados. Navegação atual aponta à fonte Markdown histórica existente; registros históricos de geração do PDF preservados.

Extensão final de cobertura diagnóstica: `cargo test --locked --test persistence_contract` passou os 5 casos, incluindo sanitização/fonte de CommitUncertain; formatação e Clippy estrito finais passaram novamente.
