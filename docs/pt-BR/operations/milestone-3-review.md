[English](../../en/operations/milestone-3-review.md) | [Catálogo](../index.md)

# Marco 3.6 — Aceitação integrada e auditoria completa do projeto

Revisão: 13/10/2026. Base Rust: `1b8b96998466eb10f68d59c2289c6abf66cd686e`; base Engineering Library: `665779b73de226c7eb39b621536249252f623c9f`. Ambos estavam sem alterações locais. Esta revisão inclui as alterações de aceitação ainda não commitadas sobre essa base. Resultados históricos permanecem históricos.

**Marco 3 fechado com limitações explícitas não bloqueantes no escopo implementado definido.** A auditoria não confirmou defeito de correção no backend dentro desse escopo. Corrigiu defeitos de infraestrutura de aceitação e documentação. A evidência é finita e cobre falha/recuperação durável; não declara o produto completo nem pronto para produção. Rollout compartilhado de replay e integração frontend/backend são fronteiras distintas.

## Classificação e matriz de aceitação

V = implementado e verificado no ambiente indicado; U = implementado, mas não verificado naquele ambiente; D = defeituoso; P = planejado e ausente; B = bloqueado por ambiente ou rollout; N = não aplicável ao escopo entregue. Testes verdes sozinhos não atribuem V: foram comparados código, contratos, metadados do banco, observações independentes do broker e interface renderizada.

Os caminhos de código abaixo pertencem a `backend/`. Cada linha relaciona implementação, invariantes, testes, observação real e limite. Várias condições ou agendas dentro de uma função de teste não são contadas como testes separados.

