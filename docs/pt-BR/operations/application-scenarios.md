# Startup e verificação de token


O startup suportado no host é `python3 scripts/start-local.py`. Compila artefatos com `--locked` em `target/runtime`, verifica/faz backup do armazenamento antes de migrações pendentes e inicia Supervisor após os pré-requisitos. Ao iniciar o gerenciador, o entrypoint prepara binários automaticamente; `docker compose up` direto não migra dados retidos. Use o startup suportado em instalações novas. Schema ausente é falha da aplicação ao iniciar, não necessariamente falha de execução do launcher. Consulte `docker compose logs --tail 60 app` e `docker compose exec app supervisorctl status`; não repita restarts nem resete volumes. Paradas intencionais STOPPED continuam respeitadas no mesmo gerenciador. Testes compilam em `target/tests`, sem substituir binários dos serviços.

Dentro de app: `python3 /app/scripts/demo-token.py rotate`. A rotação primeiro salva o token autoritativo, reinicia apenas HTTP em execução para carregá-lo, inicia uma vez programas FATAL/EXITED e preserva STOPPED intencional. Verifica acesso com token novo e rejeição do anterior, estabilidade dos processos, readiness dos processos em execução e topologia durável RabbitMQ via consultas de gerenciamento somente leitura. Não publica eventos nem purga filas. HTTP intencionalmente STOPPED mantém ativação pendente e retorna erro. Se falhar após ativação, informa explicitamente que a rotação já foi aplicada; consulte o token privadamente em `.env` e corrija a falha indicada antes de tentar novamente. Estado de processos Supervisor e topologia de filas são verificações distintas.

No incidente de startup reproduzido, os launchers existiam, eram executáveis e iniciavam os binários. Os binários então encerravam: o banco configurado não possuía tabelas da aplicação nem histórico SQLx. Os logs indicavam schema de ownership ausente, schema de consumer ausente e demo forward migration required. Após backup/prova de restauração descartável e migrações autorizadas explicitamente, o startup suportado recuperou os três processos. Espera por diretório de artefatos e comandos interrompidos manualmente não comprovam defeito da aplicação.

---

[Token e conectividade](demo-token-connectivity.md)

[English](../../en/operations/application-scenarios.md) | [Catálogo](../index.md)

# Guia da aplicação

Comece pelo [README](../../../README.pt-BR.md). Há um ambiente Compose principal com app, frontend, PostgreSQL e RabbitMQ. `compose.demo.yaml` e os scripts dedicados são artefatos históricos; não são o fluxo de inicialização atual. Nenhum repositório externo ou Engineering Library é necessário.

## Componentes e ciclo de vida

`backend/src/domain` valida envelope canônico e identidades de inventário. `application` define contratos, decisões de ownership/retry/replay e ciclos limitados dos workers. `infrastructure/postgres` implementa transações curtas e inspeção por snapshot; `infrastructure/rabbitmq` controla confirms, ACK manual e quarentena. `infrastructure/demo_delivery.rs` executa sessões limitadas de ações. `demo_http.rs` expõe intents fixos autenticados e evidências. Componentes Vue reutilizáveis ficam em `frontend/src/components`; `useDemoRun.ts` separa execução de inspeção. `backend/migrations` contém o histórico SQLx; `docker/supervisor` gerencia três binários independentes; `scripts` reúne inicialização e verificações descartáveis.

1. O produtor confirma atomicamente recebimento de mercadoria, estoque inicial e evento outbox canônico. UUID repetido com mesmo intent retorna evidências existentes; conteúdo diferente conflita.
2. O relay adquire autoridade com lease, publica e observa aceitação do broker por confirm.
3. O relay persiste conclusão com ownership/revisão ainda válidos.
4. O consumidor confirma efeito de estoque e recibo atomicamente, ou compara conteúdo imutável e registra Duplicate sem novo efeito.
5. Após persistir, tenta enviar ACK. O retorno local não comprova recebimento pelo broker.

São responsabilidades, não ordem visual estrita: evidência do consumidor pode aparecer antes da conclusão observada do relay. Outbox processado não prova conclusão do consumidor. Publisher confirm e consumer ACK são protocolos distintos. COMMIT SQL e AMQP não são atômicos; desconhecido não significa rollback. Snapshot SQL negativo não exclui commit posterior de transação em andamento.

## Configuração e instalação

