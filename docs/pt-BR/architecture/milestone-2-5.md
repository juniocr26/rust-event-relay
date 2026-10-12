[English](../../en/architecture/milestone-2-5.md) | [README](../../../README.pt-BR.md)

# Marco 2.5 — Composição e operação do worker

Implementado em 2026-10-12 a partir de `7be956c`, checkout limpo. O handoff 2.4 solicitado não estava presente; [código e semântica 2.4](milestone-2-4.md), testes e histórico Git forneceram a referência. Nenhum AGENTS.md aplicável foi encontrado nos projetos/ancestrais. Seções antigas preservam seu escopo histórico; este documento descreve o worker atual.

## Composição e admissão

[src/bin/relay.rs](../../../backend/src/bin/relay.rs) carrega `.env`/configuração, inicializa tracing JSON, constrói pool SQLx dedicado e repositório, verifica colunas de ownership sem escrever e executa o worker com lifecycle RabbitMQ. Reutiliza campos separados POSTGRES e RABBITMQ; DATABASE_URL não cria configuração concorrente. Configuração inválida, telemetry, conexão PostgreSQL inicial ou schema/permissões indisponíveis encerram antes do polling com erro sanitizado e código não zero. Unavailable no setup RabbitMQ não distingue autenticação/topologia de indisponibilidade transitória; tentativas de conexão são espaçadas explicitamente até shutdown. Falhas de persistência em operação são espaçadas, sem expor cadeias de fontes.

[application/worker.rs](../../../backend/src/application/worker.rs) usa dispatch estático sobre ports de persistência e `PublisherLifecycle`, seam focado de conexão, usabilidade e consumo no close. Construção SQLx/Lapin fica na composição. Não há tarefas criadas pelo worker, fila de entregas, batching ou paralelismo. Tarefas internas SQLx/Lapin não representam admissões adicionais de entrega. Cada tentativa cria `AcquireRequest` e UUID novos e chama `deliver_once` uma vez. Snapshot não concede posse. Sucesso permite nova tentativa imediatamente; NoEvent não comprova backlog vazio.

## Configuração validada

Durações são inteiros positivos em **milissegundos**. Valores do processo prevalecem sobre `.env`. Zero, negativos, overflow, formato inválido ou valores fora do intervalo impedem startup; mensagens identificam chave/intervalo sem repetir o valor fornecido. Conversão do lease para microssegundos i64 é verificada. O mínimo do pool permanece zero, padrão SQLx.

| Configuração | Padrão | Intervalo inclusivo | Finalidade |
| --- | ---: | --- | --- |
| RELAY_LEASE_MS | 60000 | 1–3600000 | Validade durável da posse |
| RELAY_IDLE_MS | 1000 | 1–60000 | Espera após NoEvent |
| RELAY_FAILURE_MS | 1000 | 1–60000 | Espera após entrega sem sucesso |
| RELAY_RECONNECT_MS | 5000 | 1–60000 | Espera após substituição/falha de conexão |
| RELAY_POOL_MAX | 2 | 1–16 conexões | Limite do pool dedicado |
| RELAY_POOL_ACQUIRE_MS | 5000 | 1–60000 | Admissão no pool/conexão inicial |
| RELAY_STATEMENT_MS | 5000 | 1–60000 | statement_timeout de sessão PostgreSQL |
| RELAY_LOCK_MS | 3000 | 1–60000 | lock_timeout de sessão PostgreSQL |
| RELAY_DRAIN_MS | 15000 | 1–20000 | Drain da operação atual após sinal |
| RELAY_SHUTDOWN_MS | 25000 | 1–25000 | Drain e cleanup global |

Drain deve ser menor que shutdown. PgConnectOptions configura statement_timeout/lock_timeout apenas neste pool; SQL, classificação e predicados do repositório permanecem iguais. São limites por statement/espera de lock, sem limitar mecanicamente transação inteira, resposta de rede ou entrega. Timeout de aquisição no pool não limita consulta. RabbitMQ mantém padrões connect/publish de 10000 ms, intervalo 1–300000; gate e envio/confirmação têm budgets separados, assim como conexão/setup/close após setup falho. Não há novo deadline global de entrega nem renovação de lease. Defaults deixam margem, mas múltiplas consultas, latência, relógios e stalls podem consumi-la; não garantem evitar expiração. Posse no banco não impede publicação externa.