| Requisito | Código, contrato e invariante | Evidência automatizada | Integração/runtime | Estado e limite |
| --- | --- | --- | --- | --- |
| M1 HTTP/config/ciclo de vida | [main](../../../backend/src/main.rs), [config](../../../backend/src/config.rs), [HTTP](../../../backend/src/application.rs): health é liveness; ambiente prevalece sobre dotenv | [lifecycle](../../../backend/tests/lifecycle.rs), unidades de config | Sockets/sinais reais; `/health` compartilhado responde; launchers instalados correspondem ao código | V; health não é readiness de banco/broker |
| M1 envelope canônico | [event](../../../backend/src/domain/event.rs): ID estável, UTC, identificadores não vazios, versão positiva e restauração fiel | [event_envelope](../../../backend/tests/event_envelope.rs), [repository](../../../backend/tests/postgres_repository.rs) | IDs/payloads próprios decodificados independentemente de AMQP; precisão JSONB e timestamps não suportados | V; sem schema de negócio ou ordenação global |
| M1 schema/migrações | [migrations](../../../backend/migrations): constraints de estado, conclusão, posse, retry e diagnóstico; PK e índice pending | [schema](../../../backend/tests/delivery_ownership_schema.rs), postgres_retry/replay, [CLI](../../../scripts/check-migrations.py) | Ledger SQLx limpo, cinco upgrades/checksums, no-op, down/re-up de replay; quatro checksums compartilhados coincidem | V isolado; B migração 5 compartilhada; down perde metadados |
| M1 observação | [persistence](../../../backend/src/persistence/mod.rs), [repository](../../../backend/src/infrastructure/postgres/mod.rs): snapshots limitados não conferem autoridade | [persistence_contract](../../../backend/tests/persistence_contract.rs), postgres_repository | Leitores independentes sem mutação e linhas inválidas selecionadas/excluídas no PostgreSQL | V; limite de linhas não limita bytes/custo |
| M2 publicação | [RabbitMQ](../../../backend/src/infrastructure/rabbitmq.rs): mandatory, confirms, JSON persistente, message_id estável | [rabbitmq_publisher](../../../backend/tests/rabbitmq_publisher.rs), unidades | Propriedades reais, retorno mandatory, confirmação retida e fechamento de conexão própria | V; aceitação não comprova consumo de negócio |
| M2 posse durável | [ownership](../../../backend/src/infrastructure/postgres/ownership.rs): transação curta Read Committed, SKIP LOCKED, relógio pós-lock, token novo, commit antes da exposição | [postgres_ownership](../../../backend/tests/postgres_ownership.rs), [delivery_ownership](../../../backend/tests/delivery_ownership.rs) | Workers reais concorrentes, expiração, token antigo recusado e morte de processo | V; token protege SQL, não publicação; sem renovação |
| M2 caso de uso | [delivery](../../../backend/src/application/delivery.rs): concluir somente após confirmação; preservar resultados primários/secundários | [delivery_use_case](../../../backend/tests/delivery_use_case.rs) | Broker observado antes de conclusão durável; conclusão rejeitada/bloqueada mantém pending | V; envio externo e commit SQL são transações distintas |
| M2 worker/ciclo de vida | [composição](../../../backend/src/bin/relay.rs), [worker](../../../backend/src/application/worker.rs): uma operação, pacing/substituição explícitos e drain/cleanup finitos | [worker](../../../backend/tests/worker.rs), [smoke](../../../backend/tests/worker_smoke.rs) | Binário compilado e Supervisor isolado; indisponibilidade/reconexão/sinais | V isolado; U revisão exata carregada compartilhada; shutdown cooperativo com fallback Supervisor |
| M3.1 classificação/orçamento/jitter | [policy](../../../backend/src/application/retry_policy.rs), [retry](../../../backend/src/domain/retry.rs): aquisições commitadas consomem allowance; aritmética verificada; incerteza prevalece | [retry_policy](../../../backend/tests/retry_policy.rs), [retry_state](../../../backend/tests/retry_state.rs), delivery_use_case | Sucesso/falha finais reais e distinção unavailable/uncertain | V; reprodução visual não mede política; capacidade numérica finita |
| M3.2 agendamento durável | [retry repository](../../../backend/src/infrastructure/postgres/retry.rs): dono/ciclo/revisão/intervalo ativo; due time pós-lock; evidência e lease clear atômicos | [postgres_retry](../../../backend/tests/postgres_retry.rs): schedule_is_atomic, scheduling_samples_time_after_lock | Retorno mandatory definitivo → due persistido → aquisição antecipada recusada → publicação original confirmada | V; sem retry oculto ou promessa de timing/fairness |
| M3.2 preservação terminal | retry repository: falha final/manutenção esgotada preservam conteúdo/histórico e limpam autoridade, sem processed | postgres_retry: exact_budget_boundary, maintenance_checks_due_state_revision_live_lease_and_storage_exhaustion | Falha final e limite do contador; SQL terminal independente e fila própria vazia | V; dead_letter SQL não é DLQ do broker |
| M3.3 admissão integrada | delivery/worker: última aquisição bem-sucedida pode concluir; pacing e manutenção limitados | worker, delivery_use_case, [integrated_recovery](../../../backend/tests/integrated_recovery.rs) | Crash após schedule/replay, restart compilado e conclusão no último orçamento; terminal sobrevive | V; sem garantia universal de entrega eventual |
| M3.4 descoberta/progresso | [discovery](../../../backend/src/infrastructure/postgres/discovery.rs): uma observação keyset; cursor avança antes da mutação; hint exato revalidado | [postgres_discovery](../../../backend/tests/postgres_discovery.rs), [maintenance](../../../backend/tests/maintenance.rs) | Binário ultrapassa prefixo inseguro e publica ID válido independente; wrap/inserções testados | V; restart reinicia cursor; sem limite de custo/bytes/fairness |
| M3.4 isolamento/dono ativo | discovery: lock/relógio/evidência atuais; representação não suportada não autoriza invalidade permanente | postgres_discovery: mixed_candidates, active_final_owner, locks_concurrent_scanners | Envelope inválido preservado sem lease inventado; dono final ativo conclui | V; reparo auditado de conteúdo/metadados inseguros continua P |
| M3.5 fronteira de segurança | [operator_http](../../../backend/src/infrastructure/operator_http.rs), [replay](../../../backend/src/application/replay.rs): ator confiável, permissão em todos os caminhos, orçamento por ator e body 4096 bytes | [replay_http](../../../backend/tests/replay_http.rs), [replay_application](../../../backend/tests/replay_application.rs) | Router/auth Axum reais com port contador; router com PostgreSQL/broker reais na integração | V router/aplicação; B API opt-in compartilhada; não é teste ponta a ponta de replay por socket/browser |
| M3.5 replay/auditoria | [replay repository](../../../backend/src/infrastructure/postgres/replay.rs), migração 5: request lock antes da linha; intenção/recibo/evidência prévia e ciclo atômicos | [postgres_replay](../../../backend/tests/postgres_replay.rs): atomic_preservation, concurrent_request_identity, concurrent_changed_intent | Router autorizado → auditoria → aquisição → mensagem independente → conclusão; repetição mantém recibo após avanço | V isolado; owner/superuser pode alterar objetos; sem reversão de efeitos |
| M3.5 concorrência | replay/discovery/acquisition: candidato antigo não terminaliza ciclo novo; um dono ativo | postgres_replay: replay_competes_with_stale_discovery_isolation_and_acquisition, rejected_transaction_and_lock_wait_recheck | Operações concorrentes PostgreSQL; revisão/ciclo/estado pós-lock conferidos | V; agendas finitas não cobrem todos os interleavings |
| M2/M3 rejeição/incerteza/cancelamento | [erros](../../../backend/src/infrastructure/postgres/ownership.rs), delivery/retry/replay: rejeição reverte; ACK perdido é desconhecido; drop não compensa | postgres_ownership/retry/discovery/replay, integrated_recovery | Rejeição deferred, proxies de ACK COMMIT perdido, cancelamento após commit e observações independentes | V fronteiras testadas; lookup negativo não exclui commit ainda em voo |
| M3 crash/restart | [integrated_recovery](../../../backend/tests/integrated_recovery.rs): committed_schedule_terminal_and_replay_survive_process_kill_with_final_budget_success | Três subcasos em uma função substantiva; regressão de crash antes/depois da confirmação | Filho exato morto após estado durável; worker novo; identidade/conteúdo/histórico preservados | V; crash de processo não prova recuperação de armazenamento/energia |
| Docker/Supervisor | [Compose](../../../compose.yaml), [Dockerfile](../../../docker/Dockerfile), [normal.conf](../../../docker/supervisor/normal.conf): lowercase, loopback, launchers não root, 30s filho/65s Compose | isolated_supervisor_named_and_collective_program_operation; sintaxe shell/Python e Compose | Config/entrypoint/launchers instalados idênticos; controles individuais/coletivos/interativos, stop intencional e recuperação de saída inesperada em socket privado | V; sem stop/kill compartilhado; ambos aliases exercitados interativamente em isolamento |
| Frontend de apresentação | [Vue](../../../frontend/src/App.vue), [playback](../../../frontend/src/composables/usePlayback.ts), tokens CSS: ilustrações locais, timers canceláveis e modal | [13 testes de browser](../../../frontend/tests/experience.spec.ts) | Chromium 156.0.8078.4 em 320/375/768/1024/1920px; screenshots, teclado, movimento reduzido, conteúdo longo, recursos/favicon | V código isolado; Vite compartilhado com cache antigo D/B até restart; Firefox/WebKit U |
| Integração frontend/backend real | Não existem adapter fetch/XHR, proxy ou integração CORS | Browser observa zero fetch/XHR; código/bundle revisados | Frontend de apresentação verificado; integração real com backend não implementada. | P; sem bearer em VITE_*; nenhum dashboard operacional adicionado |
| CI/documentação | [workflow](../../../.github/workflows/ci.yaml), documentação/catálogos bilíngues e validator da Library | Format/Clippy/tests/build/rustdoc locais; validator e 18 testes | Instalação/testes Chromium agora configurados na CI; links locais conferidos | V local/configuração; U CI remota |
| Consumer/producer/produção | Ausentes API de produtor transacional, consumidor/dedupe de negócio, TLS/HA/backup/métricas produtivos | Preview futuro rotulado | Zero consumers compartilhados esperado; mounts inspecionados, sem teste de backup/restore | P; aceitação de produção/desempenho N ao fechamento M3 |

