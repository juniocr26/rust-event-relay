[Other language](../../en/docker/environment-inventory.md) | [Catalog](../index.md)

# Inventário de ambiente — Marco 4.4

Auditado em 16/10/2026 na base `cabfbcaa3f31f2be08657c400ef98275acc78df4`. O `.env` raiz ignorado foi inspecionado sem exibir valores. Não existem `.env` locais em backend/frontend. As entradas da raiz correspondem ao exemplo: não foram encontrados nomes extras obsoletos, credenciais obrigatórias ausentes, valores obrigatórios vazios ou placeholders `change_me`. `REPLAY_OPERATORS` e `RELAY_MAINTENANCE_CANDIDATES` vazios desabilitam recursos opcionais de forma intencional. Nenhum segredo foi criado ou alterado.

Cada entrada é opcional com o default do código, salvo indicação de obrigatoriedade. Identidade/senha/banco PostgreSQL são obrigatórios para relay, consumer e HTTP com replay habilitado; HTTP de liveness não precisa deles. Credenciais do broker são obrigatórias para relay/consumer e inicialização do serviço, não para a apresentação. A configuração do consumer só é exigida quando ele é iniciado explicitamente. Todos os valores são configuração de infraestrutura no servidor; **nenhum configura uma API no navegador**. `VITE_*` é público, mas o frontend não consome variável VITE, não implementa proxy/API e não deve receber tokens de operador.

## Precedência e injeção observada

O Compose resolve overrides do shell, interpolação do `.env` e `${VAR:-default}` / `${VAR:?error}`. O `.env` raiz não injeta automaticamente variáveis nos contêineres: o mapeamento `environment` seleciona os nomes. Não havia overrides do shell nas chaves auditadas. Os valores injetados em app/PostgreSQL/broker correspondem à configuração resolvida atual, **exceto as chaves do consumer e REPLAY_OPERATORS, ausentes no contêiner antigo do app**. Configuração do relay e credenciais existentes correspondem. O frontend recebe apenas `npm_config_cache`, usado pelas ferramentas Node.

`APP_ENV`, `HTTP_ADDR` e `RUST_LOG` não estão no mapeamento `environment` do app. Cada binário Rust chama `Config::load`: dotenvy procura no diretório atual e ancestrais (`/app/backend` → `/app/.env`), preenche apenas variáveis ainda ausentes e depois aplica defaults. Isso é fallback pelo arquivo montado, não injeção do Compose. O mesmo mecanismo pode preencher consumer/replay em um novo binário executado no contêiner antigo, mas não atualiza processos existentes nem instala o programa ausente no Supervisor. Exportar somente essas três chaves no shell host não as encaminha ao app do Compose. Variáveis do processo nativo prevalecem sobre dotenv; erro de parsing do arquivo impede startup com diagnóstico redigido.

`RABBITMQ_HOST/PORT` no `.env` são defaults para execução nativa/testes; o Compose fixa `rabbitmq:5672` no app. Clientes internos usam `postgres:5432`; ferramentas host usam `127.0.0.1:5433`. POSTGRES_HOST/PORT no contêiner do banco configuram o cliente de diagnóstico, não o listener do servidor. Variáveis de inicialização não alteram roles/senhas/vhosts persistidos. Para instalar ambiente injetado novo, recrie o app; reiniciar só um processo não basta.

Inspeção de `/proc/11/environ` e `/proc/12/environ` confirmou que as 26 chaves injetadas de banco/broker/relay chegaram aos dois filhos atuais, sem diferenças. `/proc` mostra o ambiente inicial de exec, não alterações posteriores feitas por dotenv/setenv.

## Nomes, tipos, defaults e processos

