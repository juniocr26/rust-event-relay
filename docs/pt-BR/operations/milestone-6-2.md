[Registro histórico: instalação e endereços atuais](../operations/application-scenarios.md)

[English](../../en/operations/milestone-6-2.md) | [Catálogo](../index.md)

# Marco 6.2 — entrega duplicada com evidência durável

Utilize a aplicação principal em http://localhost:5173, com os serviços existentes PostgreSQL, RabbitMQ, frontend e HTTP/relay/consumer supervisionados. O worker de ações, publisher, sessões temporárias do consumidor e tabelas de evidência da consolidação são reutilizados. A migração nova permite três ações por execução, limitadas a 100 ações retidas globalmente, preservando checksums anteriores e registros existentes.

Selecione Duplicate delivery, abra o modal, informe DEMO_TOKEN e carregue as execuções processadas. Escolha uma execução e execute. Também é possível criar um exemplo novo de 10 + 5. Cada nova intenção deliberada recebe UUID durável; repetir a requisição reconcilia a mesma ação. Fechar interrompe apenas o polling. A seleção e identidade permanecem no sessionStorage ao reabrir/recarregar. Request another duplicate cria explicitamente outra ação; recuperar uma ação unknown não republica.

## Estados e significado da evidência

A admissão usa o advisory lock transacional existente. Repetição idêntica de request/run/copies retorna a ação armazenada mesmo na capacidade; reutilização conflitante retorna 409. Exige publicação original concluída e receipt de inventário confirmado. Não recria fato de negócio, estoque inicial, aquisição do outbox ou receipt.

O commit admite queued; um worker persiste executing antes de acessar o broker. Reinício retoma queued e transforma executing interrompido em unknown, inclusive quando a interrupção ocorreu antes do envio. Não há reenvio automático. confirmed exige sucesso das chamadas limitadas de publicação e processamento Duplicate/retorno do envio de ACK; os registros de publicação são evidência independente. failed permanece reservado no contrato herdado; falhas atuais viram unknown conservadoramente. SQL e AMQP não compartilham commit atômico.

A fila/routing key durável exclusiva `relay.demo.<UUID da ação>` no vhost/exchange existente fornece correlação persistente, sem alterar envelope, message_id ou timestamps originais. Suas sessões temporárias recebem o contexto da ação; o repositório também valida associação ação/execução. Validação de conteúdo e deduplicação continuam obrigatórias. Não há metadados novos de transporte. A correlação pressupõe o principal local confiável do broker; não prova origem criptográfica.

O receipt original comprova um efeito confirmado, 10→15. Uma observação Duplicate separada comprova comparação do conteúdo imutável e commit dessa classificação sem nova alteração de estoque. Receipt histórico não implica observação Applied. Quantidade de receipts e estoque atual são consultados no mesmo snapshot de comando. Confirmação de publicação não prova conclusão do consumidor. A observação confirma antes do ACK; ACK incerto pode gerar redelivery e outra observação limitada, mantendo um efeito. Contagens são limites inferiores, não contagens únicas de entregas. Confirmação perdida pode deixar publicação unknown mesmo com mensagem aceita. Não se promete exatamente duas entregas nem recebimento exato do ACK pelo broker.

## Limites, operação e rollback

Permanecem autenticação bearer local, verificação de Origin do navegador, corpo de 1 KiB, dez requisições/segundo, dois handlers HTTP, timeout HTTP de oito segundos, quatro ações queued/executing, um worker em execução, duas sessões/conexões de consumidor da ação, prazo de vinte segundos e uma a três chamadas de publicação. São permitidas três ações retidas por execução e 100 globalmente. Publicações e observações têm teto de 32 registros por execução; saturação torna contagens limites inferiores. Não há purge, TTL ou exclusão de receipts. Capacidade esgotada rejeita trabalho novo sem apagar evidência. Polling sequencial a cada segundo por até sessenta segundos, com timeout de requisição de até nove segundos. Pending, unknown, unavailable e stale permanecem distintos.