## Política explícita de outcomes

Logs preservam resultados primários **e** secundários sanitizados, identificadores seguros de evento/posse e metadados de aquisição; não incluem envelopes, payloads, credenciais, URLs ou cadeias de fontes.

| Outcome | Decisão | Interpretação durável |
| --- | --- | --- |
| NoEvent | Espera idle | Nenhuma aquisição nesta chamada |
| Aquisição Unavailable/InvalidStoredData/OperationFailed | Espera failure e continua | Nenhum claim reconhecido |
| Aquisição CommitUncertain | Espera failure; próximo token novo | Lease/contador podem ter sido persistidos; nunca fabricar claim |
| Published + conclusão Applied | Continua imediatamente | Broker aceitou e conclusão durável foi reconhecida |
| Published + OwnershipLost | Espera failure | Aceitação confirmada; conclusão não reconhecida |
| Published + erro comum de conclusão | Espera failure | Aceitação confirmada; não inferir rollback |
| Published + CommitUncertain | Espera failure | Conclusão pode já estar persistida |
| Publicação Unavailable | Fecha/substitui e espera reconnect | Caso de uso tentou release uma vez |
| Publicação Serialization | Espera failure, mantém conexão usável | Release tentado uma vez |
| Publicação Unroutable | Espera failure, mantém conexão usável | Falha definitiva de rota; release uma vez |
| Publicação Rejected | Espera failure, mantém conexão usável | Rejeição definitiva; release uma vez |
| Publicação Uncertain | Fecha/substitui e espera reconnect | Lease retido, aceitação desconhecida |

| Release independente | Relato e política |
| --- | --- |
| Applied | Registra liberação reconhecida; mantém política do erro primário |
| OwnershipLost | Registra perda de autoridade; mesma política |
| Unavailable/InvalidStoredData/OperationFailed | Registra falha secundária classificada; mesma política |
| CommitUncertain | Registra incerteza; transição pode ter sido persistida; mesma política |
| LeaseRetained | Registra retenção após incerteza; substitui publisher |

Conclusão falha/incerta não causa release compensatório nem republicação imediata do claim. Invocação posterior depende dos predicados duráveis, usa token novo e preserva identidade original. Release mantém available_at, last_error e attempt_count. Pacing evita hot loop após liberação imediata; não é agenda durável de retry. Não há backoff exponencial/jitter, contador novo, fairness, quarentena ou DLQ. Eventos elegíveis permanentemente inválidos/falhos podem obstruir progresso; eventos com contador esgotado permanecem pending.

## Substituição do publisher

Antes de adquirir, o loop consulta retired/status real da conexão/canal. Publisher sabidamente inutilizável é consumido/fechado e substituído antes de nova tentativa. Desconexão pode ocorrer após a consulta; Unavailable cobre essa corrida. Unavailable e Uncertain causam substituição explícita; Serialization/Unroutable/Rejected não a causam enquanto a conexão for usável. Falha de conexão espera interruptivelmente, sem adquirir eventos. Conexão nova não reconcilia aceitação anterior nem repete publicação anterior. Cancelamento após envio pode aposentar o canal sem retornar outcome; shutdown fecha/descarta sem release cego. Não há recuperação automática do protocolo ou retry oculto.

## Shutdown e Supervisor

SIGINT/SIGTERM seguem a convenção existente da aplicação. Sinal é verificado antes de cada admissão; idle/reconnect são imediatamente interrompíveis. Operação atual de connect/close/entrega pode terminar em até 15 segundos de drain. Depois, seu future é descartado e o log informa que efeitos podem ter ocorrido sem outcome. Não se libera claim abandonado cegamente. Expiração permite elegibilidade posterior, sem provar rollback ou não aceitação.