A tabela usa defaults versionados, nunca valores locais de credenciais. Entradas locais não vazias estão presentes e são compatíveis com o setup auditado; autenticação real nos testes confirma as credenciais atuais. Todas as entradas do consumer existem no `.env` local, mas não no ambiente injetado do contêiner antigo. As demais chaves explicitamente injetadas correspondem; as três chaves de aplicação usam fallback dotenv. UID/GID/portas host/build são entradas do Compose, não variáveis da aplicação.
| Nome | Processo / finalidade | Requisito / tipo / faixa | Default do código |
| --- | --- | --- | --- |
| `APP_ENV` | HTTP, relay, consumer | string não vazia | `development` |
| `RUST_LOG` | HTTP, relay, consumer | tracing EnvFilter | `info` |
| `HTTP_ADDR` | HTTP; validado por todos os binários | socket IP:porta | `0.0.0.0:8080` |
| `HOST_HTTP_PORT` | publicação de porta Compose | porta TCP 1..65535 | `8080` |
| `LOCAL_UID` | build Docker / usuário frontend | UID positivo do SO | `1000` |
| `LOCAL_GID` | build Docker / usuário frontend | GID positivo do SO | `1000` |
| `POSTGRES_DB` | mesmos clientes / inicialização PostgreSQL | banco existente não vazio; obrigatório | `obrigatório; Compose reliable_event_relay` |
| `POSTGRES_USER` | relay, consumer, HTTP com replay habilitado, SQLx, inicialização PostgreSQL/testes | role não vazia; obrigatória; privada | `obrigatório; placeholder do exemplo Compose` |
| `POSTGRES_PASSWORD` | mesmos clientes / inicialização PostgreSQL | senha não vazia; obrigatória; SEGREDO | `obrigatório; placeholder do exemplo Compose` |
| `POSTGRES_HOST` | clientes do banco / health probe | hostname; wrapper SQLx exige postgres | `postgres` |
| `POSTGRES_PORT` | clientes do banco / health probe | u16 > 0 (relay/consumer); SQLx exige 5432 | `5432` |
| `POSTGRES_HOST_PORT` | publicação de porta Compose | porta TCP 1..65535 | `5433` |
| `RABBITMQ_DEFAULT_USER` | relay, consumer, inicialização broker, management/testes | usuário não vazio; obrigatório; privado | `obrigatório` |
| `RABBITMQ_DEFAULT_PASS` | mesmos clientes / inicialização broker | senha não vazia; obrigatória; SEGREDO | `obrigatório` |
| `RABBITMQ_DEFAULT_VHOST` | clientes broker / inicialização broker | vhost codificado como um segmento de URI | `relay` |
| `RABBITMQ_MANAGEMENT_PORT` | publicação de porta Compose | porta TCP 1..65535; validador espera 15672 | `15672` |
| `RABBITMQ_HOST` | relay / consumer / testes | hostname URI válido; Compose fixa rabbitmq | `rabbitmq` |
| `RABBITMQ_PORT` | relay / consumer / testes | u16 1..65535; Compose fixa 5672 | `5672` |
| `RABBITMQ_CONNECT_TIMEOUT_MS` | relay / consumer | milissegundos inteiros 1..300000 | `10000` |
| `RABBITMQ_PUBLISH_TIMEOUT_MS` | relay / consumer | milissegundos inteiros 1..300000 | `10000` |
| `RABBITMQ_EXCHANGE` | publisher relay / testes | relay.* string curta <=255 bytes | `relay.events` |
| `RABBITMQ_QUEUE` | publisher relay / testes | relay.* string curta <=255 bytes | `relay.local` |
| `RABBITMQ_ROUTING_KEY` | publisher relay / testes | não vazia string curta <=255 bytes | `event` |
| `RELAY_LEASE_MS` | processo relay | milissegundos inteiros 1..3600000 | `60000` |
| `RELAY_IDLE_MS` | processo relay | milissegundos inteiros 1..60000 | `1000` |
| `RELAY_FAILURE_MS` | processo relay | milissegundos inteiros 1..60000 | `1000` |
| `RELAY_RECONNECT_MS` | processo relay | milissegundos inteiros 1..60000 | `5000` |
| `RELAY_POOL_MAX` | processo relay | contagem inteira 1..16 | `2` |
| `RELAY_POOL_ACQUIRE_MS` | processo relay | milissegundos inteiros 1..60000 | `5000` |
| `RELAY_STATEMENT_MS` | processo relay | milissegundos inteiros 1..60000 | `5000` |
| `RELAY_LOCK_MS` | processo relay | milissegundos inteiros 1..60000 | `3000` |
| `RELAY_DRAIN_MS` | processo relay | milissegundos inteiros 1..20000 | `15000` |
| `RELAY_SHUTDOWN_MS` | processo relay | milissegundos inteiros 1..25000 | `25000` |
| `HOST_FRONTEND_PORT` | publicação de porta Compose | porta TCP 1..65535 | `5173` |
| `RELAY_MAINTENANCE_CANDIDATES` | processo relay | opcional; <=4096 bytes; <=32 entradas únicas UUID:cycle:revision | `vazio; desabilitado` |
| `REPLAY_OPERATORS` | somente HTTP | array JSON opcional SEGREDO, veja abaixo | `ausente/vazio desabilita rotas` |
| `CONSUMER_NAME` | consumer | 1..128 letras/dígitos ASCII/._-; estável entre réplicas | `inventory.stock.v1` |
| `CONSUMER_PREFETCH` | consumer | contagem inteira 1..32; uma operação ativa | `1` |
| `CONSUMER_SOURCE_EXCHANGE` | consumer | <=255 bytes; exchange/fila relay.*; chave não vazia | `relay.events` |
| `CONSUMER_SOURCE_QUEUE` | consumer | <=255 bytes; exchange/fila relay.*; chave não vazia | `relay.local` |
| `CONSUMER_SOURCE_ROUTING_KEY` | consumer | <=255 bytes; exchange/fila relay.*; chave não vazia | `event` |
| `CONSUMER_QUARANTINE_EXCHANGE` | consumer | <=255 bytes; exchange/fila relay.*; chave não vazia | `relay.quarantine` |
| `CONSUMER_QUARANTINE_QUEUE` | consumer | <=255 bytes; exchange/fila relay.*; chave não vazia | `relay.quarantine.inventory` |
| `CONSUMER_QUARANTINE_ROUTING_KEY` | consumer | <=255 bytes; exchange/fila relay.*; chave não vazia | `inventory` |
| `CONSUMER_OPERATION_MS` | processo consumer | milissegundos inteiros 1..60000 | `10000` |
| `CONSUMER_RECONNECT_MS` | processo consumer | milissegundos inteiros 1..60000 | `5000` |
| `CONSUMER_DRAIN_MS` | processo consumer | milissegundos inteiros 1..20000 | `15000` |
| `CONSUMER_SHUTDOWN_MS` | processo consumer | milissegundos inteiros 1..25000 | `25000` |
| `CONSUMER_POOL_MAX` | processo consumer | contagem inteira 1..16 | `2` |
| `CONSUMER_POOL_ACQUIRE_MS` | processo consumer | milissegundos inteiros 1..60000 | `5000` |
| `CONSUMER_STATEMENT_MS` | processo consumer | milissegundos inteiros 1..60000 | `5000` |
| `CONSUMER_LOCK_MS` | processo consumer | milissegundos inteiros 1..60000 | `3000` |

