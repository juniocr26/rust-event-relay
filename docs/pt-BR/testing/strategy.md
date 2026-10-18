# Verificação de rotina

No host, a partir da raiz do repositório:

```sh
docker compose exec app tests
```

Dentro de app, em qualquer diretório: `tests`. Na automação: `docker compose exec -T app tests`.

O comando prepara automaticamente dependências npm travadas e Chromium. Executa formatação/lint/testes/binários/documentação Rust, links da documentação e testes das ferramentas Python, tipos/build/browser do frontend, integrações reais PostgreSQL/RabbitMQ/processos, upgrades/rollbacks SQLx em banco descartável e os cenários reais normal/duplicado/concorrente/recuperação no navegador com processos próprios, bancos descartáveis e namespaces de broker na infraestrutura existente. O browser também verifica retomada do consumer pausado, HTTP indisponível e inspeção de ação desconhecida sob Supervisor isolado. Não migra o banco principal nem para os processos gerenciados da aplicação.

A primeira execução baixa pacotes/browser e compila Rust. Os artefatos de teste ficam em `target/tests`; os serviços usam `target/runtime`. Caches npm/browser e a cópia isolada do frontend ficam em `/tmp/relay-tests`, reutilizados até recriar o container. O node_modules do serviço frontend não é modificado. Somente uma execução simultânea é permitida. Uma falha obrigatória interrompe a sequência; o resumo lista apenas fases concluídas. Códigos: 0 sucesso, 1 falha, 130 interrupção. Os diagnósticos mostram a parte final sanitizada e salvam a última fase com falha em `/tmp/relay-tests/last-failure.log` dentro de app; Ctrl+C sinaliza o grupo de processos próprio e permite limpeza antes de encerramento forçado. Falha de limpeza é reportada como falha.

Aceitação separada no host (opcional e potencialmente cara):

```sh
python3 -B scripts/test-application.py
python3 -B scripts/test-clean-install.py
python3 -B scripts/measure-isolated.py verification
```

Essas verificações orquestram infraestrutura Docker nova, controles de indisponibilidade na composição principal e instalações limpas. Exigem Docker no host; não pertencem ao app. Campanhas de capacidade continuam opcionais; consulte os modos documentados no script de medição. `tests` não afirma executar essas aceitações. Não é necessário socket Docker nem modo privilegiado.

A regressão de lifecycle reproduzida herdava `DEMO_ENABLED=true` do Compose. O diretório temporário continha apenas a fixture dotenv esperada, mas o filho tentava inicializar o banco da demo e saía com `demo forward migration required`; stderr era descartado. Agora o teste limpa a configuração herdada, mantém somente PATH e endereço próprio do exporter, captura stderr e recolhe o filho mesmo se uma asserção falhar. As verificações de dotenv, readiness/métricas e SIGTERM/SIGINT continuam.

---

**Fluxo atual:** execute `python3 scripts/start-local.py` na raiz. Os comandos de rollout, stack dedicada e observações antigas abaixo são históricos; não substituem a instalação atual. Consulte [aplicação](../operations/application-scenarios.md) e [token/conectividade](../operations/demo-token-connectivity.md).

Comandos Cargo nativos rodam em `backend/`; comandos no app Docker usam `/app/backend`. Consulte [frontend/backend](../guides/frontend-backend.md).

[English](../../en/testing/strategy.md) | [README](../../../README.pt-BR.md)


# Testes

## Marco 4.4 — verificação e fechamento atuais — 16/10/2026

Marco 4 fechado com limitações não bloqueantes explícitas. Evidência real de estoque/deduplicação, ACK/quarentena e recuperação é separada do rollout compartilhado pendente. Frontend de apresentação verificado; integração ao vivo com backend não implementada. Seções históricas abaixo mantêm o escopo das respectivas datas. [Matriz de verificação](../operations/milestone-4-4-review.md) | [Inventário de ambiente](../docker/environment-inventory.md).