SQL somente leitura está disponível em Where to verify e na [versão inglesa](../../en/operations/milestone-6-2.md). Consulte demo.actions, demo.publications, demo.consumer_observations pelo UUID da ação; consumer.receipts pelo Event ID e consumer.stock pela chave da execução. RabbitMQ management em http://localhost:15672 mostra a fila retida; estatísticas agregadas não comprovam Duplicate. Ações unknown conservam evidência SQL e mensagens para inspeção; recuperação expandida fica para 6.3.

```sh
python3 scripts/start-local.py
python3 -B scripts/check-migrations.py
python3 -B scripts/consolidate-storage.py --apply
docker compose exec -T app cargo build --locked --bins
docker compose exec -T app supervisorctl restart http
python3 -B scripts/test-application.py
```

Backup protegido e restauração descartável são verificados antes de migração pendente no banco principal. O novo rollback restaura a restrição de uma ação por execução e recusa quando existem múltiplas ações; nunca apaga evidência para forçá-lo. Rollbacks anteriores de actions/runs são destrutivos e não devem ser aplicados aos dados retidos. O harness de navegador usa cópia descartável do Compose principal, pipeline real sem mocks e remove apenas os recursos exatos de teste.

Fontes: [RabbitMQ confirms/ACK](https://www.rabbitmq.com/docs/confirms), [PostgreSQL Read Committed](https://www.postgresql.org/docs/current/transaction-iso.html).

## Validação local — 17/10/2026

Executados: 27 testes de biblioteca; 11 testes PostgreSQL demo isolados; 52 regressões existentes de consumidor/aplicação/broker; um teste adicional de observação correlacionada com ACK real descartado; 25 verificações de navegador no harness descartável do Compose principal. Campanha final aprovada, incluindo seleção histórica, múltiplas ações, fechar/reabrir, token após reload, cinco larguras, foco e movimento reduzido. Capturas mobile e de ação histórica foram inspecionadas. Clippy estrito, build/rustdoc/fmt, build/tipos Vue, nove upgrades/checksums/no-op/down/re-up, quatro testes do verificador documental, links de 121 Markdown e Compose passaram. A primeira campanha revelou uma expectativa antiga sobre o token; corrigida antes da campanha final. CI remoto não executado.

Backup/restauração: `.local-backups/consolidation-20261017T173714Z`, privado e não rastreado. Snapshot anterior de outbox/receipts/estoque idêntico após migração. App/frontend ainda utilizavam variáveis anteriores à consolidação; apenas esses dois containers foram recriados para ativar a configuração existente. Infraestrutura e dados preservados.

Execução principal `01a14af3-4765-747a-ac4d-c471ffde7fbe`, evento `01a14af3-4765-747a-ac4d-c47252bdb61b`, ação `01a14af3-4b6f-7090-9215-4572e226394b`: SQL independente confirmou estoque 15, um receipt de efeito e uma observação Duplicate correlacionada. Nove migrações válidas, uma execução/ação demo retida; dois receipts totais incluindo o original preexistente. Filas principal/ação/quarentena sem mensagens ready/unacknowledged; consumidor principal ativo. Recursos descartáveis removidos. Sem stage, commit ou push.

O snapshot do navegador passa a stale após quinze segundos sem atualização bem-sucedida; seu timestamp de observação permanece visível.

A suíte padrão completa `cargo test --locked` também passou; os testes de integração opt-in executados estão identificados separadamente acima.

Uma campanha repetida revelou um deadlock herdado de reserva na sessão concorrente: após seu primeiro ACK, slot-2 podia reservar a terceira cópia enquanto slot-1 ainda aguardava. Slot-2 agora fecha imediatamente após seu processamento atribuído, liberando cópias prefetched não confirmadas para redelivery pelo broker. Não há publicação extra e a deduplicação permanece. A campanha acrescenta três ações concorrentes reais para verificar essa fronteira. A checagem de freshness observa o prazo real de quinze segundos, sem mock de resposta do pipeline.