## Variáveis adicionais de ferramentas e testes

- `CARGO_HOME=/app/.cargo-cache`, `CARGO_TARGET_DIR=/app/target`: caches e artefatos de Compose/Docker/entrypoint. Overrides opcionais de ferramentas; os launchers continuam usando `/app/target/debug/{reliable-event-relay,relay,consumer}`. Outro target não atualiza esses caminhos.
- `DATABASE_URL`: o wrapper SQLx sobrescreve a variável com os campos POSTGRES obrigatórios e escaping de URI; exige postgres:5432. Não exiba URL ou help do SQLx. Os binários de aplicação não consomem DATABASE_URL.
- `npm_config_cache=/tmp/npm-cache`: cache de ferramentas no frontend. `PLAYWRIGHT_BROWSERS_PATH`: override opcional do Playwright; instalação e testes precisam acessar o mesmo cache. Nesta execução Chromium foi instalado/executado como root no contêiner frontend, sem alterar privilégios do backend.
- `PGPASSWORD`, `PGCONNECT_TIMEOUT`, `PGOPTIONS`: ambiente local dos diagnósticos libpq, com prazos de conexão/comandos/locks. Senhas não vão na linha de comando.
- `RECOVERY_CHILD`, `RECOVERY_READY`, `CONSUMER_BOUNDARY_FILE`: modos/barreiras de filhos dos testes. Sem configuração, as entradas internas são no-op e não contam como testes substantivos. Não configure serviços compartilhados com elas.
- `HANDOFF_FONT_DIR`: diretório opcional de fontes do gerador histórico de PDF. Nenhum PDF/handoff foi gerado no 4.4.
- CI injeta credenciais descartáveis POSTGRES/RABBITMQ, hosts e banco administrativo sem depender do `.env` local. O job de documentação usa Python; o integrado instala Supervisor. O workflow foi revisado/atualizado, não executado remotamente.

## Requisitos condicionais e ações

`REPLAY_OPERATORS`: ausente/vazio desabilita replay. Para habilitar deliberadamente, configure um array JSON confiável de 1..16 objetos: token (64 caracteres hexadecimais minúsculos, gerados por CSPRNG do SO), actor (1..64 letras/dígitos ASCII/._-), allow_replay (booleano), maximum_budget (1..100). Actor/token únicos; total <=8192 bytes. Instale antes a migration 20261013000000 e permissões. Guarde segredos apenas na configuração protegida do servidor; não os cole no chat. Valor habilitado malformado impede startup em vez de desabilitar autorização silenciosamente.

Valores numéricos de runtime rejeitam vazio/não inteiro/zero/fora da faixa; exceção: o parser separado de porta PostgreSQL no HTTP aceita sintaxe u16 e depende da falha de conexão para porta zero. Relay/consumer rejeitam zero explicitamente. Drain deve ser estritamente menor que shutdown. Mudar CONSUMER_NAME muda o escopo da deduplicação; exchanges/filas de origem e quarentena devem diferir. Consulte [parsers](../../../backend/src/config.rs), [broker](../../../backend/src/infrastructure/rabbitmq.rs), [consumer](../../../backend/src/infrastructure/rabbitmq/consumer.rs), [HTTP](../../../backend/src/main.rs) e [wrapper SQLx](../../../docker/sqlx.py).

Nenhum segredo ausente bloqueia os testes isolados. O rollout compartilhado do consumer depende da migration 20261014000000 e instalação do launcher/configuração Supervisor; replay está desabilitado intencionalmente e a migration 20261013000000 está pendente. Mantenha ambos desabilitados até o [rollout revisado](../operations/milestone-4-4-review.md). Não altere senhas persistidas para corresponder ao exemplo.