## Revisão arquitetural e limites

### Fronteira precisa da apresentação/integração

Vue importa fixtures locais tipadas; não tem adapter de requests, proxy de desenvolvimento, política CORS nem fonte de medições live. Pulso/tempos ilustrativos são rotulados e consumer é preview futuro. Backend oferece GET /health sem autenticação (liveness), POST /operator/replays opt-in (reabilitação durável autorizada) e GET /operator/replays/{request_id} por ator (observação de recibo). Nenhum fornece a projeção de entrega somente leitura necessária à apresentação. Não se alega teste de integração browser contra essas rotas privilegiadas.

Follow-up limitado: primeiro definir projeção autenticada somente leitura, campos permitidos sem payload/credenciais, ID estável, interpretação lifecycle/posse, timestamp de observação, paginação/limites e respostas indisponíveis/incertas. Depois um adapter frontend com distinção live/simulado, credenciais no servidor, cancelamento e proteção contra resposta antiga. Definir same-origin ou CORS restrito de leitura separadamente; rotas privilegiadas replay permanecem fora dessa fronteira browser. Não adicionar dashboard, provedor de login nem controles destrutivos durante fechamento.

Valores de domínio validam histórico, identidade relacionada à posse e precisão de microssegundos; transporte valida credenciais/intenção/body; ports da aplicação são focados e independentes de drivers; composição controla pools/lifecycle. PostgreSQL implementa locks/relógio/representação/erros; Lapin controla conexão/channel/confirms. Helpers concretos reutilizam início de transação, relógio autoritativo, decodificação de timestamp/JSON e classificação de commit. Não foi necessária reestruturação cosmética nem atualização de dependências.