Worker atual, configuração, cancelamento e rollout autorizado: [Marco 2.5](../architecture/milestone-2-5.md). Seções anteriores mantêm escopo histórico.


## Marco 2.4 — 2026-10-12

O caso de uso de entrega executa aquisição reconhecida → publicação do envelope original → completion somente após aceitação confirmada. Falhas definitivas liberam uma vez; incerteza mantém lease. Resultados preservam falha principal e cleanup separadamente. HTTP continua independente; não há worker composto. Consulte [Marco 2.4](../architecture/milestone-2-4.md) para resultados, cancelamento e limites de recuperação. Próximo: 2.5 composição/operação do worker; 2.6 experimentos ponta a ponta.

Seções anteriores preservam o escopo do marco/revisão registrado.

## Extensão atual — Marco 2.2

[Estado de entrega e posse](../architecture/milestone-2-2.md) e [ADR 007](../adr/007-durable-delivery-ownership.md) definem recuperação por lease durável e contratos separados de adquirir/concluir/liberar. Migração nova `20261009000000_add_delivery_ownership` adiciona token/acquired_at/expires_at nullable com lease coerente apenas em pending. Seções de marcos anteriores abaixo descrevem escopo original; afirmações antigas de posse/contador indefinidos são substituídas pelo ADR 007. Adapters produtivos de mutação ficam para 2.3; SELECT do reader e publisher preservados.


```bash
cargo test --locked
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
```

```bash
docker compose exec app cargo test --locked
docker compose exec app cargo fmt --check
docker compose exec app cargo clippy --locked --all-targets --all-features -- -D warnings
```

`cargo test` também funciona sem `--locked`; use a opção para validação reproduzível. Execute `cargo fmt` para aplicar formatação. CI usa as mesmas verificações em Rust 1.95.0. Testes padrão dispensam banco ou broker; testes PostgreSQL são opt-in.

Os testes unitários atuais verificam padrões e rejeição de endereços inválidos/ambiente vazio sem mudar o ambiente do processo. Testes de integração abrem um listener real em porta efêmera, verificam HTTP 200 e corpo, solicitam encerramento gracioso e confirmam o fechamento do listener. Um teste de subprocesso Unix carrega `.env` isolado, confere o ambiente no log de partida e envia separadamente SIGTERM/SIGINT, esperando saída bem-sucedida e log final. Ele exige o comando de sistema `kill` (incluído na imagem Docker). Arquivos temporários ficam fora das fontes. Windows ignora apenas esse teste Unix. Esperas por servidor/processo têm prazos; não há serviços externos nem portas fixas nos testes.

Esses testes comprovam bootstrap, não confiabilidade de eventos. A integração do repositório está implementada no Marco 1.7. Integração com broker existe em 2.1; próximos marcos adicionarão janelas de crash ponta a ponta entre publicação/conclusão, tentativas, idempotência, mensagens problemáticas, limites de concorrência e contrapressão. Testes de infraestrutura devem isolar estado e injetar falhas; benchmarks precisam publicar carga, hardware, metodologia e limitações medidas. Consulte [resultados de validação](validation-results.md) para comandos executados; configurar CI não comprova uma execução concluída no GitHub.

## Testes do envelope canônico

`tests/event_envelope.rs` verifica geração de identidade UUID v7 e instante UTC, validação na construção/restauração, formato JSON exato, round-trips de metadados e normalização de offsets. Esses testes independem do PostgreSQL.

## Validação de infraestrutura PostgreSQL (Marco 1.2)

São checks locais explícitos de infraestrutura, não testes de integração da persistência da aplicação. Comece com os padrões locais documentados; use seu usuário/banco configurado se diferente:

```bash
docker compose up -d --build --wait --wait-timeout 120
docker compose ps
docker compose exec -T postgres sh -c 'pg_isready -h 127.0.0.1 -p 5432 -U "$POSTGRES_USER" -d "$POSTGRES_DB"'
docker compose exec -T postgres sh -c 'PGPASSWORD="$POSTGRES_PASSWORD" psql -h postgres -p 5432 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT 1;"'
docker compose port postgres 5432
# psql no host, se instalado; informe a senha local quando solicitado:
psql -h 127.0.0.1 -p 5433 -U change_me -d reliable_event_relay -W -c "SELECT 1;"
docker compose exec -T app cargo fmt --check
docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings
docker compose exec -T app cargo test --locked
docker compose exec -T app cargo build --locked
```

Confirme PostgreSQL healthy, publicação `127.0.0.1:5433`, autenticação e arquivos em `.dockerized-postgres/18/docker/`. Uma sondagem TCP no host comprova apenas alcance, não autenticação SQL. `/health` e os testes de ciclo de vida continuam verificando a aplicação sem conexão ao banco.

### Sobrevivência à recriação do container

Apenas no seu banco local de desenvolvimento: use uma tabela comum descartável com nome único (uma tabela SQL TEMP não sobrevive à sessão), insira um marcador, recrie o container PostgreSQL sem apagar dados do host, verifique o marcador e remova a tabela. Se o nome escolhido já existir, pare e escolha outro; nunca remova um objeto não relacionado. O objeto é apenas de validação de infraestrutura e não deve permanecer:

```bash
docker compose exec -T postgres sh -c 'psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "CREATE TABLE milestone12_validation_20261007 (marker text); INSERT INTO milestone12_validation_20261007 VALUES ('\''survives-recreation'\'');"'
docker compose up -d --force-recreate --no-deps --wait --wait-timeout 120 postgres
docker compose exec -T postgres sh -c 'psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT marker FROM milestone12_validation_20261007;"'
docker compose exec -T postgres sh -c 'psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "DROP TABLE milestone12_validation_20261007;"'
```

Confirme mudança do ID do container e montagem no mesmo diretório físico. Não faça reset/apague dados durante a validação de sobrevivência. Este teste não comprova transações da aplicação, recuperação de crashes ou garantias de entrega. Consulte [resultados reais](validation-results.md) e [instruções de reset destrutivo](../docker/configuration.md).

## Validação de migrações — Marco 1.3

São verificações de ferramenta/infraestrutura, não testes de integração outbox. Testes Rust continuam sem banco. Em cluster local descartável, confirme health PostgreSQL, autenticação TCP e status; aplique, inspecione, reverta e reaplique. Reverta somente após revisar down e confirmar que o banco local é apropriado.

```bash
docker compose up -d --build --wait --wait-timeout 120
docker compose ps
docker compose exec -T app sqlx --version
docker compose exec -T app sqlx migrate info
docker compose exec -T app sqlx migrate run
docker compose exec -T app sqlx migrate info
docker compose exec -T postgres sh -c 'psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -c "SELECT nspname FROM pg_namespace;"'
docker compose exec -T app sqlx migrate revert
docker compose exec -T app sqlx migrate info
docker compose exec -T app sqlx migrate run
docker compose exec -T app sqlx migrate run
docker compose exec -T app sqlx migrate info
docker compose exec -T postgres sh -c 'PGPASSWORD="$POSTGRES_PASSWORD" psql -h postgres -p 5432 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT 1;"'
docker compose port postgres 5432
docker compose exec -T app cargo fmt --check
docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings
docker compose exec -T app cargo test --locked
docker compose exec -T app cargo build --locked
```

No histórico atual, espere installed/pending/installed na última migração outbox e tabela presente/ausente/presente, preservando schema relay e histórico anterior. Validação inicial de rollback do namespace no Marco 1.3 é histórica; revert atual não exclui relay. Execução repetida não aplica nada. Metadados SQLx permanecem após rollback. Para testar geração sem adicionar histórico, execute `docker compose exec -T app sqlx migrate add -r --source /tmp/milestone13-generated create_relay_schema` e inspecione os dois arquivos.

