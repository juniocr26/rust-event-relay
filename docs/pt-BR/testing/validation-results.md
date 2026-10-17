[Registro histórico: guia e aceite atuais](../operations/milestone-6-3.md)

[English](../../en/testing/validation-results.md) | [README](../../../README.pt-BR.md)

# Resultados de validação

## Marco 5.3 — medições finitas, verificação completa e fechamento — 17/10/2026

[Protocolo, matriz completa, hashes de rollout e decisão](../operations/milestone-5-3.md) | [22 registros finitos](../../en/testing/milestone-5-3-results.json). Base inicialmente limpa `bde9cba132e8369e71395b7872586fc1a840b08e`; nenhum AGENTS.md aplicável em projeto/ancestrais. Teste/executor/docs/comentário do exemplo locais, sem stage/commit/push. Nenhuma fonte produtiva/dependência/migração alterada. Engineering Library/Viva Bem preservados.

Main remoto exato e [run 39031053588](https://github.com/juniocr26/rust-event-relay/actions/runs/39031053588) conferidos: seis jobs obrigatórios aprovados para 5.2. Rollout nominal deliberado substituiu binários 5.1 carregados por 5.2 compilado locked de fonte limpa. HTTP/relay/consumer RUNNING, hashes carregados/instalados iguais aos selecionados; metrics/ready 200, health 200 ok, replay off. Sem migração compartilhada ou restart de dependências. Diff 5.3 sem CI remoto e sem deploy.

| Check final | Resultado real |
| --- | --- |
| Campanha dedicada | Um teste Rust opt-in com 22 experimentos (três repetições × sete cenários + stale real), aprovado em 100,64s; 1.760 IDs medidos/adquiridos/confirmados/concluídos, 1.520 Applied, 240 Duplicate, 240 cópias exatas de quarentena, 2.000 retornos de ACK; nenhum resto inexplicado |
| fmt / Clippy estrito all-targets/all-features | Aprovados |
| cargo test --locked padrão | 102 passaram, 128 explicitamente ignorados, zero exemplos doc-test |
| PostgreSQL repository / schema ownership / ownership / retry / discovery / replay / consumer / readiness | 15 / 1 / 12 / 18 / 7 / 10 / 12 / 3 passaram |
| RabbitMQ publisher / adapters ignorados | 2 / 2 passaram, incluindo admissão imediata/cancelamento |
| Consumer runtime | 22 entradas: 21 substantivas + uma child/no-op |
| Integrated recovery | 18 entradas: 17 substantivas + uma child/no-op; regressão independente de log/barreira está separadamente na suíte padrão |
| Worker / Supervisor isolado | Cinco substantivos passaram; controles nominais/interativos/recuperação de transporte |
| Total opt-in único sem campanha | 127 entradas: 125 substantivas e duas auxiliares/no-op; sem inflar com repetições de desenvolvimento |
| Três binários / rustdoc estrito | cargo build --locked --bins e RUSTDOCFLAGS=-D warnings cargo doc --locked --no-deps aprovados |
| Frontend em cópia limpa | Node 22.23.3, npm ci pelo lockfile, type-check/build; 13 Chromium no Vite acceptance próprio; capturas mobile/tablet/desktop inspecionadas |
| Migrações / Compose | Seis upgrades/checksums/no-op/down consumer/replay/re-up em banco descartável e ausência após remoção; Compose config --quiet aprovado |
| Operação compartilhada somente leitura | Diagnósticos autenticados DB/broker; seis checksums, filas classic durable/bindings/permissões esperados; sem publish/get/purge compartilhados |
| Scripts / dados | Nove Python analisados; seis shell aprovados em sh -n; JSON finito e hashes fonte/executor final conferidos |
| Documentação | Validador de links locais e quatro regressões aprovados; anchors/remotos/certificação de tradução fora desse checker |
| Git | git diff --check aprovado; sem stage/commit/push |

Falhas de desenvolvimento preservadas: primeira fixture de duplicatas diferia na precisão de timestamp PostgreSQL e foi corrigida antes da medição final; contenção/freshness reforçados antes da campanha final. Primeiro wrapper de verificação impôs RUST_LOG=error, causando 11 falhas de barreira info; corrigido apenas o log de verificação para info e repetida a verificação final com sucesso, sem relaxar assertions/deadlines. Campanha mantém error e comportamento idêntico; hashes distinguem executor medido/final. Nenhum defeito/otimização produtivo inventado. Recursos próprios das execuções malsucedidas também removidos.

Auditoria final: PIDs HTTP 30565, relay 30576, consumer 30589; manager/launchers iguais à fonte, sem restart desde rollout selecionado 5.2. Evento original `481fb751-81d5-46b7-aea2-eed344084725` processed/attempt_count=1, estoque 15, recibo 10+5→15/completed_at 2026-10-16 15:34:23.514788+00. Source/quarentena ready/unacked=0/0, consumers 1/0. Nenhum segredo obrigatório ausente/placeholder; três endereços de exporter ausentes da injeção usam defaults loopback validados. Sem container/rede de medição ou banco/fila de fixture residual; serviços/volumes anônimos próprios removidos; browser removido, capturas/logs só em /tmp, caches de build/browser são artefatos de validação. Dados/credenciais/serviços compartilhados preservados.

**Marco 5 fechado com limitações explícitas não bloqueantes.** Presentation frontend verified; live backend integration is not implemented. Resultados finitos completion-paced não estabelecem capacidade máxima, saturação sustentada, teto global de memória/disco ou prontidão produtiva. Utilização por serviço, percentis produtivos de cauda/recuperação, HA/power-loss e certificação completa de browsers/acessibilidade não medidos. Projeto não declarado concluído; Marco 6 indefinido. Próximo escopo depende de decisão, sem implementação. Este diff de testes/docs/comentário não exige deploy 5.3; mudança futura de runtime exige rollout deliberado.

## Marco 5.2 — validação local e runtime 5.1 — 17/10/2026

[Contrato, orçamentos, hashes do rollout e limites](../operations/milestone-5-2.md). O [run remoto 38974809458](https://github.com/juniocr26/rust-event-relay/actions/runs/38974809458) passou nos seis jobs para `9b0323af3fad769b1d0813e0330b32e87d64a91c` (5.1). Os resultados abaixo do diff 5.2 são locais, sem CI remoto/deploy. Executáveis carregados 5.1 permanecem nos caminhos originais; builds 5.2 usam `/app/target/m52`. Compilador observado: Rust 1.95.0.

Executados: `cargo fmt --check`; `cargo clippy --locked --all-targets --all-features -- -D warnings`; `cargo test --locked` padrão (**102 passaram, 127 explicitamente ignorados, zero falhas**); `cargo build --locked --bins`; `RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps`. Suítes isoladas explícitas: PostgreSQL repository 15, ownership schema 1, ownership 12, retry 18, discovery 7, replay 10, consumer persistence 12; RabbitMQ publisher 2 e adapter 2; integrated recovery 18 (**97 passaram** nesses comandos). Novos testes ignorados operational-readiness: **3 passaram**, cobrindo lock/recuperação/conexão única, deadline total entre statements limitados e HTTP real com replay mantendo liveness durante readiness 503 e recuperando. Consumer runtime: **22 passaram**, incluindo prontidão real sob perda/recuperação de transporte em proxy exclusivo do teste, sem publicar mensagens. Worker/Supervisor: **5 passaram**, incluindo perda/recuperação controlada de transporte do relay e controles nominais/interativos. Inspeção final: nenhum banco/fila de teste residual, PIDs compartilhados 11/12/13 preservados e hashes carregados/arquivos 5.1 iguais aos originais; exporters compartilhados continuam servindo o contrato 5.1.

Suíte padrão cobre starting/unknown/ready, degradação/recuperação de banco e broker, stale/shutdown definitivo, dependência opcional de replay, vocabulário finito, HTTP real, unready acessível versus exporter ausente e requisições repetidas sem fabricar observações de dependência. Sem sampler de backlog: testes de categorias/count ficam adiados com o desenho explícito de consulta/índice/orçamento. Regressões existentes preservam assertions de entrega, recibos, incerteza e quarantine.

O primeiro teste novo de outage do consumer teve corrida ao recriar fila e fechou seu canal observador numa declaração incompatível; cleanup reportou InvalidChannelState. Substituído por chave online/offline em proxy TCP exclusivo do teste; reteste focado e target completo de 22 casos passaram. Removida somente a fixture UUID vazia residual, após verificar zero mensagens/consumers; nenhuma topologia compartilhada removida. Clippy também exigiu separar dois `if` adjacentes no helper; corrigido sem supressões. Tentativas anteriores com falha não são contabilizadas como sucesso.

Passaram Compose config, sintaxe shell dos launchers/scripts, AST Python, 109 arquivos Markdown sem links locais quebrados, quatro testes do validador documental e `git diff --check`. Validador não verifica anchors/links remotos. Sem benchmark, mudança de frontend ou outage compartilhado. Inspeções compartilhadas preservaram seis migrations, demo processada/uma aquisição, estoque 15 e recibo original concluído; seis SHA-384 de migrations coincidem com fonte. Consumer RUNNING explicitamente autorizado. Sem stage, commit ou push.


## Investigação do CI integrated-recovery — 2026-10-16

O log fornecido do [job 116936761544](https://github.com/juniocr26/rust-event-relay/actions/runs/38889957523/job/116936761544), revisão `652bbd21e1cac5dbd4fe587fcf03dcd73a906625`, identifica `committed_schedule_terminal_and_replay_survive_process_kill_with_final_budget_success`: `explicit process barrier: Elapsed(())` na linha 227 do teste. Esperado no primeiro cenário (`scheduled`): o filho confirma o agendamento de retry commitado, escreve o token de prontidão e permanece vivo para SIGKILL. Observado após a barreira de oito segundos: pending, contador 8, posse/expiração/processamento nulos, em vez do agendamento confirmado com contador 9. Saída/erro original do filho foram descartados; a causa interna exata daquela falha remota permanece não confirmada. Download público retornou HTTP 403 e o helper de credenciais configurado retornou HTTP 401; o log fornecido pelo usuário é a evidência remota. O aviso de depreciação de actions não está estabelecido como causa.

Defeito local do harness reproduzido e corrigido: o filho instrumentado de execução única herdava orçamento de 200 ms para conexão/setup do publisher, destinado aos testes de reconexão do worker compilado. Limitando somente o runner exclusivo a 0,25 CPU, o teste afetado falhou com o mesmo snapshot de contador 8. Diagnósticos de fases fixas identificaram saída do filho em `Publisher`, depois fonte tipada Tokio `Elapsed` em `PublisherDeadline`. Ao atribuir ao setup instrumentado o orçamento de oito segundos já existente no harness, o teste passou sob a mesma restrição de 0,25 CPU (9,21 s para os três cenários). Isso fundamenta essa alteração específica de orçamento, sem aumento cego, retry oculto ou afirmação sobre a carga original do CI. Deadlines do worker compilado, limites de prontidão/experimento do pai, prazo de publicação, produção e assertions de confiabilidade permanecem iguais.

A barreira de prontidão agora observa a saída do filho exato além do arquivo ready e informa fase permitida em saída/deadline. Não expõe stderr bruto, fontes dos drivers, credenciais ou payloads. Nova regressão independente de infraestrutura verifica falha de filho encerrado antes do deadline, com fase sanitizada; conteúdo de fase desconhecido não é ecoado. Limpeza exata das fixtures, incerteza tipada, identidade original e assertions de SIGKILL/estado commitado foram preservadas.

Checkout inicialmente limpo em `d1d596e265ec1f8d0ae7c5c6a7064222248241bc`; nenhum AGENTS.md aplicável no projeto/ancestrais. O alvo recovery não mudou desde a revisão com falha. Trabalho posterior de runtime do consumidor/extração do publisher/helpers/workflow foi preservado; revisão com falha exportada para diretório temporário. Antes da correção, execuções locais sem restrição passaram 17/17 nas duas revisões; revisão com falha também passou 17/17 com runner de duas CPUs. Aprovações isoladas não demonstraram ausência de intermitência.

Reprodução com runner/rede exclusivos, PostgreSQL `18.6-bookworm`, RabbitMQ `4.3.6-management`, credenciais descartáveis/banco administrativo/vhost do workflow, Rust `1.95.0`, `--locked --offline` e compilação em diretório separado. Serviços sem volumes compartilhados. Diferenças: Linux local Docker Desktop/Bookworm aarch64, VM de 10 CPUs/aproximadamente 8,3 GB compartilhada pelos containers, serviços via DNS da bridge; CI configura `ubuntu-latest`, execução no host e serviços em loopback nas portas 5432/5672. Hardware/carga reais do CI não foram recuperados. Estresse de 0,25 CPU é experimento diagnóstico, não afirmação sobre recursos do GitHub. Dados compartilhados, linhas de demonstração, estado do consumidor, credenciais e filas não foram alterados. Não se alega execução remota do CI após a correção; Milestone 5.1 não iniciada.

Validação após correção: cenário afetado sob restrição **1 aprovado**, regressão de prontidão independente de infraestrutura **1 aprovado**, alvo recovery completo com gate ignored **17 aprovados**, suíte padrão **88 aprovados / 0 falhas / 120 explicitamente ignorados**, formatação e Clippy estrito para todos os alvos/features aprovados. O ajuste final de limpeza da fixture de regressão foi seguido por seu teste específico e Clippy estrito. Checks documentais: 105 arquivos Markdown, zero links locais sem destino; quatro testes do validador aprovados; `git diff --check` aprovado. Observações finais no banco/broker isolados encontraram zero bancos/filas de fixtures e somente exchanges nativas. Alterações limitadas ao harness recovery e a estes resultados bilíngues; sem staging, commit ou push.

Para excluir influência do dotenv local no startup do worker compilado, a fonte final também foi exportada sem `.env` para diretório temporário: teste afetado aprovado primeiro, seguido do alvo recovery completo (**17 aprovados**, um teste independente de infraestrutura filtrado por `--ignored`). Essa execução adicional trata uma diferença concreta de configuração local/CI. Ausência das fixtures conferida novamente; containers exclusivos, seus volumes anônimos e a rede foram então removidos.

## Marco 4.4 — verificação e fechamento atuais — 16/10/2026

Marco 4 fechado com limitações não bloqueantes explícitas. Evidência real de estoque/deduplicação, ACK/quarentena e recuperação é separada do rollout compartilhado pendente. Frontend de apresentação verificado; integração ao vivo com backend não implementada. Seções históricas abaixo mantêm o escopo das respectivas datas. [Matriz de verificação](../operations/milestone-4-4-review.md) | [Inventário de ambiente](../docker/environment-inventory.md).


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

A divergência de credenciais foi observada diretamente; usuário/senha reais foram omitidos. A validação usou projeto separado `relay-milestone13-validation`, dados em /tmp e loopback 15433. Configuração apenas ilustrativa e função Compose exata estão em [testes](strategy.md#ambiente-isolado-de-validação). Nenhum diretório de dados do desenvolvedor foi excluído. Containers/rede do teste foram removidos; dados temporários permanecem em /tmp. Ambiente original continua executando.

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

[Revisão](../operations/milestone-1-review.md) encontrou três inconsistências documentais médias (status antigo, sugestão de ausência de locks, SQL fonte versus metadados no banco) e duas melhorias baixas de cobertura/validação. Corrigidas nos dois idiomas. Nenhum defeito produtivo confirmado; sem alteração produtiva, dependências, Cargo.lock ou migrações. Caso JSON complexo agora compara fixture inteira explicitamente, normalizando somente representação do expoente. Novo caso ignorado executa fixture SQL existente de 25 assertions no harness isolado e verifica zero linhas após rollback.

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

[Semântica de falhas/transações](../architecture/failure-and-transactions.md) usa documentação oficial PostgreSQL major 18 e SQLx 0.8.6, distinguindo testes, código inspecionado, semântica publicada e inferência futura. ADRs 001-005 preservados sem mudar decisões históricas. READMEs/status, arquitetura, guias, armazenamento/contratos/repositório/testes e revisão reconciliados nos dois idiomas; registros históricos preservados.

`HANDOFF_MARCO_1.pdf` gerado de `docs/pt-BR/operations/handoff-marco-1.md` por `scripts/generate_handoff.py`, com ReportLab em ambiente Python temporário isolado, sem dependências Rust/Dockerfile. Poppler 22.12.0 instalado somente no container de desenvolvimento para QA. Metadados e reabertura pypdf estrita passaram; oito páginas A4 têm texto selecionável e acentos. Todas renderizadas a 110 dpi via pdftoppm e inspecionadas visualmente. Parágrafo de referências inicialmente órfão gerou nona página; tabela condensada, PDF regenerado e layout final de oito páginas conferido. Extração textual, limites de página, numeração, links relativos e whitespace verificados. Base revisada e estado não commitado explícitos; sem credenciais, caminhos pessoais ou logs brutos no handoff.

### Limites da evidência

Job CI dedicado revisado frente ao comando testado e serviço PostgreSQL 18.6; nenhuma execução GitHub remota. Rust nativo no host, Windows, deploy produtivo, benchmark/desempenho, exercícios de backup/restore, limpeza/recuperação após término abrupto, injeção de falhas publisher/ack, prazo de cancelamento, pool esgotado na integração e toda sequência de escritas concorrentes não validados. Nenhum check de aceitação atual bloqueado ou pulado. Limitações futuras não transformam persistência/observação em garantia de entrega funcional; veja matriz de falhas e decisões abertas.

## Marco 2.1 — 2026-10-08

Executado no host Docker Desktop do usuário com Rust 1.95.0, Lapin 4.12.0, RabbitMQ 4.3.6 management e PostgreSQL 18.6 preservado. Implementação direta sem commit/push. Handoff do Marco 1 não foi editado. [Escopo e comandos do Marco 2.1](../architecture/milestone-2-1.md) substituem descrições anteriores de setup/runtime.

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

## Auditoria documental estática — 2026-10-09

Revisados composição HTTP, SQL de leitura, confirmações/incerteza do publisher, modelos/portas de ownership, migração lease e CI. Corrigidas afirmações atuais de dependência de broker ausente/Marco 2 não iniciado; navegação ADR 006/007 e escopo bilíngue alinhados. Mutações SQL, orquestração/polling adiados; sem garantia ponta a ponta. Não executados testes padrão/opt-in, CI, migrações, Cargo, broker ou PostgreSQL. Resultados históricos dos marcos preservados.

Checagens estáticas de links/âncoras, fences, pares de idioma e SHA-256 para alterações somente documentais estão na revisão de entrevista (`engineering-library/docs/pt-BR/testing/verification.md`, external repository). Arquivos reais de ambiente e backups sensíveis não foram lidos/alterados.

## Marco 2.3 — 2026-10-12

Checkout inicial limpo `bbc2c1f`; sem commit/push. Containers app/PostgreSQL/RabbitMQ existentes e saudáveis foram reutilizados sem restart/rebuild/reset. Dependências/publisher compartilhado preservados. Comandos executados via `docker compose exec -T app`, salvo indicação. [Escopo/semântica](../architecture/milestone-2-3.md) e continuidade (arquivo ausente no checkout 2.4) distinguem limites e evidência.

| Check realmente executado | Resultado |
| --- | --- |
| cargo fmt --check | Aprovado |
| cargo clippy --locked --all-targets --all-features -- -D warnings | Aprovado em todos os targets/features finais, incluindo proxy |
| cargo test --locked | 33 não ignorados aprovados; infraestrutura opt-in executada separadamente |
| cargo test --locked --test postgres_ownership -- --ignored --test-threads=4 | 12 casos PostgreSQL reais aprovados |
| cargo test --locked --test postgres_repository -- --ignored --test-threads=4 | 15 regressões existentes aprovadas |
| cargo test --locked --test delivery_ownership_schema -- --ignored | 1 regressão de schema aprovada |
| cargo build --locked | Aprovado |
| RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps | Aprovado |
| Metadados read-only de migração/schema de desenvolvimento e catálogo de fixtures | Só migrações originais instaladas; sem coluna de posse; zero bancos relay_it_ restantes |

Primeiro Clippy falhou por binding de teste escondendo helper request; renomeado. Primeira integração com oito casos passou sete e falhou na prova de início transacional anterior à expiração. Coordenador mantinha snapshot de atividade em cache; pg_stat_clear_snapshot explícito corrige essa leitura de evidência. Suíte corrigida passou e versão final ampliada a 12 casos foi aprovada. Código produtivo não foi alterado para enfraquecer predicado de autoridade.

Testes reais demonstram aquisições concorrentes disjuntas, SKIP LOCKED, rejeição por expiração pós-espera de completion/release, token antigo após recuperação, dado inválido/overflow verificado de duração sem mutação, contador esgotado, envelope/metadados estáveis, limpeza atômica e conflitos repetidos/ausência/terminal. Fixture com timestamp futuro verifica limite adquirido inferior, não reversão real de relógio. Lease de um microssegundo é observado expirado após retorno. Testes determinísticos existentes foram reexecutados para igualdade exata de expiração; isso difere da evidência real ao redor da expiração.

Rejeição SQL imediata e constraint diferida no COMMIT preservam linhas e retornam OperationFailed. Proxy de protocolo de teste descarta resposta de conclusão COMMIT nas três operações; todas retornam CommitUncertain e consulta independente confirma estado durável. Prova um cenário incerto confirmado, sem concluir commit ou rollback de toda incerteza. Cancelamento durante lock wait pré-mutação preserva fixture e pool utilizável. Cancelamento no COMMIT, falha de processo/rede antes de observar conclusão, todos os schedules/isolamentos, justiça, desempenho/perda de energia, Rust nativo e CI remoto continuam não verificados.

Migração de posse aplicada somente em bancos de teste e pendente no desenvolvimento; sem migração nova ou schema alterado. RabbitMQ opt-in não reexecutado: nenhum publisher/dependência compartilhado mudou nem instrução aplicável exigiu isso; testes padrão do publisher passaram. Registros de validação anteriores permanecem históricos.

Escopo documental: READMEs/índices Rust, arquitetura/contratos/falhas, schema/migração/repositório, atualizações de segurança/implantação, estratégia/resultados, página 2.3 bilíngue e handoff em português. Engineering Library mudou somente dossiê/entrevista/índice/verificação existentes bilíngues, preservando revisão histórica e distinguindo princípios gerais de decisões Rust. Resultados de checks documentais finais são acrescentados abaixo.

Checks documentais finais: `python3 -B engineering-library/tools/check_docs.py --workspace .` no workspace irmão aprovado: 212 documentos, zero erros e 18 warnings link-blocked de fixtures preexistentes. Verifica pares de idiomas, índices, links/âncoras e estrutura de entrevistas de Rust/Library; não consulta links externos. Checagem separada de `Reports/MILESTONE_2_3_HANDOFF.md` aprovou todos os links relativos/âncoras, pois Reports fica fora do scan do catálogo. `git diff --check` aprovado separadamente em rust-event-relay e engineering-library. Nenhum teste/runtime de aplicação da Library foi executado; oito arquivos alterados são apenas Markdown.

## Marco 2.4 — 2026-10-12

Início em `edb1f90` limpo; marco anterior já commitado. O handoff 2.3 solicitado está ausente deste checkout. Links quebrados foram corrigidos para registros existentes de marco/validação, preservando o texto histórico de evidência. [Escopo atual](../architecture/milestone-2-4.md) e handoff 2.4 (ausente no checkout 2.5) descrevem implementação e limites.

Containers app/PostgreSQL/RabbitMQ existentes foram inspecionados e reutilizados sem restart/rebuild/reset. Metadados somente de leitura mostram migrações `20261007000000` e `20261007175358` aplicadas com sucesso, sem coluna ownership_token: migração de posse permanece pendente. Nenhuma migração aplicada; eventos e credenciais não foram impressos. Infraestrutura, dependências e schema não mudaram; regressões opt-in reais não foram repetidas sem mudança pertinente.

| Comando executado | Resultado |
| --- | --- |
| `docker compose exec -T app cargo fmt --check` | Aprovado |
| `docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings` | Aprovado |
| `docker compose exec -T app cargo test --locked --test delivery_use_case` | 5 aprovados |
| `docker compose exec -T app cargo test --locked` | 38 aprovados, 31 opt-in ignorados; zero falhas |
| `docker compose exec -T app cargo build --locked` | Aprovado |
| `docker compose exec -T -e RUSTDOCFLAGS='-D warnings' app cargo doc --locked --no-deps --all-features` | Aprovado |
| `python3 -B engineering-library/tools/check_docs.py --workspace .` a partir do workspace pai | 214 documentos, zero erros, 18 warnings preexistentes de fixtures restritos |
| Verificação separada dos links relativos de Reports/MILESTONE_2_4_HANDOFF.md | Aprovada; Reports fica fora do catálogo |
| `git diff --check` nos dois projetos | Aprovado |

Cinco testes com fakes percorrem cinco categorias de publicação, todos os resultados/categorias de erro de transição e falhas de aquisição. Demonstram igualdade completa do envelope original, aquisição/publicação únicas, chave correta, confirmação antes de completion, ausência de cleanup/retry proibidos, preservação de erro/fonte e falhas de release, Debug sanitizado e Send/despacho estático. Notify sincroniza espera pela confirmação e cancelamento durante aquisição/publicação/completion/release. Não comprovam rollback PostgreSQL real, não aceitação RabbitMQ, cancelamento durante commit ou crash/recuperação ponta a ponta. Nenhum teste de aplicação falhou; links documentais incompletos foram resolvidos na redação. Sem commit/push. Próximo: 2.5 composição e operação do worker; 2.6 experimentos ponta a ponta.

## Milestone 2.5 — 2026-10-12

Executado no ambiente Docker existente, a partir de Rust limpo `7be956c` / Library `769bb66`. O handoff 2.4 solicitado estava ausente; API/testes/histórico foram inspecionados. Sem migração compartilhada/reset/mudança de credenciais. Somente imagem app reconstruída e container app recriado with `docker compose build app` / `docker compose up -d --no-deps app`; PostgreSQL/RabbitMQ permaneceram ativos.

| Check executado | Resultado |
| --- | --- |
| cargo fmt --check | Passou |
| cargo clippy --locked --all-targets --all-features -- -D warnings | Passou |
| cargo test --locked | 47 passaram, 34 opt-in ignorados, sem falhas |
| cargo build --locked --bins | Passou, both compiled programs |
| RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps | Passou |
| postgres_repository --ignored --test-threads=4 | 15 passaram |
| postgres_ownership --ignored --test-threads=4 | 12 passaram |
| delivery_ownership_schema --ignored | 1 passaram |
| rabbitmq_publisher --ignored | 2 passaram |
| library closed_owned_connection_is_unavailable --ignored | 1 passaram |
| worker_smoke --ignored --test-threads=1 | 3 passaram: binary delivery/SIGTERM, isolated Supervisor controls, sanitized missing-schema setup |
| python3 scripts/check-supervisor.py inside app | HTTP named/interactive controls, unexpected-exit restart, manager/private socket and relay STOPPED passaram |

Comandos executados com `docker compose exec -T app` e Cargo `--locked`; casos ignored broker/lib selecionados explicitamente. Gate de usabilidade e teste de schema ausente foram validados com checks padrão completos. A consulta final verifica todas as colunas usadas nas operações de posse; smoke de composição/schema, Clippy estrito, formatação, build e rustdoc foram repetidos. Regressões de infraestrutura acima são desta sessão, não resultados históricos. Correções de formatação/lifetime temporário/import não usado foram resolvidas.

`sqlx migrate info` somente leitura antes/depois da recriação app: duas migrações originais instaladas, `20261009000000` ainda pending. Supervisor: http RUNNING / relay STOPPED. Relay compartilhado não foi iniciado; teste negativo e controles individuais/coletivos reais usaram estado próprio isolado. `all` agora inclui ambos; script HTTP compartilhado evita start all.

Nove testes controlados worker/config usam tempo Tokio pausado/notificações: decisões de todos outcomes/releases, tokens novos, pacing, substituição apenas quando indicada, ausência de aquisição sem publisher/com publisher inutilizável, interrupção, shutdown prévio, drain e cleanup limitado. Close fake bloqueado usa mesmo deadline global; não prova todas as situações SQLx reais. Três composições opt-in usam bancos gerados e nomes UUID broker exatos; binário real preserva ID/payload, confirma e reconhece processed com uma aquisição. Child/daemon encerrados antes de deletar queue/exchange e remover banco próprio. Teste Supervisor deriva configuração física versionada, mantém autostart false, HTTP efêmero e controla relay/http/all pelos dois aliases. Evidência Linux local em container, sem novo job Supervisor CI.

Configuração: validação Compose, igualdade instalada/fonte Supervisor, launchers executáveis e inspeção read-only migração/status/probe HTTP. Validador documental do sibling tools verifica 216 documentos, zero erros e 18 avisos existentes de fixtures restritas; links do handoff (relatório 2.5 ausente neste checkout) verificados separadamente. git diff --check passa nos dois projetos. Library alterou apenas Markdown, sem alegar testes de aplicação/runtime ou execução remota GitHub Actions.

Sem alegações de throughput/fairness/efeitos do consumidor/at-least-once ponta a ponta. Happy path, cancelamento fake e controles isolados não provam crash-window/two-worker recovery, escopo 2.6. [Operação/configuração/rollout](../architecture/milestone-2-5.md) detalha budgets/limites. Migração e startup compartilhados aguardam autorização deliberadamente; sem UI/readiness framework/métricas/dashboard.


## Marco 2.6 — 2026-10-12

Partida limpa: Rust `d6f529b`, Library `88f8e62`. Handoff 2.5 ausente não foi reconstruído; código/documentação/histórico versionados forneceram a base. [Matriz e observações](../architecture/milestone-2-6.md), [aceitação](../operations/milestone-2-review.md), handoff (historical handoff unavailable in this checkout) e transcript final sanitizado (historical artifact unavailable in this checkout) registram evidência.

| Comando executado no app existente (`docker compose exec -T app`) | Resultado efetivo |
| --- | --- |
| cargo fmt --check | Passou |
| cargo clippy --locked --all-targets --all-features -- -D warnings | Passou |
| cargo test --locked | 47 passaram, 45 opt-in ignorados, zero falhas |
| cargo test --locked --test integrated_recovery -- --ignored --test-threads=1 --nocapture | 11 entradas passaram, uma helper interna no-op; dez testes de experimento; execução final 27,09 s |
| cargo test --locked --test postgres_ownership -- --ignored --test-threads=4 | 12 passaram |
| cargo test --locked --test postgres_repository -- --ignored --test-threads=4 | 15 passaram |
| cargo test --locked --test delivery_ownership_schema -- --ignored | 1 passou |
| cargo test --locked --test rabbitmq_publisher -- --ignored | 2 passaram |
| cargo test --locked --lib infrastructure::rabbitmq::tests::closed_owned_connection_is_unavailable -- --ignored | 1 passou |
| cargo test --locked --test worker_smoke -- --ignored --test-threads=1 | 3 passaram, inclusive Supervisor físico isolado; 22,81 s |
| cargo build --locked --bins | Ambos os binários compilados |
| cargo doc --locked --no-deps --all-features com RUSTDOCFLAGS='-D warnings' | Passou |

`docker compose config --quiet` passou; Ruby/Psych parseou workflow/quatro jobs. Configuração/launchers Supervisor não mudaram; regressão isolada passou. CI combinado foi configurado, não executado remotamente. Erro intermediário de Clippy por import/helper não usado foi corrigido; nenhum defeito produtivo confirmado. Assertion deliberada de cleanup é esperada; teste externo só passa após verificar ausência independente.

Expiração real, estados duráveis e identidades originais sustentam recuperação nos schedules testados, sem prova universal/exactly-once. Desconexão broker em entrega usa worker real/lifecycle test-only; barreiras de crash usam executável de teste instrumentado com caso de uso/adapters reais. Reinício, concorrência e sinais usam relay original. Proxy de resposta perdida demonstra um completion incerto já commitado. Sem mudança de source/schema/dependências, retry/quarantine/replay duráveis ou negócio produtor/consumidor.

Ambiente final somente leitura: serviços originais continuamente ativos; http RUNNING/relay STOPPED; duas migrações originais instaladas, ownership 20261009000000 pendente. Zero bancos fixture; nenhuma queue/exchange fixture, worker/helper de teste ou diretório relay_recovery_ remanescente. Sem migração compartilhada, restart/recriação, purge ou término de sessão alheia. Cleanup exato também passou após assertion intencional. Rollout permanece sem autorização específica.

Validação documental final: checker read-only da Library, a partir do workspace pai, conferiu 220 documentos, zero erros e 18 avisos existentes de fixtures restritas. Links relativos/âncoras do handoff conferidos separadamente porque Reports não entra no catálogo. git diff --check passou nos dois projetos. Library mudou somente Markdown, sem aplicação/dependências/runtime; sem alteração Viva Bem, commit ou push.


## Marco 3.1 — 2026-10-12

Commit inicial Rust: `15067056f379dd98aeb3bd5c6af09b6700504281`; engineering-library: `7caf977d5f3294c05efa1b29f21edff1b8107ca8`. Árvores limpas; nenhum AGENTS.md aplicável de projeto/ancestral encontrado. Skills disponíveis consideradas; nenhuma skill de artefato/plugin foi necessária para código puro Rust e Markdown.

Executados no container app existente: cargo fmt --all (depois --check), cargo clippy --locked --all-targets -- -D warnings, cargo test --locked, cargo build --locked e RUSTDOCFLAGS=-Dwarnings cargo doc --locked --no-deps. Verificações finais aprovadas: 50 testes passaram, 45 ignorados, nenhuma falha; três testes novos cobrem orçamento/ciclo/capacidade, equal jitter limitado, entradas inválidas, precisão e overflow temporal. Tentativas iniciais de Clippy recusaram cálculo manual de saldo limitado a zero; expressão final limita somente saldo derivado, nunca contadores históricos.

Validador documental executado no workspace abrangendo ambos: zero erros, 18 avisos de links restritos de fixtures Go existentes; nenhum nos repositórios alterados. git diff --check aprovado nos dois. Isso não prova SQL de agenda, concorrência, isolamento ou replay; nada disso foi integrado. Categorias existentes de aquisição/use case/worker revisadas contra a matriz nova.

Consulta somente leitura docker compose ps confirmou app/frontend ativos e PostgreSQL/RabbitMQ saudáveis. Sem consulta de status/migration compartilhada, inspeção/alteração de credenciais, restart, início do relay compartilhado ou reset. Testes opt-in de infraestrutura/recuperação não foram reexecutados pois somente cálculos puros independentes e docs mudaram. Evidência do Marco 2 permanece histórica; CI remota não executada. Sem staging, commit ou push.

[Política e próximos marcos](../architecture/milestone-3-1.md) | [ADR 008](../adr/008-durable-failure-policy.md).

## Marco 3.2 — 12/10/2026

Bases: rust-event-relay `2257d02543958eff5851a49d7be16887c0b68d6a`; engineering-library `d9f6e556258a9a1739dcb9ad9df7f835f4511853`. Árvores Git limpas na entrada; nenhum AGENTS.md aplicável no projeto/ancestrais. ADRs/política/migrations/modelos/SQL de ownership/harness/proxies/regressões atuais foram inspecionados. Instruções Dev Career Copilot e metodologia documental existente consultadas; fontes PostgreSQL verificaram NULL em CHECK e semântica temporal.

Checks executados localmente no app existente; versões finais passaram:

- cargo fmt --check; cargo clippy --locked --all-targets --all-features -- -D warnings.
- cargo test --locked: 53 aprovados, 63 casos opt-in ignorados, nenhuma falha.
- cargo test --locked --test postgres_retry -- --ignored --test-threads=4: 17 aprovados.
- Regressões PostgreSQL: postgres_ownership 12, postgres_repository 15 opt-in, delivery_ownership_schema 1; todas aprovadas em bancos isolados. Caso independente do repositório também passou na suíte padrão.
- integrated_recovery: 11 aprovados (10 experimentos e entrada interna child sem ação), sequencialmente em bancos/recursos de broker exclusivos do teste.
- worker_smoke: três casos originais aprovados, inclusive controle coletivo Supervisor isolado; novo preflight sem schema retry aprovado. Ambos os preflights foram conferidos novamente após ajuste do helper. Nenhuma inicialização compartilhada.
- cargo build --locked --bins; RUSTDOCFLAGS=-Dwarnings cargo doc --locked --no-deps.
- Validador bilíngue: 230 documentos no workspace configurado, zero erros, 18 avisos preexistentes de fixtures Go restritas; nenhum finding nos dois repositórios alterados. git diff --check aprovado em ambos.

Rodadas iniciais identificaram organização de itens no Clippy e premissas antigas de fixtures sobre revisão/metadados terminais. Corrigidas sem remover assertions existentes. Backfill terminal legado passou a registrar observação pela migration, sem inventar horário histórico. Testes finais verificam esse instante e preservação de todos os campos originais. Experimentos de incerteza/cancelamento reutilizam proxy existente com barreira de COMMIT do servidor; efeito durável observado não permite generalizar toda incerteza como sucesso.

Inspeção somente leitura confirmou bind app `/app`, bind PostgreSQL `/var/lib/postgresql`, volume RabbitMQ preservado `/var/lib/rabbitmq` e bind de configuração. Histórico SQLx compartilhado mantém somente `20261007000000` e `20261007175358`, ambas aprovadas; outbox compartilhada vazia e relay STOPPED. Ownership/retry continuam pendentes nesse banco; up/down executados somente em bancos isolados. Catálogo não tinha bancos de teste remanescentes na inspeção. Sem reset, migration compartilhada, mudança de credencial broker, restart da aplicação ou ativação do relay neste marco. Diretórios fonte Supervisor corrigidos de `/app/Backend` para `/app/backend`; lifecycle isolado validou fonte sem reconstruir/reiniciar app compartilhado.

CI inclui novo target PostgreSQL, mas não foi executada remotamente. Integração da política, scanner poison geral, replay autenticado/auditado e fechamento integrado permanecem 3.3/3.4/3.5/3.6. Sem stage, commit ou push. Relatório temporário de continuação retornado no chat, sem salvar nos repositórios.

[Implementação e rollout](../architecture/milestone-3-2.md).

## Marco 3.3 — 2026-10-13

Base Rust `e6f8a2f44d9ac250e1bcdf5f0b6a3b5b7bd5f1eb`; Library `303ed8602aa5727e2a47bf02a2ef7f0856e21760`, ambos inicialmente limpos. Sem AGENTS.md aplicável ou relatório anterior fornecido. Evidência histórica acima não conta como execução atual.

Executado no contêiner app existente (Rust 1.95.0), com bancos UUID do harness, topologia única de broker e processos/proxies próprios:

- Testes padrão: 60 passaram; opt-in executados separadamente abaixo. Casos controlados de entrega: 8; worker: 12; política de retry: 4. Sem estatística frágil para jitter.
- PostgreSQL retry: 18 passaram, incluindo publisher Rejected injetado em deliver_once com perda real de reconhecimento COMMIT em agendamento/terminalização. O publisher é fake nessas experiências, não NACK real.
- Recuperação integrada: 12 casos reais e entrada interna boundary_child, no-op sem RECOVERY_CHILD. Inclui rota real ausente → agendamento → aquisição antecipada negada → rota corrigida/token novo/identidade original concluída; falha final terminal; manutenção configurada protegendo dono ativo e entrega independente.
- Ownership PostgreSQL: 12; repository: 15 opt-in; schema ownership: 1; publisher RabbitMQ: 2; worker smoke: 4; regressão de conexão fechada: 1; todos passaram. Total opt-in: 65 casos substantivos e entrada interna no-op.
- cargo fmt -- --check, Clippy estrito em todos os targets, build dos dois binários e rustdoc com -D warnings passaram.
- Testes do validador documental: 18 passaram. Validador documental final somente leitura: 232 documentos, zero erros e 18 avisos existentes de fixtures restritos; git diff --check passou nos dois repositórios.

Correções durante o trabalho: Clippy detectou escopo de MutexGuard de teste atravessando await; teste agora copia as observações limitadas antes de suspender. Scan documental intermediário encontrou quatro anchors novos ausentes enquanto estas entradas eram escritas; validação final inclui os anchors. Nenhum defeito confirmado exigiu mudança de SQL/schema/protocolo. Contagens anteriores de recuperação permanecem; novo caso agendado usa duas aquisições comprometidas porque a primeira falha definitivamente e a segunda conclui após elegibilidade durável.

Inspeção somente leitura: PostgreSQL/RabbitMQ saudáveis; HTTP RUNNING, relay compartilhado STOPPED/Not started. sqlx migrate info: somente 20261007000000/20261007175358 instaladas; ownership 20261009000000 e retry 20261012000000 pendentes. Nenhuma migration compartilhada aplicada, serviço compartilhado parado/iniciado/reconstruído, credencial ou volume alterado. Migrations aplicadas apenas nos fixtures isolados. CI remoto não executado. Sem staging/commit/push. Library somente documentação; Viva Bem inalterado.

Limites: Serialization genérico não distingue evento/encoder global e retém ownership conservadoramente. Manutenção finita configurada não descobre candidatos/cobre backlog geral, pausa sem publisher utilizável e não tem cursor durável após restart; remover configuração antiga e reobservar metadados. Sem reconciliação genérica ou idempotência de consumidor. [Contratos e escopo exato restante de 3.4](../architecture/milestone-3-3.md).

Comandos executados:

```bash
docker compose exec -T app cargo test --locked
docker compose exec -T app cargo test --locked --test postgres_retry --test postgres_ownership --test postgres_repository --test delivery_ownership_schema --test rabbitmq_publisher --test worker_smoke --test integrated_recovery -- --ignored --test-threads=4
docker compose exec -T app cargo test --locked --lib closed_owned_connection_is_unavailable -- --ignored
docker compose exec -T app cargo test --locked --test integrated_recovery configured_worker_maintenance -- --ignored --nocapture
docker compose exec -T app cargo clippy --locked --all-targets -- -D warnings
docker compose exec -T app cargo build --locked --bins
docker compose exec -T app sh -c 'RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps'
docker compose exec -T app cargo fmt -- --check
python3 -B ../engineering-library/tools/check_docs.py --workspace ..
python3 -B -m unittest discover -s ../engineering-library/tools -p 'test_check_docs.py' -v
git diff --check
git -C ../engineering-library diff --check
```

## Rollout local autostart — 2026-10-13

Bases Rust cae62cc654efc0b411408dd84a0b92a57738eb38 e Library c272476429fe4c9223f274d74293edeabb8c79b5, ambos limpos. Usuário autorizou migração/ativação locais explicitamente. Supervisor/Compose/adaptador/preflight e SQL das duas migrations aditivas revisados. Outbox compartilhado tinha zero linhas/esgotados; somente migrations iniciais instaladas. Management host autenticado com credenciais atuais sem divulgação.

sqlx migrate run explícito aplicou 20261009000000 e 20261012000000; quatro migrations instaladas/bem-sucedidas, nove colunas retry presentes, outbox ainda vazio. Apenas app reconstruído/recriado por docker compose up -d --build --no-deps app; PostgreSQL/broker/frontend existentes, credenciais/storage preservados. Sem migrations automáticas, reset, exclusão de volumes, purge ou publicação de demonstração. Diff frontend vazio.

Executado: startup automático http/relay RUNNING; página/API management autenticadas localhost:15672 e exchange/fila/binding duráveis configurados; controles diretos/interativos por supervisorctl/supervisor; stop relay manteve parado e preservou HTTP/management/metadados/contagens; start retomou; SIGKILL de cada filho ativo provocou recuperação de saída inesperada. Limites de shutdown preservados. Fila compartilhada observada com zero ready/unacknowledged; persistência de payload não vazio não foi testada inventando mensagens. Formulário login do navegador não operado.

Clippy estrito em todos os targets e fmt check passaram; quatro worker_smoke isolados passaram, inclusive autostart pela configuração física e recursos reais próprios de banco/broker. Suite padrão: 60 passaram. Validador documental final somente leitura: 234 documentos, zero erros e 18 avisos existentes de fixtures restritos; git diff --check passou em ambos. Management/API e HTTP do host também reverificados durante stop intencional; relay restaurado para RUNNING. Execuções integradas anteriores de 3.3 mantêm escopo histórico próprio; CI remoto não executado. Sem staging/commit/push. [Instruções locais atuais](../operations/local-relay-rollout.md).

Comandos executados:

```bash
python3 scripts/check-rabbitmq.py
python3 scripts/check-rabbitmq.py --require-topology
docker compose exec -T app sqlx migrate info
docker compose exec -T app sqlx migrate run
docker compose up -d --build --no-deps app
docker compose exec -T app python3 /app/scripts/check-supervisor.py
docker compose exec -T app supervisorctl stop relay
docker compose exec -T app supervisorctl start relay
docker compose exec -T app supervisor status
docker compose exec -T app cargo clippy --locked --all-targets -- -D warnings
docker compose exec -T app cargo test --locked --test worker_smoke -- --ignored --test-threads=4
docker compose exec -T app cargo test --locked
python3 -B ../engineering-library/tools/check_docs.py --workspace ..
git diff --check
git -C ../engineering-library diff --check
```


## Marco 3.4 — validação local executada, 13/10/2026

Bases Rust a756797bbdd64d50671d536ae7de697a3705ce8b e Library c272476429fe4c9223f274d74293edeabb8c79b5. Rust inicialmente limpo; oito arquivos documentais da Library já modificados, preservados. [Detecção, autoridade e limites](../architecture/milestone-3-4.md).

Passaram: suíte padrão final all-targets com 65 testes, incluindo três de manutenção determinística e 14 do worker. PostgreSQL real isolado: descoberta 7, retry 18, ownership 12, reader 15 e migration de ownership 1. Smoke worker/Supervisor: 4. Target integrado: 15, incluindo casos auxiliares boundary-child/harness. Os dois casos novos de descoberta/publicação também foram reexecutados com os binários finais após refatoração equivalente da checagem de identidade e guarda de admissão na aplicação. Clippy estrito all-targets, rustdoc com -D warnings, build dos dois binários e formatação final passaram. Target novo incluído na configuração CI; CI remoto não executado. Comandos exatos disponíveis na [versão inglesa](../../en/testing/validation-results.md).

Evidência nova: mistura de inválidos/orçamento/capacidade/válidos, conteúdo/histórico preservados, cursor/limite superior/retorno ao início, inserção/alteração concorrente, avanço após prefixo inseguro, aquisição por ID revalidada, salto de lock, isolamento concorrente, ciclo/revisão obsoletos, reparo válido sem incrementar revisão, dono final ativo/conclusão, dono expirado, schema/timestamp não suportados, revisão/ownership inseguros, pool fechado, rejeição diferida de commit, perda de acknowledgement e cancelamento pós-commit com observação durável independente. Testes determinísticos verificam ausência de mutação em observação/erro, consumo do cursor, manutenção durante outage do broker, pausas interrompíveis e ausência de rajada compensatória. Worker real deixa identidade nil pending/intacta, isola poison com espaços e entrega ID válido original; banco e mensagem do broker observados separadamente. Constraints removidas somente da fixture malformada no banco descartável exato; produção intacta.

Checks operacionais somente leitura: Compose válido; http/relay ainda RUNNING; PostgreSQL saudável/autenticado e configuração em execução coincide com Compose. Quatro migrations existentes bem-sucedidas, inclusive 20261009000000 e 20261012000000. Sem migration nova/compartilhada, reinício, mudança de credenciais, reset de storage ou purge de fila compartilhada. Containers existentes reutilizados. Frontend e Viva Bem intactos. Sem stage/commit/push. Comportamento novo validado em workers isolados; relay compartilhado já em execução não foi reiniciado para carregá-lo.

Validador documental: 236 documentos, zero erros, 18 avisos anteriores de fixtures restritas; seus 18 testes unitários passaram. git diff --check passou nos dois projetos. Schedules finitos não provam justiça/cobertura completa, limite de bytes, perda de energia, toda concorrência, exactly-once ou idempotência de consumidor. Replay auditado e fechamento continuam 3.5/3.6.

## Marco 3.5 — 13/10/2026

[Checks executados e limites operacionais](../architecture/milestone-3-5.md#validação-executada). Migration 5/ativação privilegiada compartilhada pendentes; testes isolados de banco/broker não provam efeito de negócio de consumidor nem execução remota de CI.


## Aceitação atual do Marco 3.6 — 13/10/2026

[Auditoria completa, matriz rastreável, contagens executadas, lacunas operacionais e leitura guiada](../operations/milestone-3-review.md). Marco 3 fechado com limitações explícitas não bloqueantes; rollout replay compartilhado, idempotência consumer, frontend/backend real e prontidão produtiva permanecem separados. Seções anteriores preservam o escopo histórico datado.


## Marco 4.1 — 13/10/2026

Container app existente, CARGO_TARGET_DIR separado em /tmp/relay-m41-target: cargo fmt --check; cargo clippy --locked --all-targets --all-features -- -D warnings; cargo test --locked (75 aprovados, 0 falhas, 88 ignorados, incluindo seis testes puros novos); cargo build --locked --bins; RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps. Validador documental bilíngue: 0 erros, 18 avisos de fixtures restritas do projeto Go. git diff --check passou nos dois repositórios.

Testes cobrem limites de identificadores/quantidade/estoque, tipo/versão, identidade coerente, comparação imutável, política de ACK, limites corpo/propriedades e duplicação de quarentena após crash/ACK incerto simulados. Recorder é fake: não prova atomicidade PostgreSQL, preservação RabbitMQ, correlação return/confirm ou recuperação real de crash. Suítes de infraestrutura permanecem ignoradas, não executadas: sem alteração em adaptador/migration/runtime. Docker consultado/reutilizado sem restart, consumo compartilhado, topologia/credenciais ou migrations. Engineering Library somente documentação. [Contratos e experimentos reais planejados](../architecture/milestone-4-1.md).

## Marco 4.2 — execução local real, 13/10/2026

Base Rust `4d3734c79b3d635a91a4e7d415a903b18b43a610`; Library `281231333240e20a9704e15bb9fbcdaeb5ed151c`; ambos inicialmente limpos. App Docker existente usa Rust 1.95.0, SQLx fixado 0.8.6 e PostgreSQL 18.6. Sem rebuild/restart de serviço, escrita compartilhada ou consumo.

- `cargo fmt --check`: passou no código final.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: passou no código final.
- `cargo test --locked`: suíte padrão passou; testes de banco/broker conservam gating ignored explícito.
- `cargo test --locked --test postgres_consumer -- --ignored --test-threads=1`: **10 passaram**, zero falhas. Inclui disputa observada de estoque/identidade, preservação de envelope/histórico relay no upgrade, comparação de nanos/offset UTC, constraints/corrupção somente de teste, rejeição de comando/commit diferido, rollback, overflow, duplicata histórica, proxy de perda de COMMIT e cancelamento.
- Regressões isoladas afetadas: postgres_repository **15**, delivery_ownership_schema **1**, postgres_ownership **12**, postgres_retry **18**, postgres_discovery **7**, postgres_replay **10** passaram (63 no total). Caso opt-in existente do repository continua filtrado pelo gating.
- `cargo build --locked --bins`: binários HTTP e relay compilados. `RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps`: passou.
- `python3 scripts/check-migrations.py`: seis upgrades/checksums SQLx ordenados, no-op, down/re-up de consumidor e replay passaram; banco gerado removido e ausência verificada. Down/up SQL de consumidor também testado com estoque/recibos, comprovando perda. Harness verifica cleanup individual de cada banco exato.
- Validador Engineering Library: zero erros, 18 avisos existentes de links restritos de fixtures; seus 18 testes sintéticos passaram. Sintaxe Python do checker passou. `git diff --check` passou nos dois repositórios.

CI configurado para executar a suíte de consumidor no job PostgreSQL; nenhuma execução remota alegada. Frontend/broker/aceitação integrada de crash não mudaram nem foram reexecutados. Inspeção compartilhada somente leitura mostra quatro versões instaladas (20261007000000, 20261007175358, 20261009000000, 20261012000000); replay 20261013000000 e consumidor 20261014000000 permanecem pendentes. Supervisor HTTP PID 11 e relay PID 12 permaneceram ativos; consulta de catálogo retornou zero bancos relay_it_/relay_migration_ restantes. Sem staging, commit ou push.

[Transação, justificativa do trigger, limites de incerteza e pré-requisitos exatos de 4.3](../architecture/milestone-4-2.md). Cancelamento/falhas de protocolo são evidência local finita, sem provar processo morto, perda de energia, throughput ou prontidão produtiva. Validação anterior conserva caráter histórico.


## Revalidação da 4.2 e lacunas confirmadas — 16/10/2026

Base Rust `652bbd21e1cac5dbd4fe587fcf03dcd73a906625`; Engineering Library `a25591eaf04a7f95b1769b7dbc9d6b7b403945a5`. Ambos os working trees rastreados começaram limpos. Nenhum AGENTS.md aplicável encontrado no projeto/ancestrais. Lidos ADR 010, contratos/testes 4.1, envelope canônico, implementação/schema 4.2 existentes, harness/proxies isolados e fonte transacional do SQLx 0.8.6 fixado. Referências PostgreSQL 18 de INSERT/isolamento/triggers foram reconferidas. Schema/representação/transação reutilizados; corrigidos descarte de causas tipadas e ausência de Unavailable, e fortalecido teste de completude para excluir rejeição apenas por FK.

Executado no container app existente (Rust 1.95.0), com `CARGO_TARGET_DIR=/tmp/relay-m42-review-target` separado:

- Formatação e Clippy estrito (`--locked --all-targets --all-features -- -D warnings`): aprovados, incluindo fonte final.
- `cargo test --locked`: **76 aprovados**, 0 falhas, **99 ignorados** pelos gates explícitos de integração. Inclui sanitização/causas tipadas e decode da representação.
- Suíte PostgreSQL do consumidor (`--ignored --test-threads=1`): **11 aprovados**, 0 falhas. Preservados casos de falhas/incerteza/concorrência; adicionados pool fechado/sem mutação e assertions específicas de causas/completude.
- Suítes isoladas existentes: postgres_repository **15** (um caso opt-in filtrado pelo gate), delivery_ownership_schema **1**, postgres_ownership **12**, postgres_retry **18**, postgres_discovery **7**, postgres_replay **10**: **63 aprovados**, 0 falhas.
- `cargo build --locked --bins`: ambos os binários compilados. Rustdoc estrito (`RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps`): aprovado na fonte final.
- Checker de migrations: seis upgrades/checksums SQLx ordenados, no-op e down/re-up de consumidor/replay aprovados em banco descartável exato; remoção e ausência verificadas. Nenhuma migration alterada. Harness verifica limpeza de cada banco exato; observação final do catálogo encontrou zero bancos relay_it_/relay_migration_.
- Validador documental bilíngue: **250 documentos, 0 erros, 18 avisos preexistentes de fixtures restritas**. Seus **18** testes sintéticos passaram. `git diff --check` aprovado nos dois repositórios.

A CI já configura postgres_consumer no job PostgreSQL; nenhuma alteração ou execução remota foi necessária/alegada. HTTP PID 11 e relay PID 12 continuam RUNNING, separados dos artefatos compilados em /tmp; nenhum processo foi reiniciado para carregar estas mudanças. Serviços PostgreSQL/RabbitMQ/frontend reutilizados sem reset, troca de credenciais ou consumo compartilhado. Ledger compartilhado permanece com quatro migrations até 20261012000000; replay e consumidor continuam pendentes. Frontend e Viva Bem intocados. Sem staging, commit ou push. Engineering Library mudou somente documentação. Relatório de continuação entregue no chat, sem arquivo salvo.

O [protocolo e exemplo com tabelas atualizados](../architecture/milestone-4-2.md#revalidação-e-correção-dos-diagnósticos--16102026) mantêm esperas limitadas configuradas pelo chamador, ausência de retries ocultos, resultado histórico de duplicata e recuperação de commit incerto. Esta evidência não demonstra encerramento real de processo consumidor/redelivery, durabilidade após queda de energia ou rollout de produção. Runtime de ACK manual/quarentena permanece 4.3; crash integrado/incerteza de ACK permanece 4.4.


## Marco 4.3 — execução local efetiva, 16/10/2026

Base Rust `652bbd21e1cac5dbd4fe587fcf03dcd73a906625`, Library `a25591eaf04a7f95b1769b7dbc9d6b7b403945a5`. Já existiam edições da 4.2: cinco arquivos Rust de fonte/testes e quatro documentos bilíngues de arquitetura/validação; Library tinha dois documentos de arquitetura alterados. Foram preservadas. Nenhum AGENTS.md aplicável encontrado no projeto/ancestrais. Inspecionados contratos/ADR atuais, schema/adapter/testes PostgreSQL, publisher/lifecycle, Supervisor, harness/proxies e fonte Lapin 4.12.0. [Manual 4.3 e escopo restante exato](../architecture/milestone-4-3.md).

Reutilizados app/PostgreSQL/RabbitMQ Docker vivos, Rust 1.95.0, SQLx 0.8.6 e Lapin 4.12.0 fixados. Artefatos em `/tmp/relay-m42-review-target` separado, sem substituir artefatos `/app/target` dos processos vivos.

- `cargo fmt --check` e `cargo clippy --locked --all-targets --all-features -- -D warnings`: aprovados na fonte/testes finais.
- `cargo test --locked`: **87 aprovados**, 0 falhas, **109 ignorados** pelos gates explícitos. Inclui sete testes novos de aplicação e quatro novos de propriedades/configuração/pacing AMQP.
- `cargo test --locked --test consumer_runtime -- --ignored --test-threads=1`: **10 aprovados**, 0 falhas. Efeitos/histórico/conflito/bytes inválidos; mandatory return; confirm de quarentena perdido com original retido e cópia duplicada; rejeição/indisponibilidade DB; COMMIT sem resposta seguido de duplicata histórica; ACK original inutilizado; binário com prefetch 2 (uma espera SQL ativa, uma entrega em buffer, terceira pronta) e recuperação após TERM/restart; cancelamento do servidor/reassinatura/shutdown ocioso; declaração incompatível sem substituição destrutiva; schema ausente falha antes da assinatura; Supervisor isolado opt-in/start/stop nomeado.
- Regressões isoladas afetadas: postgres_consumer **11**, rabbitmq_publisher **2**, worker_smoke **5**, integrated_recovery **17**: **35 aprovados**, 0 falhas. Regressão Supervisor usa controles HTTP/relay nomeados, preservando consumer desativado. Testes existentes de publisher/worker/crash mantêm escopo dos marcos anteriores; não encerram a 4.4 do consumidor.
- `cargo build --locked --bins`: **três binários** compilados (HTTP, relay, consumer). `RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps`: aprovado. Alterações finais posteriores foram apenas assertions/cobertura de testes.
- Parse Compose somente leitura: válido, defaults limitados, grace app 95s; credenciais resolvidas omitidas. Checks Supervisor fonte: autostart=false só no consumer, autostart HTTP/relay preservado, grupos, autorestart inesperado, parada 30s e launcher executável. Sintaxe shell do launcher/entrypoint e AST dos scripts Python aprovadas. check-supervisor.py compartilhado não executado por controlar processos vivos; controles afetados equivalentes testados em instâncias isoladas.
- Validação documental bilíngue: **252 documentos, 0 erros, 18 avisos preexistentes de fixtures restritas**; **18** testes sintéticos do validador aprovados. `git diff --check` aprovado nos dois repositórios.

Novos testes possuem filas/exchanges UUID exatos e bancos descartáveis; remoção do broker é conferida por declaração passiva **404**, não por erro genérico de conexão. Harness verifica remoção de cada banco exato. Observação final: **zero** bancos relay_it_/relay_migration_ e **zero** filas/exchanges relay.it. Filhos consumer/Supervisor dos testes receberam TERM e foram aguardados; sinais nunca atingiram processos compartilhados. Sem publicação, consumo ou purge de mensagens compartilhadas.

Observação compartilhada final: HTTP PID 11 e relay PID 12 seguem RUNNING. Consumer não foi instalado/recarregado/iniciado nesse manager; configuração fonte mantém autostart=false. relay.local continua classic durável, x-queue-type=classic, sem auto-delete e com zero consumidores; binding event existente e ausência de política efetiva DLX/TTL inspecionados somente leitura. Quatro migrations aplicadas até 20261012000000; replay/consumer pendentes. Sem nova migration, reset de volumes, troca de credenciais, restart de serviços ou mutação de topologia compartilhada. Frontend/Viva Bem intocados. Sem staging, commit ou push.

CI agora configura a suíte no job combinado PostgreSQL/RabbitMQ, excluindo apenas o teste específico de Supervisor (não instalado no job): nove casos configurados. Não se alega execução remota. Execução local completa dos dez inclui Supervisor. Aceitação ampliada 4.4 de crash/redelivery/ACK perdido/capacidade e rollout compartilhado autorizado continuam pendentes. Relatório de continuação em português entregue somente no chat, sem arquivo salvo/rastreado.


## Marco 5.1 — validação executada da implementação

Base `05f0ebbb84722a6bf4e3b68f695d726cc2dd7df3`, árvore inicial limpa. [Implementação, matriz, métricas e lacunas](../operations/milestone-5-1.md). Reutilizados serviços Docker sem restart/migration compartilhados; testes usam bancos descartáveis/topologias UUID. Frontend não mudou e não foi validado novamente nesta sessão.

| Check local executado | Resultado |
| --- | --- |
| `cargo fmt --check`; Clippy estrito `--locked --all-targets --all-features -- -D warnings` | Passaram na fonte final |
| `cargo test --locked` | 99 passaram, 123 ignorados, zero falhas; ignorados não são evidência de execução padrão |
| `cargo build --locked --bins`; rustdoc com `RUSTDOCFLAGS='-D warnings'` / `--no-deps` | Passaram; três binários |
| `integrated_recovery` completo / ignored / uma thread | 18 passaram: 17 substantivos + entrada interna de filho; preservada correção de orçamento/diagnóstico |
| `consumer_runtime` completo / ignored / uma thread | 21 passaram: 20 substantivos + entrada interna; oversize real, perda de ACK/COMMIT, crash, quarentena e métricas reais |
| PostgreSQL consumer / discovery / ownership / replay / retry | 12 / 7 / 12 / 10 / 18 passaram |
| RabbitMQ publisher | Dois passaram: retorno mandatory, confirm perdido/cancelamento |
| Conexão própria e admissão real publisher (lib) | Dois passaram: classificação de conexão fechada, saturação, liberação no cancelamento e retirement; admissão configurada também no CI RabbitMQ |
| Worker smoke / Supervisor isolado | Cinco passaram; fixture final exclui consumer e usa portas de métricas próprias |
| Endpoints dos processos compilados finais | Relay: um; consumer/exporter e prefetch: dois passaram novamente após correção de consistência dos histogramas; HTTP real verificado pelo lifecycle padrão |
| Exporter, contratos e observações semânticas | Incluídos na suíte padrão: colisões/cardinalidade/quatro clientes/shutdown/rebind, bytes/escapes/profundidade/nós, admissão replay/drop/erro, Applied/Duplicate, quarentena falha, aquisição e transições duráveis incertas |
| Compose, links, testes do validador e `git diff --check` | Passaram: 107 Markdown / quatro testes; sem validar âncoras ou URLs remotos |

O teste novo de métrica terminal inicialmente configurou count 12, mas o ciclo do fixture esgotava em baseline 3 + maximum 10 = 13. Corrigido para derivar a fronteira exata; assertion e política de produção preservadas. Teste focado e suíte final passaram. Um filtro --exact incompleto selecionou zero testes; não foi contado como evidência e foi substituído pelo nome completo. Nenhum timeout de produção foi ampliado ou assertion de confiabilidade enfraquecida. Histogramas usam locks locais curtos para coerência interna de buckets/count/sum; fases distintas continuam sem snapshot atômico. Fixture Supervisor de worker exclui consumer, sem depender de schema ausente para impedir assinatura.

Comandos executados no app existente, em `/app/backend`:

```sh
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked
cargo test --locked --test integrated_recovery -- --ignored --test-threads=1
cargo test --locked --test consumer_runtime -- --ignored --test-threads=1
cargo test --locked --test postgres_consumer --test postgres_discovery --test postgres_ownership --test postgres_replay --test postgres_retry --test rabbitmq_publisher -- --ignored --test-threads=1
cargo test --locked --lib infrastructure::rabbitmq::tests:: -- --ignored --test-threads=1
cargo test --locked --test worker_smoke -- --ignored --test-threads=1
cargo test --locked --test integrated_recovery --test consumer_runtime compiled_ -- --ignored --test-threads=1
cargo build --locked --bins
RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps
```

Na raiz: `docker compose config --quiet`, `python3 -B scripts/check-docs.py`, `python3 -B -m unittest discover -s scripts -p 'test_*.py'`, `git diff --check`. O [run remoto 38970986836](https://github.com/juniocr26/rust-event-relay/actions/runs/38970986836) passou todos os jobs na **base**, conforme API GitHub. Este diff sem commit não foi executado remotamente. Logs do job antigo retornaram HTTP 403; assertion/causa original seguem desconhecidas. Cenários reais atuais de recovery passaram localmente; não é reprodução de uma falha histórica desconhecida no Ubuntu.

Inspeção final somente de leitura: seis migrations bem-sucedidas; mesmos PIDs consumer=11/HTTP=12/relay=13 RUNNING; health 200 e replay 404; sem listeners compartilhados de métricas (sem rollout). Demo processed/count 1, estoque 15 e exatamente um recibo 10/5/15. Origem ready=0/unacked=0/consumers=1, quarentena 0/0/0. max_message_size broker=16777216 bytes. Nenhum banco relay_it_ ou fila/exchange relay.it. restante. Preservadas credenciais, topologia/volumes/linhas compartilhadas. Testes de trabalho limitado não provam teto de memória ou capacidade medida. Readiness/backlog pertencem a 5.2; medição reproduzível de capacidade/backpressure/recovery a 5.3.