Foram revisados aritmética/conversões, predicados de revisão/posse, estados terminais, política/reconexão, cancelamento, admissão de tarefas e formatação sanitizada. Dead-letter não conta como sucesso e incerteza não é repetida automaticamente. Capacidade representável não promete progresso infinito: revisão esgotada ou dados não suportados exigem análise do operador. O leitor pode observar pending com lease; não é port de aquisição. Payload e histórico outbox/auditoria não têm política de tamanho/retenção; são lacunas explícitas de hardening.

## Correções desta aceitação

- Reproduzidos doze timeouts no browser após npm ci: servidor reutilizado em 5173 devolvia `504 Outdated Optimize Dep` para Vue. Aceitação passou a controlar porta loopback 5174, exigir servidor novo e usar cache Vite distinto. Nenhuma asserção foi enfraquecida. Nova regressão verifica recursos, console, favicon e ausência de API.
- CI frontend só executava type-check/build, omitindo a suíte de browser existente. Instalação Chromium e npm test configurados; execução remota não alegada.
- Novo controle interativo Supervisor pelos dois aliases, permanência do stop intencional e restart após matar filhos http/relay privados exatos. Falha inicial de ordenação do novo teste corrigida: aguardar startup e observar/dar ACK à mensagem fixture antes dos controles. Nenhuma alteração de comportamento produtivo.
- Novas regressões reais de morte de processo após schedule, terminalização e replay, com último orçamento, conteúdo/timestamps, token novo e um recibo histórico. Adicionados replay concorrente com descoberta antiga/aquisição e down destrutivo da auditoria.
- Novo check reproduzível da CLI SQLx para ledger/checksums/upgrades/down e cleanup do banco exato. README atual corrigido: discovery/replay/retry já implementados; HTTP com replay habilitado conecta ao banco. Revisões históricas mantidas com datas.

## Contagens efetivamente executadas

| Check | Evidência final |
| --- | --- |
| Testes Rust padrão | 69 passaram; 88 entradas opt-in ignoradas; zero doctests. Ignorado não significa passou. |
| PostgreSQL | 63 funções substantivas: repository 15, schema de posse 1, ownership 12, retry 18, discovery 7, replay 10. Última execução replay inclui as duas regressões novas. |
| RabbitMQ | 2 publisher + 1 integração unitária de conexão própria passaram. |
| Worker smoke | 5 passaram, incluindo Supervisor isolado direto/interativo, stop intencional, saída inesperada e os dois schemas ausentes no startup. |
| Recuperação integrada | 17 passes reportados = 16 funções substantivas + helper boundary_child sem efeito sem instrumentação. Função nova contém três agendas de kill/restart; crash pré/pós-confirmação contém duas. |
| Frontend | npm ci, type-check/build e 13 funções Chromium passaram. As 12 falhas iniciais foram diagnosticadas; asserções finais preservadas com verificações adicionais. |
| Ferramentas | cargo fmt; Clippy estrito todos targets/features; dois binários; rustdoc estrito; Compose; sintaxe de 5 shells e 6 Python; upgrades/checksums/down/re-up SQLx reais. |
| Documentação | Validator Library: zero erros, 18 avisos existentes de links restritos a fixtures; 18 testes sintéticos passaram. git diff --check nos dois repositórios. |

