[English](../../en/operations/milestone-5-3.md) | [Catálogo](../index.md)

# Marco 5.3 — medições reproduzíveis e aceitação

Protocolo definido antes da carga em 17/10/2026. Campanha final: 22 experimentos finitos aprovados; execuções de desenvolvimento separadas, sem misturar resultados. Base `bde9cba132e8369e71395b7872586fc1a840b08e`, árvore inicialmente limpa. O diff acrescenta teste, executor, evidências/documentação e corrige um comentário de `.env.example`; nenhum código produtivo, dependência ou migração foi alterado. Não houve stage, commit ou push. Somente rust-event-relay foi alterado; Engineering Library e Viva Bem permanecem fora do escopo.

## Pré-requisito e rollout 5.2

Consulta independente confirmou main remoto no SHA exato da base. O [run 39031053588](https://github.com/juniocr26/rust-event-relay/actions/runs/39031053588) concluiu com sucesso check, documentation, frontend, postgres-integration, rabbitmq-integration e integrated-recovery. Isso valida remotamente 5.2, não o diff 5.3.

Inicialmente HTTP/relay/consumer estavam RUNNING e carregavam os hashes 5.1 registrados em [5.2](milestone-5-2.md); replay desabilitado. Revisado o orçamento PostgreSQL: relay 2 conexões de negócio + 1 observador; consumer 2 + 1; HTTP sem replay 0, ou 2 + 1 se habilitado. São até seis conexões de aplicação no cenário atual, sem cota global; max_connections=100 e 11 sessões totais no instante inicial não representam reserva de capacidade.

Capturados os estados nominais, filas vazias e demonstração preservada. Build limpo/locked dos três binários em `/app/target/m52`; parada graciosa apenas dos programas nomeados consumer/relay/http; cópia dos artefatos para os caminhos instalados; partida dos mesmos três programas. Nenhuma recriação de Supervisor ou dependências, alteração de credencial/topologia ou migração. Não houve ativação acidental por autostart. Hashes de `/proc/<pid>/exe` iguais aos artefatos selecionados:

| Programa | SHA-256 carregado 5.2 |
| --- | --- |
| HTTP | `3cc33cdb91c2fef41c605f2e9d279c56e23a5d8aa6f9619110f0d7bca1910239` |
| Relay | `f6a2d34f680fefdbc7b0ef3fd94fac204727cdeebd347d3be8a3738baaa98f9c` |
| Consumer | `fd1fbd2ecdd9e999667e696dd3af9fdac3809a8a3568e3f4b4d68f610e81972c` |

Os três `/ready` e `/metrics` responderam 200 com papel/observações corretos; `/health` manteve `200 ok`. HTTP sem replay usa timestamps de dependências null, não zeros de sucesso inventados. Não existe atestado de revisão embutido: build locked de fonte limpa e igualdade de hashes são a evidência disponível. A composição usada por `cargo test` unifica features de dependências de desenvolvimento e pode produzir hashes diferentes do build dev simples instalado, embora a fonte produtiva seja a mesma 5.2. Nada do working tree 5.3 foi instalado nos processos compartilhados.

## Protocolo e limites

Reprodução: `python3 scripts/measure-isolated.py campaign`. O [executor](../../../scripts/measure-isolated.py) cria rede/containers com nomes UUID e labels de propriedade, sem portas publicadas ou credenciais compartilhadas. PostgreSQL 18.6-bookworm: 1 CPU, 512 MiB RAM e tmpfs de dados 256 MiB. RabbitMQ 4.3.6-management: 1 CPU, 768 MiB RAM e tmpfs 256 MiB. Runner: 2 CPUs/2 GiB; logs Docker até 2 MiB por container. Serviços dedicados usam defaults das imagens e credenciais descartáveis; não copiam a política de permissões do broker compartilhado. Docker Desktop e cache/build montados ainda disputam recursos físicos do host.

Exige 3 GiB livres; startup até 100s; runner até 1500s; caso até 35s dentro do limite de 45s da fixture; observer interno da campanha até 600s. Limites de memória/tmpfs interrompem excesso; não existe carga até falhar. Cleanup normal ou de falha remove apenas nomes exatos com propriedade conferida, volumes anônimos e rede. Morte do host/daemon exige inspeção desses nomes; não se promete cleanup após perda do próprio executor.

Ambiente observado: Rust 1.95.0, Cargo 1.95.0, SQLx 0.8.6, Lapin 4.12.0, perfil dev sem otimização/com debuginfo, `--locked`, target separado `/app/target/m52`. VM aarch64 Linux 6.12.76-linuxkit com 10 CPUs lógicas e 8.321.515.520 bytes; SC_CLK_TCK=100. [Evidência JSON finita](../../en/testing/milestone-5-3-results.json) contém hashes da fonte/lockfile/binários de teste, identidades das imagens e todos os registros. Não é equivalência de desempenho com release ou runner CI.

Sete cenários com três repetições independentes: baseline com chaves distintas; 80 originais + 80 entregas duplicadas adicionais (50% das entregas desse cenário são duplicatas); mesma chave com dois consumers isolados e barreira de estoque; identificadores maiores suportados; consumer pausado; linhas pending bloqueadas; tipo inválido com quarentena. Cada repetição tem banco/topologia novos, um relay e um consumer, exceto contenção com dois consumers sequenciais. Exige observar dois waiters reais nesse cenário; nenhuma concorrência produtiva foi aumentada. Cada processo admite um handler, prefetch=1, pool de negócio=2 e observador=1. Harness usa pool até 2, admin até 2; barreira de contenção/freshness acrescenta uma conexão.

Oito eventos válidos de aquecimento drenam separadamente; depois 80 IDs únicos, quantidade 1, três campos de payload válidos ou tipo não suportado. A distribuição é uma chave por evento, exceto os cenários de mesma chave; identificadores de até 128 bytes permitem envelope maior válido, sem acrescentar campos rejeitados pelo consumer. Um teste adicional, com uma repetição, bloqueia o estoque por 17s após admissão com budgets statement/lock/operação de 30/25/30s; exige `/ready` real 503 broker_stale e recuperação para 200. É regressão de estado, não amostra de capacidade estável.

Gerador completion-paced: somente um INSERT em voo, sem fila de tarefas, aguardando o restante de 20ms entre inícios (teto nominal 50/s); rajadas Paused/Locked usam 2ms (teto 500/s). Inserções lentas reduzem chegadas; contagem generator_late e taxa efetiva ficam explícitas. Não é open-loop independente. Nenhum ID propositalmente rejeitado/não admitido; falha SQL aborta. Vetores/mapas de 80 IDs e observações limitadas, sem histórico ilimitado de payload/log. Consumer pausado retoma 750ms depois das admissões; Locked pausa apenas relay próprio para semear, bloqueia as 80 linhas, retoma relay e libera após 750ms. SKIP LOCKED não precisa gerar waiter. Contenção libera a barreira ao fim da admissão. Amostras com falha/barreira não são misturadas ao baseline.

Tempo usa Instant monotônico do mesmo runner. Latência observada do recibo vai do início da tentativa de INSERT à primeira leitura do recibo durável; inclui inserção, polling e escalonamento. Poll nominal 20ms mais queries/observações sequenciais. Percentis nearest-rank p50/p95, população de 80 recibos únicos por repetição válida; não se faz média de percentis, não se anuncia p99. Inválidos têm zero recibos e percentis null. Nenhuma subtração de timestamps de relógios entre processos. Histogramas produtivos continuam aproximados por buckets. Repetições abortadas são censuradas e excluídas, não transformadas em sucesso.

Reconciliar separadamente INSERT, aquisição reconhecida, confirmação do broker, conclusão SQL reconhecida, Applied/Duplicate, quarentena confirmada e retorno do envio de ACK. Esse retorno não prova recebimento do ACK pelo broker. SQL dead_letter difere da quarentena RabbitMQ. Fila ready vazia isoladamente não prova unacked=0; ACKs observados e fechamento das conexões próprias complementam a evidência. Replay fica desabilitado na campanha e no ambiente compartilhado; regressões autorizadas de replay usam fixtures isoladas.

Observador exclusivo de teste lê no máximo os 88 recibos da fixture, nunca backlog produtivo. Amostra RSS/ticks por processo na ordem relay/consumer(s), sessões/waiters do banco medido e ready por declaração passiva; retém apenas resultados limitados. Outro observador consulta management dedicado a cada 200ms, com deadline 1s e resposta até 128 KiB, guardando máximos/contagens. Estatísticas são cacheadas; picos breves podem desaparecer. RSS/ticks amostrados não são teto de memória nem utilização de todo serviço. CPU/RSS por serviço e pico de disco não foram medidos; limites aplicados não viram utilização inventada. Lapin materializa corpos antes da validação, SQLx materializa JSON; backlog, recibos e logs podem crescer sem limite de retenção.

## Resultados finais

A tabela apresenta mínimo–máximo entre repetições, sem calcular média de percentis. IDs/s de admissão usa inserções reconhecidas/intervalo real; taxa seguinte usa IDs únicos/intervalo até o último retorno de ACK, incluindo drenagem. Em Invalid isso significa quarentena+ACK, sem efeito de estoque. Duplicates inclui publicação adicional depois dos originais; drain inclui essa etapa. Freshness tem uma amostra com espera intencional.

| Cenário | Repetições | Admissão IDs/s | IDs únicos/intervalo até ACK, /s | Recibo observado p50 / p95, s | Drenagem após liberação, s |
| --- | --- | --- | --- | --- | --- |
| Baseline | 3 | 43.8–44.3 | 35.1–35.3 | 0.623–0.636 / 0.974–0.978 | 0.440–0.472 |
| Duplicates | 3 | 43.1–44.1 | 32.9–33.1 | 0.603–0.627 / 0.973–0.980 | 0.573–0.612 |
| Contended | 3 | 43.3–44.0 | 36.0–36.3 | 0.987–1.026 / 1.740–1.770 | 0.363–0.389 |
| Larger | 3 | 43.6–43.9 | 34.9–35.1 | 0.618–0.621 / 0.981–0.985 | 0.443–0.465 |
| Paused | 3 | 249.6–259.7 | 59.9–63.9 | 1.015–1.059 / 1.071–1.110 | 0.177–0.242 |
| Locked | 3 | 254.7–274.7 | 34.7–35.4 | 1.992–2.036 / 2.015–2.050 | 1.190–1.249 |
| Invalid | 3 | 43.6–43.8 | 34.9–35.2 | N/A (0 recibos) | 0.444–0.455 |
| Stale | 1 | 43.2 | 4.2 | 18.041 / 18.783 | 0.199 |

Totais medidos: 1.760 IDs únicos, aquisições reconhecidas, confirmações de publicação pelo relay e conclusões SQL reconhecidas; 1.520 Applied, 240 Duplicate, 240 registros exatos de quarentena confirmada e 2.000 retornos de envio de ACK. Os 176 eventos de aquecimento não entram nesses números. O gerador de duplicatas obteve separadamente 240 confirmações de publicação; não são contadores do relay. Todos os IDs válidos encontrados nos recibos, estoque reconciliado, cada inválido com corpo/ID original exatamente uma vez na quarentena inspecionada, dead_letter=0 e ready final=0. Sem resto inexplicado nesses experimentos; não é prova de perda impossível ou transporte exatamente uma vez. Nenhuma operação final rejeitada ou expirada; uma inserção Paused ultrapassou o intervalo nominal e foi registrada, sem catch-up oculto.

Baseline admitiu 43,8–44,3/s apesar do teto nominal 50/s; 80 IDs reconciliados em 2,267–2,279s, ou 35,1–35,3/s. Não é capacidade máxima. p95 observado 0,974–0,978s é compatível com espera ociosa configurada de 1s e inclui polling; não é latência exata do COMMIT. Envelope baseline 293–294 bytes, maior válido 791, tipo inválido 280–281; nenhum teto de memória para corpos grandes foi inferido.

Paused acumulou 15–30 mensagens ready amostradas; 15/25/29 na retomada, drenagem 0,177–0,242s. Locked reteve 80 linhas e não gerou waiter SQL, conforme SKIP LOCKED; drenou 1,190–1,249s após liberação, incluindo próxima aquisição ociosa. Contenção observou dois waiters em todas as três repetições e estoque 80 dos IDs medidos, além do aquecimento separado.

RSS máximo amostrado por processo: relay 19.192–19.268 KiB; consumer comum 27.020–27.120 KiB; consumers de contenção 26.892–26.964 e 26.896–26.928 KiB. Soma de CPU entre primeiras/últimas amostras: 0,21–0,38 CPU segundos por experimento; freshness 0,32s durante aproximadamente 19s. Máximo de sessões no banco medido: comum 8, contenção 12, freshness 9, incluindo harness/barreira e excluindo sessões do banco admin. Picos mais curtos podem escapar.

Management teve 179 amostras utilizáveis, zero falhas de requisição e máximos cacheados ready=79/unacked=1. Filas de vida curta podem desaparecer antes da atualização; zeros nesse observer não provam ausência de trabalho anterior. Declaração passiva e reconciliação durável prevalecem nesses casos. Freshness manteve um delivery bloqueado, observou broker_stale depois de 15s com sampler de banco independente, liberou a barreira e exigiu prontidão e reconciliação final; os 17s intencionais explicam percentis próximos de 18s.

Perda temporária de conexão, resposta COMMIT perdida, confirmação/ACK perdidos, reconexão e shutdown também foram exercitados pelas suítes integrated_recovery/consumer_runtime/worker_smoke em serviços dedicados. São experimentos com assertions/barreiras e deadlines próprios (por exemplo reconexão de 100ms em fixtures), sem percentis misturados ao steady state ou promessa de recuperação produtiva. Saturação cobre try-lock real do publisher e liberação por cancelamento, dois corpos HTTP de replay retidos e quatro sockets do exporter com quinto aguardando capacidade. Não mede saturação open-loop sustentada.

## Matriz de aceitação e cobertura

V=verificado no ambiente finito indicado; U=implementado mas não verificado; D=defeituoso; B=bloqueado por configuração/rollout; P=planejado e ausente; N=não aplicável. Referências históricas [M1](milestone-1-review.md), [M2](milestone-2-review.md), [M3](milestone-3-review.md), [M4](milestone-4-4-review.md) e [4.5](milestone-4-5-rollout.md) preservadas. A [matriz inglesa](../../en/operations/milestone-5-3.md) contém links individuais de cada implementação/teste; caminhos abaixo são relativos a backend.

| Requisito/invariante | Fonte/configuração | Cobertura automatizada/infra real isolada | Runtime compartilhado | Classe/limitação |
| --- | --- | --- | --- | --- |
| M1 HTTP/config/sinais | main.rs, config.rs, application.rs | Units, lifecycle, sockets/sinais, HTTP com replay sob degradação DB | health 200, replay off | V; liveness não garante entrega |
| Envelope/identidade/JSON/tempo | domain/event.rs, postgres/mod.rs | event_envelope, postgres_repository, restauração AMQP/números/timestamps | Evento original preservado | V; precisão PostgreSQL diferente de nanos do transporte |
| Schema/migrações/snapshots | migrations, persistence | Repository/schema, seis upgrades/checksums/down/re-up isolados | Seis checksums iguais | V; snapshots sem autoridade, limite de linhas não limita bytes/custo |
| M2 publicação obrigatória/persistente/confirmada | infrastructure/rabbitmq.rs | publisher, retornos/rejeições/confirmações incertas | Exchange direct/fila durable | V; aceitação não é consumo |
| Aquisição/token/lease/autoridade vencida | postgres/ownership.rs | Ownership/concorrência/expiração/kill | Demo processed/attempt=1 | V isolado; token protege SQL, não publicação externa; sem renovação |
| Confirmação antes de conclusão | application/delivery.rs | delivery_use_case e recovery com observações independentes | Demo existente, sem republicação | V; transações DB/broker separadas |
| Worker sequencial/reconexão/drain | worker.rs, bin/relay.rs | Worker, smoke, proxies, signals, Supervisor isolado | RUNNING/hash carregado | V; prazos cooperativos, sem garantia de fairness |
| M3 orçamento/jitter/retry/terminal | retry_policy.rs, postgres/retry.rs | Retry policy/state/SQL e sucesso na tentativa final/kill | Sem falhas compartilhadas | V isolado; SQL dead_letter preserva histórico |
| Discovery/isolamento com autoridade fresca | postgres/discovery.rs, maintenance.rs | Prefixo inseguro/live-owner/revisão concorrente | Lista manual vazia | V isolado; pode materializar/varrer, não é sampler de backlog |
| Replay autorizado/idempotente/auditado | postgres/replay.rs, operator_http.rs | Application/HTTP/SQL e ciclo até entrega | Schema instalado, REPLAY_OPERATORS vazio | V isolado; B compartilhado intencional; identidade/efeito mantidos |
| M4 estoque/recibo atômico/dedupe/conflito | postgres/consumer.rs, migração consumer | Concurrency/constraints/grants/COMMIT perdido, campanha | Estoque 15 e recibo 10+5→15 | V; transação local, escritor privilegiado pode burlar proteções |
| Consumer/prefetch/recuperação | rabbitmq/consumer.rs, bin/consumer.rs | Processo real, cancelamento/ACK/reconexão/pausa | Um consumer, prefetch1, RUNNING | V; um handler por processo; réplicas multiplicam orçamento |
| Quarentena exata antes do ACK | consume.rs, rabbitmq/properties.rs | Inválido/conflito/limite/confirmação perdida/oversize; IDs/corpos exatos | Quarentena durable vazia, sem consumers | V; cópia duplicada possível; preservação não suportada interrompe progresso |
| Supervisor/launchers/shutdown | docker/supervisor, compose.yaml | Smokes nominais/interativos e sinais/grupos isolados | Config instalada igual, estados preservados, 30s/95s | V; autostart pode retomar após recriação |
| M5.1 pools/prefetch/admissão | Contrato 5.1/config/adapters | Gate publisher, replay saturado, barreira SQL/prefetch real | Defaults exportados, replay off | V; sem teto global de HTTP/bytes de processo |
| Métricas/exporter finitos | telemetry/metrics.rs, observation.rs | Labels/reset/cancel, quatro clientes reais, quinto aguarda; roles compilados | Três exporters corretos | V; reset no restart, buckets sem p99 exato, falha scrape sem efeito de negócio |
| M5.2 readiness/freshness/single-flight | telemetry/readiness.rs, application/readiness.rs | Locks/deadlines/HTTP/reconexão reais, processo stale/recuperação | Três roles frescos após rollout | V; probe não prova futuras mutações/roteamento; sem query por scrape |
| M5.3 medição/cleanup limitado | capacity_measurements.rs, measure-isolated.py | Repetições finitas/reconciliação/recursos/cleanup | Sem carga/falhas no compartilhado | V para casos registrados; sem capacidade máxima |
| Frontend Vue/apresentação | frontend components/playback/tests | npm ci/type/build, 13 Chromium, cinco larguras, foco/Escape/timers/replay/reduced-motion/wrapping/favicon/rede | Vite compartilhado preservado | V apresentação; sem backend live |
| API live/integração produtor-outbox | Sem fetch/xhr ou endpoint de negócio | Browser confirma zero chamadas; sem teste de integração inventado | Origem da demo é fixture operacional | P; integração de produto ausente, não defeito implementado |
| Replay/repair de quarentena/outro destino | Não implementados | Nenhuma integração afirmada | Sem ativação | P se selecionados posteriormente |
| HA/TLS/permissões/backup automatizado | Ambiente de desenvolvimento | Grants isolados, restore manual histórico | DB superuser/CREATEDB | U para implantação produtiva; dependem de objetivos |
| Prometheus/Grafana/coletor de backlog | Somente exporters locais | Nenhum sampler caro por scrape | Sem coletor implantado | P se escolhido; desconhecido não vira zero |
| Pagamentos/efeitos externos/certificação completa de acessibilidade | Fora do escopo | Nenhuma certificação inventada | Não aplicável | N |

## Configuração, correções e limites operacionais

Nenhuma variável obrigatória vazia, credencial change_me, segredo específico ausente, override de shell ou `.env` local de backend/frontend observado. Dotenv raiz continua privado; processo precede fallback dotenv; interpolação Compose não é injeção automática. APP_ENV/HTTP_ADDR/RUST_LOG ainda usam dotenv/default. Container antigo não injeta os três endereços dos exporters, mas defaults loopback validados equivalem aos valores desejados; recriação só para isso é desnecessária. Mudanças futuras de endereço exigem revisão explícita de injeção/portas/orçamento. Exporters não publicados; nenhum VITE_* ou segredo no navegador.

Config Supervisor instalada igual à fonte. Defeito documental confirmado: comentário `.env.example` dizia autostart=false após rollout true; corrigido sem mudar política. Formatos/ranges permanecem no [inventário](../docker/environment-inventory.md). Nenhuma credencial criada, exibida ou rotacionada. REPLAY_OPERATORS/maintenance vazios são intencionais.

Filas source/quarentena classic durable, exchanges direct/bindings esperados, sem auto-delete/políticas/operator policies. Source ready/unacked=0/0, um consumer; quarentena 0/0, zero consumers. Management autenticado/permissões configure/write/read `^relay[.].*` verificados sem consumir mensagens compartilhadas. Nenhum purge/basic.get/publicação de teste/migração no banco compartilhado.

Na primeira campanha, três baselines passaram antes da fixture duplicada falhar: o timestamp original em nanos diferia da representação persistida. Consumer classificou corretamente conflito. Fixture agora normaliza para micros antes de inserir e confirma representação idêntica; assertions mantidas. Essa execução abortada é excluída. Campanha de desenvolvimento posterior passou 21 casos; campanha final reforçou contenção e acrescentou stale real, passando 22 em 100,64s. Somente registros finais tabulados.

Primeiro wrapper de verificação impôs RUST_LOG=error e provocou 11 falhas em barreiras de log info, mesmo com conclusões duráveis em alguns casos. Serviços descartáveis sem OOM/alarme e com espaço livre. Wrapper final usa info para verificação e mantém error para campanha; nenhuma assertion/deadline/default produtivo alterado. Evidência guarda digests do script medido e final; a correção afeta só a verificação. Recursos das execuções malsucedidas também removidos.

## Reprodução e validação final

```sh
# Ler/revisar o protocolo finito antes da execução.
python3 scripts/measure-isolated.py campaign
python3 scripts/measure-isolated.py verification
python3 scripts/check-migrations.py
docker compose config --quiet
sh scripts/check-postgres.sh
python3 scripts/check-rabbitmq.py --require-topology
python3 -B scripts/check-docs.py
python3 -B -m unittest discover -s scripts -p 'test_*.py'
git diff --check
```

Executor inclui fmt; Clippy estrito all-targets/all-features; testes padrão; suítes explícitas PostgreSQL ownership/retry/discovery/replay/consumer/readiness, RabbitMQ, recovery, worker/Supervisor; builds dos três binários e rustdoc estrito. Para imediatamente se um comando falhar. Resultados: **102 testes padrão passaram, 128 ignorados explicitamente, zero doc-tests**; **127 entradas opt-in passaram, das quais 125 substantivas e duas entradas auxiliares no-op**; campanha é **um teste opt-in adicional com 22 experimentos**, não 22 novos testes Rust. Contagens não incluem repetições de desenvolvimento. [Resultados completos](../testing/validation-results.md).

Frontend usa cópia limpa/servidor Vite acceptance próprio em 5174, sem compartilhar node_modules/cache/servidor de desenvolvimento:

```sh
docker run --rm --cpus 2 --memory 2g \
  --mount "type=bind,src=$(pwd)/frontend,dst=/source,readonly" \
  node:22-bookworm-slim sh -c '
set -eu
mkdir /tmp/frontend
cd /source
tar --exclude=node_modules --exclude=dist --exclude=test-results --exclude=playwright-report -cf - . | tar -xf - -C /tmp/frontend
cd /tmp/frontend
npm ci
npm run type-check
npm run build
npx playwright install --with-deps chromium
npm test'
```

Executado com Node 22.23.3, instalação pelo lockfile e container root exclusivo para dependências Linux do browser. Treze casos Chromium passaram nas larguras 320/375/768/1024/1920. Capturas mobile/tablet/desktop inspecionadas: landing ocupa largura, modal responsivo com scroll, foco/Escape/background lock, reset/descarte de timers/replay, reduced motion, texto longo, favicon e recursos. Sem erros de console/recurso e sem fetch/xhr; tempos simulados rotulados. Não certifica Firefox/WebKit nem acessibilidade completa. **Presentation frontend verified; live backend integration is not implemented.** Container removido, capturas somente em /tmp, frontend compartilhado preservado.

Migrações: seis upgrades ordenados/checksums/no-op/down consumer/replay/re-up em banco descartável, remoção e ausência conferidas; nenhum upgrade compartilhado. Compose válido. Scripts Python/shell e documentação verificados. Não executar checker destrutivo de Supervisor compartilhado; suítes isoladas fornecem a evidência de lifecycle.

## Fluxo de estudo e fechamento

Seguir outbox durável do produtor → aquisição reconhecida pelo relay → confirmação RabbitMQ obrigatória/roteada → conclusão SQL reconhecida → transação consumer de recibo/estoque → retorno do envio de ACK. Conclusão do relay pode anteceder efeito no consumer. Retry agenda falhas definitivas com contexto, mantendo leases/incerteza quando necessário; discovery isola só com autoridade fresca. SQL dead_letter preserva histórico do outbox; quarentena broker preserva bytes/propriedades inválidos/conflitantes antes do ACK. Identidade já receipted retorna Duplicate com efeito histórico, sem novo incremento. Replay abre ciclo autorizado/auditado do relay; não desfaz nem repete o efeito no consumer. Interface ilustra etapas, sem chamar esse pipeline real.

**Marco 5 fechado com limitações explícitas não bloqueantes no escopo definido.** A decisão usa os checks finais aprovados, não apenas a campanha. Capacidade máxima, saturação open-loop sustentada, teto de memória/disco de todo processo, utilização por serviço e percentis produtivos de latência/recuperação permanecem desconhecidos. Evidência finita cobre correção/recursos/backpressure/prontidão/recuperação nos cenários declarados. Prontidão produtiva do projeto completo não foi estabelecida; projeto não foi declarado concluído. Marco 6 permanece indefinido, sem implementação iniciada.

Decisões futuras: nenhum defeito produtivo confirmado do escopo implementado ficou aberto. Riscos operacionais: tamanho não preservável que interrompe progresso, retenção/backlog/logs sem limite, privilégios de desenvolvimento e único domínio de falha. Melhorias opcionais: campanhas open-loop mais longas com admissão explicitamente limitada, amostragem por serviço, rotação de logs e atestado de revisão. Integração live UI/produtor, replay/repair de quarentena e outro destino são capacidades ausentes dependentes de escolha de produto. TLS/HA, backups automatizados e implantação de menor privilégio dependem dos objetivos operacionais; não viram requisitos automáticos do portfólio local. Aumentar concorrência, contar backlog produtivo por scrape, injetar falhas/carga compartilhada, expor segredos em VITE, criar migração e recriar manager só para defaults iguais são desnecessários agora.

Inspeção final: HTTP PID 30565, relay 30576, consumer 30589 RUNNING, hashes carregados/instalados 5.2 preservados, endpoints prontos; config/launchers iguais à fonte. Evento original `481fb751-81d5-46b7-aea2-eed344084725` processed/attempt_count=1, estoque 15, recibo original 10+5→15/completed_at 2026-10-16 15:34:23.514788+00. Filas preservadas ready/unacked=0/0; nenhum banco/fila de fixture ou container/rede/volume próprio residual. Credenciais, dados, serviços e demonstração mantidos. Não houve rollout 5.3; alterações de teste/docs/comentário não exigem deploy. Mudança futura de binário/config continua exigindo rollout deliberado da revisão selecionada.

Referências primárias de metodologia: [histogramas/quantis e precisão por buckets](https://prometheus.io/docs/practices/histograms/), [PostgreSQL 18 locks/SKIP LOCKED](https://www.postgresql.org/docs/18/sql-select.html). Código instalado e observações reais fundamentam o comportamento específico; referências não substituem resultados.

Precisão dos campos: drain começa após retorno do comando de retomada/rollback do lock; não inclui escalonamento anterior ao retorno. Campo legado `ready_at_release` significa mensagens ready do broker antes de retomar Paused, mas linhas pending do outbox retidas até liberar Locked; são populações diferentes, nunca somadas. Definições no JSON tornam isso explícito. Respostas management sem estatísticas cacheadas utilizáveis ficam fora da contagem de amostras, sem virar zero saudável.