Substitua POSTGRES_USER/PASSWORD e RABBITMQ_DEFAULT_USER/PASS no `.env` ignorado. Credenciais novas são para instalação nova: variáveis de inicialização não alteram credenciais de volumes existentes. DEMO_TOKEN vazio é gerado uma vez; exige 64 caracteres hexadecimais. Digite-o em tempo de execução; o navegador mantém o segredo apenas em memória e exige nova entrada após reload. sessionStorage guarda apenas identidades de pedidos/ações/seleções. Nunca use VITE_* para segredos.

Os demais valores de `.env.example` têm defaults explícitos. HOST_HTTP_PORT, HOST_FRONTEND_PORT, POSTGRES_HOST_PORT e RABBITMQ_MANAGEMENT_PORT alteram publicações loopback do host; mantenha portas internas 8080/5173/5432 coerentes com Compose. LOCAL_UID/GID selecionam o usuário Linux. REPLAY_OPERATORS vazio desativa replay privilegiado; RELAY_MAINTENANCE_CANDIDATES vazio desativa candidatos explícitos, preservando descoberta normal limitada. Exporters são apenas loopback interno. Tempos e pools são limites finitos. Os cenários locais pressupõem consumidor de inventário padrão e intent 10+5; topologia personalizada exige configurações correspondentes. Placeholders e token inválido são rejeitados sem revelar valores. Porta ocupada exige liberar a porta configurada.

`python3 scripts/start-local.py` inicia infraestrutura, compila binários locked, verifica checksums, cria backup privado com restauração de prova antes de migração necessária e inicia aplicação. Migrações SQLx precedem os workers. Verificação sem alterações preserva dados. Rollbacks antigos podem excluir evidências: execute apenas no harness descartável. A 6.3 não reescreve migrações aplicadas.

## Cenários e inspeção

- Entrega: novo intent fixo, estoque 10 → 15, outbox processado e um recibo.
- Duplicata: crie fixture ou escolha execução bem-sucedida. Uma ação republica envelope original idêntico; confirme observação Duplicate correlacionada, estoque 15 e um recibo. “Request another duplicate” admite trabalho novo explicitamente com outro UUID.
- Duplicatas concorrentes: três chamadas limitadas de publicação e duas sessões temporárias; classificações preservam um efeito. Fechar cada sessão concluída libera cópias prefetched ainda não confirmadas para redelivery. Ordem e quantidade de chegadas não são garantias de exatamente uma vez.
- Recuperação controlada: rejeição durável uma vez antes do envio AMQP, agendamento/revisão de retry registrados e entrega posterior. Não é queda de infraestrutura nem NACK do broker.
- Playback de crash, race e perda de ACK: apenas ilustrativo. Falhas reais são verificadas em testes isolados; não há controles de shutdown global, SQL/shell arbitrário ou Docker socket na interface.

Em “Inspect retained runs and actions”, autentique, carregue uma página de dez execuções, selecione e inspecione inclusive pendentes. `/demo/history?limit=10&offset=0` aceita 1–20 linhas e offsets 0–100, ordenados por data de aceite/UUID. `/demo/runs` mantém seleção das execuções bem-sucedidas. Selecione ação histórica para suas publicações/classificações. Reabrir/recarregar não envia trabalho: inspeção usa GET. Ação desconhecida nunca é republicada automaticamente. Ausência de pedido confirmado exige nova inspeção; “Submit same producer intent” é envio explícito idempotente, não prova de rollback anterior.

Loading, pending, unavailable, terminal, unknown e stale têm explicações distintas. Cada snapshot exibe observed_at e envelhece após 15 segundos. Consulta indisponível preserva fatos anteriores, sem tratá-los como atuais. Polling sequencial dura no máximo 60 segundos, com até nove segundos por requisição. Fechar o modal interrompe observação, não trabalho confirmado.

## Onde verificar

Conecte em localhost:5433 com POSTGRES_DB/USER/PASSWORD privados, ou evite senha em argumentos:

```sh
docker compose exec -T postgres sh -c 'PGPASSWORD="$POSTGRES_PASSWORD" psql -X -h postgres -U "$POSTGRES_USER" -d "$POSTGRES_DB"'
```

Copie IDs e SQL filtrado no modal. Substitua placeholders somente pelas identidades exibidas:

```sql
BEGIN READ ONLY;
SELECT * FROM demo.runs WHERE run_id = '<run UUID>';
SELECT * FROM demo.goods_receipts WHERE run_id = '<run UUID>';
SELECT id,status,attempt_count,available_at,processed_at FROM relay.outbox_events WHERE id = '<event UUID>';
SELECT * FROM demo.actions WHERE run_id = '<run UUID>';
SELECT * FROM demo.actions WHERE request_id = '<action request UUID>';
SELECT * FROM demo.publications WHERE action_id = '<action UUID>';
SELECT * FROM demo.consumer_observations WHERE run_id = '<run UUID>';
SELECT * FROM demo.faults WHERE run_id = '<run UUID>';
SELECT consumer_name,event_id,stock_before,quantity,stock_after,completed_at FROM consumer.receipts WHERE event_id = '<event UUID>';
SELECT units FROM consumer.stock WHERE warehouse_id = '<warehouse>' AND sku = '<SKU>';
COMMIT;
```

`demo` contém runs, goods_receipts, actions, publications, consumer_observations e faults. `relay.outbox_events` contém publicação, leases e retry; `relay.replay_requests` registra replay autorizado. `consumer.stock` e `consumer.receipts` guardam efeito e deduplicação por conteúdo imutável. `public._sqlx_migrations` guarda versões/checksums/sucesso. Goods receipt prova fato do produtor; processed_at prova conclusão do relay; recibo 10+5=15 prova efeito; observação Duplicate correlacionada prova classificação sem novo efeito. Contagens saturadas são limites inferiores.

RabbitMQ: http://localhost:15672, credenciais privadas, vhost `relay`, exchange direct durável `relay.events`, routing key `event`, fila `relay.local`. Cada ação retida usa `relay.demo.<action UUID>` como fila e routing key. Quarentena: exchange `relay.quarantine`, key `inventory`, fila `relay.quarantine.inventory`. O modal exibe topologia configurada da origem. Gráficos são agregados: consumo rápido pode ocorrer entre amostras. `dead_letter` SQL é estado terminal do relay, distinto de quarentena RabbitMQ. Entrada rejeitada deve ser preservada por quarentena confirmada antes de ACK da origem.

## Operação, capacidade e recuperação

`/health` prova apenas liveness HTTP. Dentro de app, `/ready` e `/metrics` em 9100/9101/9102 representam dependências e freshness de HTTP/relay/consumer, sem publicação no host. Consulte com Python urllib no container. Consumidor parado pode deixar outbox processado e efeito pendente: inspecione ambos. Use `supervisorctl status` e controles por nome. Parada intencional é respeitada pelo mesmo gerenciador; recriação retoma autostart. Diferencie código, binário no disco e processo efetivamente carregado.

Limites: 100 runs retidos, 100 ações totais, três ações/run, quatro runs/ações ativos, um worker de ação, duas sessões, 1–3 chamadas de publicação, 20 segundos por ação, 32 publicações e 32 observações de consumidor/run. Filas duráveis permanecem inclusive em resultado desconhecido. Capacidade esgotada rejeita admissão sem apagar histórico. Não há retenção automática ou endpoint genérico de limpeza. Suspenda admissão e exporte evidências antes de planejar manutenção manual; não exclua recibos enquanto redelivery for possível. Não há reparo geral nem reenvio de ação desconhecida.

PostgreSQL persiste em `.dockerized-postgres/`; RabbitMQ em volume nomeado. `docker compose stop` preserva ambos. `.local-backups` é privado e ignorado. Antes de migração necessária, `consolidate-storage.py --apply` cria backup, comprova restauração descartável e aplica somente forward; se atual, verifica versões/checksums. Restrinja acesso a backups. Prove restauração em banco novo primeiro; nunca sobrescreva automaticamente o principal. pg_dump não inclui filas RabbitMQ; backup/restauração independente do broker e disaster recovery de produção estão fora desta demonstração local.

## Roteiro de estudo

Siga um Event ID exibido: `application/demo.rs` e transação em `postgres/demo.rs`; consulte goods receipt e outbox; acompanhe `application/worker.rs`, `delivery.rs` e adapters de ownership/retry; veja confirms em `rabbitmq.rs`; percorra `application/consume.rs`, adapter PostgreSQL e ACK/quarentena; compare recibo com estoque. Depois siga Action ID por `demo_delivery.rs` e tabelas correlacionadas. ADRs 007–010/012 explicam decisões e incerteza; testes demonstram limites de falha. Compare fatos persistidos antes de interpretar o estado visual.

Referências consultadas: [confirms e ACKs RabbitMQ](https://www.rabbitmq.com/docs/confirms), [snapshots Read Committed PostgreSQL](https://www.postgresql.org/docs/current/transaction-iso.html). Explicam semântica de protocolo/banco; testes da aplicação comprovam o comportamento implementado.