### Ambiente isolado de validação

Quando credenciais locais divergem do estado persistido ou dados precisam ser preservados, use projeto Compose e diretório separados. Credenciais são apenas exemplos, com caracteres reservados de URI para testar encoding. Escolha diretório/projeto/porta livres; nunca reutilize dados desconhecidos. Interface gráfica não é exigida.

```bash
mkdir -p /tmp/relay-milestone13-validation
cat > /tmp/relay-milestone13-validation/test.env <<'EOF'
POSTGRES_USER=validation_user
POSTGRES_PASSWORD='validation:p@ss/%?#'
POSTGRES_DB=milestone13_validation
POSTGRES_HOST=postgres
POSTGRES_PORT=5432
EOF
cat > /tmp/relay-milestone13-validation/compose.yaml <<'EOF'
services:
  app:
    image: rust-event-relay-app
    pull_policy: never
    ports: !reset []
  postgres:
    volumes:
      - /tmp/relay-milestone13-validation/data:/var/lib/postgresql
    ports: !override
      - "127.0.0.1:15433:5432"
EOF
dc() {
  docker compose -p relay-milestone13-validation --env-file /tmp/relay-milestone13-validation/test.env -f compose.yaml -f /tmp/relay-milestone13-validation/compose.yaml "$@"
}
dc up -d --wait --wait-timeout 120
# Substitua docker compose por dc nas verificações acima.
# Docker Desktop: autenticação pela publicação no host:
dc exec -T postgres sh -c 'PGPASSWORD="$POSTGRES_PASSWORD" psql -h host.docker.internal -p 15433 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT 1;"'
# Remova apenas containers/rede do ambiente de teste; preserve seu diretório:
dc down
```

Exige Compose com `!override` (2.24.4+) e `!reset`. A imagem customizada já deve estar construída. host.docker.internal é específico do Docker Desktop; no Linux use psql autenticado no host.

Para reproduzir variáveis de inicialização, mude apenas ambiente postgres do teste em override adicional, recrie esse container com `up --force-recreate --no-deps` condicionado ao health e confirme que novas credenciais falham enquanto as originais do app funcionam. Restaure o ambiente original e confirme sobrevivência do schema aplicado. Nunca reinicialize cluster do desenvolvedor para este experimento. Para indisponibilidade, pare apenas postgres do teste, espere falha de `dc exec -T app sqlx migrate info --connect-timeout 2`, reinicie com `dc up -d --wait --wait-timeout 120 postgres` e confirme installed. Veja [resultados reais](validation-results.md).

## Diagnóstico de autenticação somente leitura

```bash
./scripts/check-postgres.sh
docker compose exec -T app sqlx migrate info
```

[check-postgres.sh](../../../scripts/check-postgres.sh) verifica health, publicação IPv4 loopback, Compose/.env atual versus configurações dos containers, identidade autenticada e histórico existente. Não imprime usuário ou senha configurados. Falha com saída não-zero em divergência de configuração/autenticação, nunca altera usuários, cria metadados, aplica migrações ou reinicializa dados. Usa Python no container app; a sondagem TCP do host usa nc ou Python 3 e informa explicitamente quando pula por ausência de ambos. SQLx é validado separadamente.

Compose fornece POSTGRES_HOST/PORT ao postgres para diagnósticos de cliente, sem alterar escuta do servidor. Host/DBeaver usa 127.0.0.1:5433 e POSTGRES_DB/USER/PASSWORD atualmente inicializados; containers usam postgres:5432. A interface DBeaver não foi testada.