Contagens por função, não por condição interna. Suítes PostgreSQL/publisher/smoke sem mudanças não foram repetidas para inflar números. Aceitação nova executada no código final dos testes. Sem alegação de CI remota, benchmark, certificação de acessibilidade, Firefox/WebKit, backup/restore, power loss ou deploy produtivo.

## Operação compartilhada versus artefatos testados

PostgreSQL 18.6 e RabbitMQ 4.3.6 saudáveis; outbox compartilhada sem linhas. Quatro checksums SQLx instalados coincidem com o código: 20261007000000, 20261007175358, 20261009000000 e 20261012000000. A quinta, 20261013000000, passou isoladamente e permanece ausente no histórico compartilhado. Credenciais replay não configuradas. Não houve migração, troca de credenciais, reparo de linha, publicação, consumo, purge ou reset de volume compartilhado.

Topologia esperada e real: vhost `relay`, exchange direct durável `relay.events`, fila classic durável `relay.local` com `x-queue-type=classic`, binding com routing key `event`. Listagem final contém somente relay.local; ready=0, unacknowledged=0, consumers=0. Sem DLX/DLQ declaradas: dead_letter SQL é outro mecanismo. API management autenticada e permissões configure/write/read `^relay[.].*` verificadas. Login pelo formulário Chromium e página da fila verificados separadamente. Management host continua localhost:15672. Publisher e consumer são funções distintas.

Supervisor manteve http PID 11 e relay PID 12 durante toda a tarefa; SHA-256 dos executáveis carregados:

- http: `36479025eaf07eb005fd40e843741a4a023215e2d97775bf0455aaba54d8a721`
- relay: `eb0ab22cbff0e2915b672c1ac1da1bd07d66d3855fc8ca0543d34de6bca996aa`

Revisão exata carregada não observável: os binários não incorporam revisão e hashes divergem dos builds atuais. Cargo test padrão compila no `/app/target` configurado e substitui os artefatos em disco; os processos mantiveram inodes antigos, agora `(deleted)`. Também passaram builds separados em `/tmp/relay-m35-target`. Nenhum filho compartilhado foi reiniciado, mas os artefatos do target padrão foram atualizados; um restart futuro carregará os novos arquivos. Compilação não comprova rollout.

O Vite compartilhado foi preservado. Após npm ci ainda devolvia URL otimizada antiga do Vue; a apresentação atual passou na porta isolada 5174. Restart deliberado do servidor de desenvolvimento fica pendente em 5173 após instalação de dependências. É lacuna real da operação local atual, separada da aceitação do código.

Depois do cleanup final não restavam bancos de teste, topologia RabbitMQ própria nem diretórios temporários recovery/Supervisor. Observadores deram ACK somente em filas próprias; fila compartilhada intacta. Caches, logs e screenshots ignorados são artefatos de verificação, não arquivos versionados. Mounts persistentes não comprovam backup nem durabilidade produtiva.

## Comandos reproduzíveis e pré-requisitos de rollout

Na raiz, com serviços existentes e permissões CREATE DATABASE/broker do projeto. Em validações futuras use target separado e não reinicie filhos compartilhados:

```sh
docker compose exec -T -e CARGO_TARGET_DIR=/tmp/relay-acceptance-target app cargo fmt --check
docker compose exec -T -e CARGO_TARGET_DIR=/tmp/relay-acceptance-target app cargo clippy --locked --all-targets --all-features -- -D warnings
docker compose exec -T -e CARGO_TARGET_DIR=/tmp/relay-acceptance-target app cargo test --locked
docker compose exec -T -e CARGO_TARGET_DIR=/tmp/relay-acceptance-target app cargo test --locked --test postgres_repository --test delivery_ownership_schema --test postgres_ownership --test postgres_retry --test postgres_discovery --test postgres_replay -- --ignored --test-threads=1
docker compose exec -T -e CARGO_TARGET_DIR=/tmp/relay-acceptance-target app cargo test --locked --test rabbitmq_publisher -- --ignored
docker compose exec -T -e CARGO_TARGET_DIR=/tmp/relay-acceptance-target app cargo test --locked --lib infrastructure::rabbitmq::tests::closed_owned_connection_is_unavailable -- --ignored
docker compose exec -T -e CARGO_TARGET_DIR=/tmp/relay-acceptance-target app cargo test --locked --test worker_smoke compiled_worker_confirms -- --ignored
docker compose exec -T -e CARGO_TARGET_DIR=/tmp/relay-acceptance-target app cargo test --locked --test integrated_recovery -- --ignored --test-threads=1
docker compose exec -T -e CARGO_TARGET_DIR=/tmp/relay-acceptance-target app cargo build --locked --bins
docker compose exec -T -e CARGO_TARGET_DIR=/tmp/relay-acceptance-target -e RUSTDOCFLAGS='-D warnings' app cargo doc --locked --no-deps
python3 -B scripts/check-migrations.py
sh scripts/check-postgres.sh
python3 scripts/check-rabbitmq.py --require-topology
docker compose config --quiet
```

