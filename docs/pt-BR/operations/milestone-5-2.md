[Medições atuais 5.3, rollout 5.2 validado remotamente e fechamento do Marco 5](milestone-5-3.md). Pendências datadas abaixo são históricas.

[English](../../en/operations/milestone-5-2.md) | [Catálogo](../index.md)

# Marco 5.2 — prontidão e atualidade operacional

## Rollout validado do 5.1

Em 17/10/2026, checkout limpo e `origin/main` apontavam para `9b0323af3fad769b1d0813e0330b32e87d64a91c`. A API GitHub confirmou o [run 38974809458](https://github.com/juniocr26/rust-event-relay/actions/runs/38974809458): check, documentation, frontend, postgres-integration, rabbitmq-integration e integrated-recovery passaram nesse SHA exato. Este registro atualiza as pendências históricas de CI/rollout no documento 5.1, preservado.

Os containers inicialmente estavam parados. O usuário autorizou iniciar serviços e consumer. Foram executados início das dependências e `cargo build --locked --bins` em container temporário; na inspeção seguinte todos os serviços, inclusive app/frontend, estavam em execução. As observações disponíveis não permitem atribuir o início de app/frontend a um ator específico. Não foi necessário reiniciar/recriar app: os três executáveis carregados já coincidiam com os arquivos confirmados pelo build validado. Não houve mudança de configuração do Supervisor, credenciais, volumes, topologia ou migrations. Permaneceram seis migrations bem-sucedidas; filas vazias, source com um consumer e quarantine sem consumer.

Evento preservado `481fb751-81d5-46b7-aea2-eed344084725`: processed, attempt_count=1, estoque=15, recibo before=10/quantity=5/after=15/completed_at=`2026-10-16 15:34:23.514788+00`. Nenhuma publicação/fetch de demonstração foi usada. Consumer intencionalmente RUNNING conforme autorização desta sessão. Replay desabilitado.

SHA-256 carregado e igual ao arquivo:

| Programa | SHA-256 |
| --- | --- |
| HTTP | `77f940a7a64330e4f9d7343728ebca5a220bf3daca6fd3404a37550df83c8f26` |
| Relay | `38e872d2a9bf6a6bf7702ae4b971675c20e80e904fb9e44c1b7391bba5ba54cc` |
| Consumer | `cda31435b1b7c0acbe7377516680a67b59a9805574261855c0cda44608593dc6` |

Não há atestado de revisão embutido: a evidência combina fonte limpa, build locked e igualdade carregado/arquivo. Exporters reais retornaram 200 no namespace app, com identidade/limites corretos; `/health` retornou `200 ok`. Variáveis dos exporters ausentes na injeção do container existente: aplicaram-se defaults revisados. Não foi necessário rebuild da imagem.

## Contrato de admissão

`GET /ready` nos exporters existentes retorna 200 somente quando este processo tem evidências locais recentes para admitir seu trabalho; demais estados retornam 503. `/health` continua liveness HTTP independente de workers/banco/broker. Prontidão não garante entrega futura, conclusão de negócio, backlog vazio, durabilidade, HA ou saúde de outro processo.

| Papel | Pré-requisitos |
| --- | --- |
| HTTP | Configuração válida, exporter e listener HTTP vinculados. Sem replay, não depende de banco/broker. Replay habilitado exige credenciais, preflight inicial e observação recorrente do banco/schema de replay. |
| Relay | Configuração/preflight válidos, admissão iniciada, observação recente do banco/schema outbox e publisher/canal utilizáveis. Aquisição vazia pode estar pronta. |
| Consumer | Configuração/preflight válidos, admissão iniciada, observação recente de stock/receipts e assinatura estabelecida com conexão/canal source e publisher quarantine utilizáveis. Assinatura ociosa pode estar pronta. Fim/cancelamento do stream/reconexão limpa prontidão AMQP. |

Falhas iniciais podem expor starting brevemente e depois sair; a política existente de falha/restart não foi alterada. Preflight/observações verificam colunas e SELECT; não provam todas as permissões de escrita, funções/constraints, estabilidade concorrente do schema ou roteamento futuro. Validação de entrega permanece autoritativa. Não há transação de negócio para testar saúde.

Estados: `starting` antes da admissão; `ready` com dependências observadas com sucesso e atuais; `not_ready` com dependência desconhecida/falha/obsoleta; `stopping` ao reconhecer sinal, antes de drenar. Stopping é definitivo para a vida do processo: sucesso tardio não o reverte. É estado informativo; não cria outro gate de admissão nem muda retry, entrega, deduplicação ou cancelamento.

Vocabulário finito e precedência: `stopping`, `starting`, depois banco (`database_unknown`, `database_stale`, `database_unavailable`), broker (`broker_unknown`, `broker_stale`, `broker_unavailable`), senão `ready`. JSON contém `process`, `state`, um `reason`, `freshness_seconds`, `observation_age_seconds` monotônico e timestamps Unix de último sucesso (null quando desconhecidos/não aplicáveis). `Cache-Control: no-store`. Sem URL, segredo, payload, fila, identidade ou cadeia de erro. Primeiro motivo vence; métricas mostram atualidade individual das dependências. Scrape e `/ready` não consultam banco nem abrem conexões.

## Orçamentos, atualidade e ausência

Sampler inicia imediatamente após preflight e espera **5 segundos após concluir** cada amostra, sem sobreposição/catch-up. Pool lazy exclusivo, máximo **1 conexão adicional por papel dependente de banco**; aquisição e amostra completa têm deadline cliente **2 s**, statement_timeout **1000 ms**, lock_timeout **500 ms** e transações default read-only. Executa as mesmas colunas do preflight com `LIMIT 0` (duas consultas no consumer). Não conta/materializa backlog. Planejamento, catálogos e locks têm custo; timeout não prova custo constante. Cancelamento pode deixar trabalho no servidor até seu próprio deadline. Shutdown cancela/recolhe task (1 s) e limita pool.close (1 s); Drop aborta a task. Deadline existente de cleanup do worker permanece.

Orçamento total por processo inclui pool de negócio **mais 1 do sampler**: default relay=3, consumer=3; HTTP=0 sem replay ou 3 com replay. Não é quota do cluster. Sem novas variáveis: intervalo/deadline/validade são constantes explícitas. Mesmas credenciais de banco, sem novas credenciais/polling management RabbitMQ.

AMQP usa resultados reais de ciclo de vida e status em memória do cliente. Relay atualiza antes de entregar e informa false ao conectar/substituir. Consumer atualiza após assinatura e a cada segundo enquanto espera entregas; limpa antes de reconectar/fechar. Tick não refaz assinatura nem altera crédito. Operação longa pode atrasar observação e produzir stale conservador. Transporte conectado pode falhar imediatamente depois; half-open depende de heartbeat/detecção do cliente. Não é publicação de teste nem garantia ponta a ponta.

Idade usa relógio monotônico Tokio: **mais de 15 s é stale**, mesmo com sucesso anterior. Timestamps Unix são diagnósticos, sem decidir validade. Falha atualiza idade/tentativas e contador, preserva timestamp do último sucesso e não restaura saúde. Métricas omitem timestamp desconhecido; JSON usa null. Sampler que para deixa observação stale posteriormente.

| Observação | Significado |
| --- | --- |
| Endpoint acessível, 200 | Evidência recente de admissão deste processo. |
| Endpoint acessível, 503 | Inspecionar motivo finito, atualidade e logs. |
| Exporter acessível, motivo stale | Listener funciona; observação requerida não é atual. |
| Endpoint ausente/inacessível | Não existe relato: usar scrape `up` externo, namespace, Supervisor e logs. Ausência sozinha não distingue rede/exporter/processo encerrado. |
| Processo intencionalmente parado | Endpoint não é esperado; intenção e estado nominal são externos. Não fabricar 503 nem saúde do worker no HTTP. |

Sem collector, Prometheus/Grafana ou frontend live. Processo encerrado não responde sobre si; `up` vem de scraper externo. Estado/contadores reiniciam com processo.

## Métricas e backlog adiado

Contrato 5.1 preservado. Labels novos limitados: `process` (três), `dependency` (`database`, `broker`), `reason` (nove).

| Métrica | Significado |
| --- | --- |
| `relay_readiness_pool_connections_limit` | Máximo do pool de observação (0 ou 1), adicional ao gauge do pool de negócio. |
| `relay_ready` | 1 apenas no snapshot ready, senão 0. |
| `relay_readiness_reason{reason}` | Conjunto fixo de motivos, somente um com valor 1. |
| `relay_dependency_observed{dependency}` | Observação concluída existe; só dependências requeridas. |
| `relay_dependency_fresh{dependency}` | Idade ≤15 s; falha recente ainda é unready. |
| `relay_dependency_failures_total{dependency}` | Contador saturante de observações falsas/falhas; broker não conta incidentes únicos. |
| `relay_dependency_last_success_timestamp_seconds{dependency}` | Unix segundos, omitido antes do sucesso. |
| `relay_dependency_observation_age_seconds{dependency}` | Idade monotônica da última tentativa concluída, omitida antes da observação. |
| `relay_dependency_observation_duration_seconds{dependency}` | Duração da última amostra; status AMQP em memória usa zero. Não é histograma. |

Snapshot de prontidão coerente sob mutex curto; demais métricas 5.1 continuam independentes. Exporter mantém quatro clientes/1024 bytes/2 s; sem labels de erro dinâmicos, filas de telemetria ou efeitos de negócio.

**Sampler de backlog adiado.** Sem count/gauge de idade ou zero fictício. Antes de implementar: avaliar planos/índices e tamanho realista; definir pending com lease vivo / sem lease e atrasado / sem lease e devido, separar inválidos/exauridos e dead_letter; cutoff PostgreSQL em snapshot de uma instrução, idade do candidato válido elegível mais antigo, pool/statement/custo de scan, cancelamento e unavailable/stale. Lease tem precedência sobre devido/atrasado para evitar dupla contagem. Probes de schema não medem backlog. Não existe snapshot atômico PostgreSQL+RabbitMQ nem polling duplicando métricas do broker.

## Inspeção, validação e próximo deploy

Defaults: `HTTP_METRICS_ADDR=127.0.0.1:9100`, `RELAY_METRICS_ADDR=127.0.0.1:9101`, `CONSUMER_METRICS_ADDR=127.0.0.1:9102`; caminhos `/metrics` e `/ready`. Parser exige loopback/porta não zero, portas distintas entre si e do HTTP; colisão no bind falha startup. Compose não publica essas portas; localhost do host é outro namespace.

```sh
docker compose exec -T app supervisorctl status
docker compose exec -T app python3 -c 'import urllib.request; print(urllib.request.urlopen("http://127.0.0.1:9101/metrics", timeout=3).read().decode())'
# /ready devolve HTTPError no urllib para 503: inspecionar status/body.
# Runtime compartilhado ainda é 5.1; readiness depende do deploy deliberado do 5.2.
docker compose exec -T -e CARGO_TARGET_DIR=/app/target/m52 app cargo test --locked
```

Artefatos 5.2 em `/app/target/m52`, preservando executáveis compartilhados `/app/target/debug`. Fonte/testes locais não equivalem a validação remota nem deploy. Antes de implantar 5.2: revisar diff, commit/push pelo usuário, CI do SHA exato, revisar conexão adicional por sampler, compilar revisão escolhida, capturar processos/dados/filas e reiniciar apenas programas afetados. Preservar intenção do consumer, replay e recibo. Sem migration. Recriar Supervisor pode autoiniciar consumer.

Checks executados/limites: [resultados](../testing/validation-results.md). Testes cobrem transições determinísticas, stale/shutdown definitivo/dependência opcional, endpoints HTTP/relay/consumer reais, falha/recuperação por lock/deadline PostgreSQL isolado e perda/recuperação do transporte da assinatura em topologia descartável sem publicação. Regressões existentes de entrega/consumer continuam necessárias.

**Pré-requisitos/escopo 5.3:** revisar/validar remotamente 5.2, implantar revisão deliberada e verificar exporters reais; preservar demo/intenção do consumer. Definir workloads/payloads isolados reproduzíveis, orçamentos de recursos/medição, warmup, protocolo de backpressure/recuperação e limpeza exata antes de medir capacidade. Sem benchmark, concorrência extra, frontend live, endpoint de negócio, replay/purge quarantine, HA/TLS ou expansão de identidade neste marco.

Fontes primárias: [separação de probes](https://kubernetes.io/docs/concepts/workloads/pods/probes/), [lifecycle consumer RabbitMQ](https://www.rabbitmq.com/docs/consumers), [deadlines PostgreSQL](https://www.postgresql.org/docs/18/runtime-config-client.html). Referências orientam semântica; testes provam apenas os cenários exercitados.