Mudar `.env` não atualiza cluster inicializado. Usuário ausente pode gerar erro TCP genérico de senha. Inspecione detalhes do servidor/usuários para distinguir de usuário existente com senha incorreta. A correção real verificou ausência de dados antes do reset destrutivo explicitamente autorizado e executou migrações existentes. Startup/shutdown Compose normal continua não destrutivo. Reset exclui todo estado; rollback de migração não corrige credenciais.

Configuração Compose bruta, dumps de ambiente e ajuda SQLx podem revelar segredos; use parser que informe apenas campos não sensíveis/comparações e remova credenciais dos logs compartilhados. Veja [correção PostgreSQL](../database/postgresql.md#correção-de-autenticação-no-cluster-local-real) e [resultados reais](validation-results.md#correção-de-autenticação-postgresql-local).

## Validação do schema outbox — Marco 1.4

São fixtures de schema, não testes de integração de repositório Rust. O [fixture SQL versionado](../../../backend/tests/sql/outbox_schema.sql) usa transação e ROLLBACK final: ON_ERROR_STOP=1 também encerra sessão com falha e desfaz transação, preservando linhas preexistentes. Usa UUIDs sintéticos reservados; colisão falha com segurança, portanto use banco local apropriado. Nenhum seed é adicionado.

Verifica 25 casos: envelope/defaults válidos, identidade duplicada, 11 campos NOT NULL, strings obrigatórias vazias, versões 0/-1/acima de u32, tentativas negativas, status inválido, coerência do timestamp de conclusão, máximo u32/UUIDs opcionais/JSON null/offsets e representação de retry pending/falha terminal. Tipo do payload é inspecionado como JSONB. Sem broker, loop de retry ou claim de worker.

```bash
docker compose exec -T app sqlx migrate info
docker compose exec -T app sqlx migrate run
docker compose exec -T app sqlx migrate info
docker compose exec -T postgres sh -c 'psql -X -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1' < tests/sql/outbox_schema.sql
docker compose exec -T postgres sh -c 'psql -X -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT count(*) AS outbox_rows FROM relay.outbox_events;"'
```

Use catálogos para colunas/tipos/nullability/defaults, pg_constraint para PK/CHECKs, pg_indexes para definições e _sqlx_migrations para versão/sucesso. Consultas exatas executadas e resultados estão em [validação](validation-results.md).

**Rollback destrutivo:** somente após confirmar ausência de dados importantes e última migração create_outbox_events, execute migrate revert, inspecione ausência da tabela e preservação relay/histórico, depois migrate run e info. Não deixe schema revertido. Após dados reais, rollback perde todas as linhas; sem garantia de recuperação em produção. Execute quatro verificações Cargo Docker documentadas acima. A tabela final teve zero linhas após fixtures e reaplicação.

A [inspeção de catálogo somente leitura](../../../backend/tests/sql/inspect_outbox_schema.sql) verifica colunas, constraints, índices, histórico e contagem:

```bash
docker compose exec -T postgres sh -c 'psql -X -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1' < tests/sql/inspect_outbox_schema.sql
```

## Testes de contratos de persistência — Marco 1.5

[tests/persistence_contract.rs](../../../backend/tests/persistence_contract.rs) contém cinco testes Rust sem banco:

- Limites positivos, rejeição de zero e ausência de máximo arbitrário; corte UTC explícito preservado.
- Identidade/tempo/payload/UUIDs do envelope intactos, com tentativa/disponibilidade fora dos nove campos serializados.
- Todas categorias de erro, fontes opcionais com downcast e remoção de texto sensível em Display/Debug.
- Chamador OutboxReader genérico com fake de uma resposta, captura da requisição e future Send em Tokio spawn.
- Snapshot vazio bem-sucedido e propagação de erro Unavailable pelo chamador genérico.

Fake mantém um resultado preparado; não é engine em memória nem simula filtros, locks, durabilidade ou claims. Testes não consultam PostgreSQL, instanciam pools, leem .env ou executam SQL. Marco 1.6 adiciona testes do adapter e smoke focado; suíte completa implementada no 1.7. Metadados usam u32 unsigned; conversão de largura assinada é implementada pelo adapter.

```bash
docker compose exec -T app cargo test --locked --test persistence_contract
docker compose exec -T app cargo fmt --check
docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings
docker compose exec -T app cargo test --locked
docker compose exec -T app cargo build --locked
docker compose exec -T -e RUSTDOCFLAGS='-D warnings' app cargo doc --locked --no-deps
```

Distinga fixtures SQL do Marco 1.4 acima, testes Rust de contrato/modelo do 1.5 e integração PostgreSQL do 1.7. Fixtures de schema permanecem inalterados e não foram reexecutados neste marco Rust. Veja [requisitos do contrato](../architecture/persistence-contracts.md) e [validação real](validation-results.md).

## Integração do repositório PostgreSQL — Marco 1.7

`cargo test --locked` continua independente de banco: 15 casos PostgreSQL são ignorados; pool fechado/limite excessivo dispensa servidor. Execução opt-in explícita:

```bash
docker compose exec -T app cargo test --locked --test postgres_repository -- --ignored --test-threads=1
docker compose exec -T app cargo test --locked --test postgres_repository -- --ignored --test-threads=4
# Equivalente nativo/CI, com variáveis POSTGRES exportadas:
cargo test --locked --test postgres_repository -- --ignored --test-threads=4
```

Obrigatórias: POSTGRES_USER, POSTGRES_PASSWORD e POSTGRES_DB (banco existente para conexão administrativa). POSTGRES_HOST padrão postgres; POSTGRES_PORT padrão 5432. No host use 127.0.0.1 e porta publicada (padrão 5433); CI usa 127.0.0.1:5432. Harness não carrega .env; Compose fornece variáveis. Configuração ausente ou PostgreSQL indisponível falha explicitamente. Papel exige CREATEDB e propriedade dos bancos gerados. Job dedicado usa PostgreSQL 18.6 e credenciais descartáveis, com papel de inicialização capaz de criar bancos. Verificações sem banco permanecem separadas.

Suporte compartilhado cria um banco `relay_it_<UUID hexadecimal>` por caso, cita com segurança o nome controlado e aplica as duas migrações up versionadas em ordem cronológica. Fixtures usam IDs/tempos explícitos e SQL fora da API produtiva. Banco administrativo/aplicativo nunca recebe fixtures. Task preserva diagnósticos de assertions; erros retornados são sanitizados, com operação/SQLSTATE para SQL fixture. Prazos: conexão/aquisição 10 segundos, instrução 5 segundos, lock 3 segundos, caso 45 segundos, fechamento de pool/remoção 10 segundos. Barreira sincroniza leitores independentes, sem sleeps fixos.

Após sucesso, panic ou erro retornado, fecha pool antes de remover somente seu banco e verifica ausência no catálogo. Dois casos verificam limpeza após assertion e erro retornado. Término abrupto, indisponibilidade durante limpeza ou prazo excedido pode deixar banco isolado; log identifica nome exato. Confira propriedade e remova manualmente somente essa fixture após fechar conexões; nunca faça reset nem remoção ampla de bancos.

Cobertura: tabela vazia e leitura vazia com linhas futuras/terminais; corte inclusivo; filtros pending/processed/dead_letter; backlog limitado e desempates available_at/created_at/id como comportamento do adapter. Restauração verifica todos os campos e metadados, versões 1/u32::MAX, tentativas 0/i32::MAX, UUIDs opcionais, espaços preservados, tempos históricos/offsets/microssegundos, JSON aninhado/null/arrays/escalares e números exatos, incluindo 1e1000. Comparações usam valores semânticos armazenados, permitindo normalização JSONB; timestamps comparam instante UTC, não grafia do offset ou nanossegundos.

Leituras repetidas e dois leitores observam snapshots iguais sem alterações; linhas completas coincidem antes/depois. Campos obrigatórios em branco, tempos infinitos, disponibilidade -infinity e data finita fora do Chrono falham lote inteiro com fontes preservadas; correção restaura leitura válida. Linhas inválidas futuras/terminais são excluídas. Pool fechado e tabela ausente preservam fontes SQLx tipadas e formatação sanitizada. Limite usize excessivo representável falha antes de acessar pool fechado. Constraints não mudam; contadores negativos, versões inválidas e bytes JSONB malformados permanecem nos testes unitários/de schema.

Não comprova entrega concorrente segura, ordem de negócio nem snapshots sob toda sequência de escritas concorrentes. Marco 1.8 documenta [falhas/recuperação e semântica transacional](../architecture/failure-and-transactions.md); injeção de crash permanece não validada. Escritas de produtores, claims, leases, entrega, retries e transições permanecem fora desta suíte.

## Verificações de fechamento do Marco 1

Caso ignorado `committed_schema_fixture_validates_constraints_and_rolls_back` executa fixture SQL existente de 25 casos somente no banco isolado e verifica zero linhas após rollback. Não adiciona escrita produtiva nem altera dados compartilhados. Fixture JSON aninhada é comparada ao conteúdo esperado explícito, normalizando somente grafia do expoente grande. Suíte tem 15 casos opt-in e um sem banco.

Execute `./scripts/check-postgres.sh` e `docker compose exec -T app sqlx migrate info` antes das verificações Cargo acima. Script `tests/sql/inspect_outbox_schema.sql` é somente leitura; gravações fixture pertencem ao caso Rust isolado. Veja [revisão](../operations/milestone-1-review.md), [semântica](../architecture/failure-and-transactions.md) e [resultados reais](validation-results.md).

## Testes do Marco 2.1

[Comandos de integração RabbitMQ, ciclo Supervisor e validação management](../architecture/milestone-2-1.md#comandos-de-teste-e-evidências) são opt-in. Suíte padrão continua independente de banco/broker. Proxy com broker real verifica aceitação incerta após timeout/cancelamento sem reiniciar broker.

## Integração de posse do Marco 2.3

Reutilizar containers existentes; suíte aplica migração de posse apenas dentro de cada banco do harness. São 12 casos reais, com locks/expiração coordenados e rejeição imediata/diferida. Igualdade exata é evidência do modelo; proxy isolado testa confirmação perdida após COMMIT observado; cancelamento durante commit não verificado. Veja [escopo/evidência](../architecture/milestone-2-3.md) e [resultados](validation-results.md). Publisher/dependências RabbitMQ não mudaram; regressão do broker não foi exigida nem reexecutada.

```bash
docker compose exec -T app cargo test --locked --test postgres_ownership -- --ignored --test-threads=4
```


## Marco 2.6 — Escopo atual

[Experimentos integrados](../architecture/milestone-2-6.md) e [revisão do Marco 2](../operations/milestone-2-review.md) atualizam a aceitação sob premissas explícitas. Rollout compartilhado continua pendente; registros anteriores mantêm seu contexto histórico.

## Marco 3.2 — persistência atual

Schema durável de retry, agenda/preservação terminal protegidas por dono/ciclo/revisão, orçamento de aquisição e manutenção exata de esgotados estão implementados. Integração fica em 3.3, scanner geral em 3.4, replay em 3.5 e fechamento integrado em 3.6. Seções anteriores preservam contexto histórico de cada marco. [Contratos, migration, falhas e rollout](../architecture/milestone-3-2.md).

## Marco 3.3 — integração atual

Integração de entrega/worker implementada; seções anteriores mantêm seu escopo histórico datado. [Contratos atuais, jitter, matriz contextual, manutenção e cancelamento](../architecture/milestone-3-3.md). Scanning geral fica em 3.4, replay auditado em 3.5 e aceitação completa em 3.6; rollout compartilhado permanece pendente.