A execução completa das cinco funções worker_smoke usou o target padrão do Docker: o launcher Supervisor fixa `/app/target/debug`. Para executá-la com target separado é preciso configuração isolada apontando ambos os programas aos artefatos escolhidos; CARGO_BIN_EXE não altera esse launcher. Não executar scripts/check-supervisor.py nos serviços compartilhados durante auditoria: ele para/mata filhos intencionalmente. Comandos normais diretos/interativos em [rollout local](local-relay-rollout.md).

Frontend, Node 22.12+ nativo ou container existente: `npm ci`, `npm run type-check`, `npm run build`, `npx playwright install --with-deps chromium`, `npm test`. Testes controlam exclusivamente porta 5174 e cache próprio. Alteração das dependências de um Vite ativo exige restart coordenado separado. Nunca colocar bearer replay em VITE_*.

Rollout replay compartilhado depende de autorização específica: revisar migração 5/privileges, decidir backup/recuperação, injetar credenciais confiáveis no servidor, validar fronteira de rede/TLS local, aplicar migração explícita, instalar/reiniciar artefatos identificados e observar ledger/health/auditoria/recibo independentemente. Worker exige migração 4; HTTP liveness não precisa de banco; HTTP replay exige migração 5. Esta revisão não autoriza ativação privilegiada compartilhada.

## Leitura guiada e próximo marco

Comece pelo exemplo normal isolado de [integrated_recovery](../../../backend/tests/integrated_recovery.rs): ID inserido, aquisição, observação JSON/message_id e SQL processed independente. Depois siga [composição](../../../backend/src/bin/relay.rs) → [worker](../../../backend/src/application/worker.rs) → [deliver_once](../../../backend/src/application/delivery.rs) → [contratos de posse](../../../backend/src/persistence/ownership.rs) / [publisher](../../../backend/src/application/publisher.rs) → adapters PostgreSQL/RabbitMQ. Use a matriz para relacionar cada invariante à regressão.

Compare cinco agendas concretas: crash após aceitação; dono expirado versus token novo; mandatory return e elegibilidade atrasada; prefixo inseguro versus isolamento com autoridade nova; replay autorizado com ACK COMMIT perdido e recibo repetido. Leia [ADR 007](../adr/007-durable-delivery-ownership.md), [008](../adr/008-durable-failure-policy.md) e [009](../adr/009-authorized-idempotent-replay.md) com as observações dos testes. Aprendizagem Rust: newtypes, Result/match, traits com futures Send, cancelamento async e ownership. Decisões distribuídas: fronteiras de commit, leases, identidade estável, incerteza e dedupe downstream. A apresentação ilustra; não comprova runtime.

Marco 4 começa com consumidor demonstrativo em topologia isolada, prefetch limitado e ACK manual depois de transação local atômica. Definir efeito local e registro dedupe único pelo ID estável, commitá-los juntos e validar duplicatas, concorrência, crash antes de commit, crash depois de commit/antes de ACK, reconexão e redelivery. Preservar identidade original; ACK de duplicata só após observar o registro commitado. Replay com o mesmo ID pode intencionalmente produzir **nenhum efeito adicional** se o consumidor já commitou essa identidade. Efeitos externos precisam de protocolo próprio e não herdam atomicidade local. API produtora, dashboard real, reparo, throughput, retenção, HA/TLS e hardening produtivo continuam em escopos separados.

Fontes primárias consultadas: [locks PostgreSQL 18](https://www.postgresql.org/docs/18/explicit-locking.html), [confirms RabbitMQ 4.3](https://www.rabbitmq.com/docs/confirms) e [CI Playwright](https://playwright.dev/docs/ci). Explicam mecanismos; somente a execução registrada acima sustenta o comportamento testado do projeto.