Um deadline absoluto inicia quando o sinal é observado: 25 segundos por padrão. Close do publisher e Pool::close compartilham o restante; conexões emprestadas não tornam a espera ilimitada. Falha/timeout de cleanup é registrada; saída intencional por sinal é zero mesmo quando foi necessário limitar cleanup. Setup inicial somente leitura pode ser cancelado diretamente e fecha pool construído dentro do budget. Instalação de sinais preserva fail-fast do HTTP. Deadlines Tokio dependem de polling cooperativo; SIGKILL/quebra de energia não permitem drain.

A [configuração física](../../../docker/supervisor/normal.conf) preserva `http` e adiciona [launcher relay-worker executável](../../../docker/supervisor/relay-worker), usando `/app/target/debug/relay`, sem cargo run. Preserva developer, logs stdout/stderr, socket privado e alias. `relay` tem **autostart=false**, aguardando autorização da migração compartilhada. Cada filho tem TERM/30 segundos; Compose concede 65 segundos para shutdown sequencial dos grupos e margem. `autorestart=unexpected`, `exitcodes=0` reinicia falha inesperada; stop intencional ou sinal com saída zero não reinicia filho já RUNNING. Falha durante STARTING de um segundo gera até três novas tentativas e FATAL, mesmo com código zero. Outage RabbitMQ em runtime reconecta no loop.

```bash
docker compose exec app cargo build --locked --bins
# Instalar mudanças físicas: somente app é reconstruído/recriado.
docker compose build app
docker compose up -d --no-deps app
docker compose exec app supervisorctl status
docker compose exec app supervisorctl stop relay
docker compose exec app supervisorctl start relay
docker compose exec app supervisorctl restart relay
docker compose exec app supervisorctl stop http
docker compose exec app supervisorctl start http
docker compose exec app supervisorctl restart http
```

`supervisor` aceita os mesmos argumentos; ambos têm console interativo. `stop/start/restart all` agora controla **ambos** os programas, incluindo relay com autostart desabilitado; não controla banco/broker. Não execute `start all` contra schema compartilhado pendente. Código exige stop/build/start; configuração instalada exige rebuild/recriação do app.

## Pré-requisito para rollout autorizado

`sqlx migrate info` somente leitura em 2026-10-12 confirmou 20261009000000 pendente; não foi aplicada ao banco compartilhado. Relay permanece STOPPED. `/health` comprova apenas liveness HTTP. Revise dados e autorize a migração antes de executar:

```bash
# Somente após autorização para este banco compartilhado:
docker compose exec -T app sqlx migrate info
docker compose exec -T app sqlx migrate run
docker compose exec -T app sqlx migrate info
docker compose exec app supervisorctl start relay
docker compose exec app supervisorctl status relay
```

Não precisa reset, mudança de credenciais ou recriação dos serviços de infraestrutura. Ativar autostart posteriormente exige decisão operacional versionada e instalação apenas do app.

## Validação e próximo escopo

[Testes do worker](../../../backend/tests/worker.rs) usam tempo pausado/notificações para tokens novos, pacing, substituição, admissão encerrada, esperas interruptíveis, drain e cleanup limitado. [Smoke de composição](../../../backend/tests/worker_smoke.rs) usa bancos próprios migrados e recursos RabbitMQ UUID; verifica identidade/payload, uma aquisição, estado processed, SIGTERM e controles individuais/coletivos do Supervisor físico isolado. Consumidor da fixture inspeciona mensagens; não existe consumidor produtivo. Fakes provam política, sem provar rollback de crash ou ausência de aceitação.

[Resultados datados](../testing/validation-results.md#milestone-25--2026-10-12) e handoff (relatório 2.5 ausente neste checkout) registram execução. **2.6** adiciona experimentos integrados de crash/falha/recuperação, dois workers e fechamento do marco. Happy path não é evidência de recuperação. Sem alegações de throughput, fairness, efeitos do consumidor ou at-least-once ponta a ponta. Interface gráfica pode ser considerada após conclusão do núcleo, se atender necessidade operacional concreta; UI/readiness framework/métricas/dashboard não integram 2.5.
