[English](../../en/architecture/milestone-4-3.md) | [Catálogo](../index.md)

# Marco 4.3 — Consumidor RabbitMQ e quarentena confirmada

Implementado em 16/10/2026 na base Rust `652bbd21e1cac5dbd4fe587fcf03dcd73a906625`. Preservadas as correções ainda sem commit de diagnósticos/testes/documentação da 4.2. A Library, base `a25591eaf04a7f95b1769b7dbc9d6b7b403945a5`, também já tinha dois documentos alterados. A [4.2](milestone-4-2.md) fornece o adapter atômico real; esta etapa acrescenta orquestração, preservação bruta, ciclo AMQP explícito e o terceiro binário. Não encerra o Marco 4 nem ativa consumo compartilhado.

## Fronteiras e fluxo concreto

[consume::handle_delivery](../../../backend/src/application/consume.rs) valida uma entrega limitada e coordena as portas existentes com IncomingDelivery. Domínio/aplicação continuam independentes de Lapin e SQLx. [PostgresInventoryProcessor](../../../backend/src/infrastructure/postgres/consumer.rs) mantém a transação curta READ COMMITTED; não há transação SQL aberta durante confirm de quarentena ou ACK. [Adapters/runtime AMQP](../../../backend/src/infrastructure/rabbitmq/consumer.rs) possuem objetos de protocolo e o acker original. O [binário consumer](../../../backend/src/bin/consumer.rs) compõe configuração, pool, sinais e preflight de schema; nunca aplica migrations.

| Etapa | consumer.stock(local, SKU-1) | Recibo / ação no broker |
| --- | ---: | --- |
| Fixture inicial | 10 | Sem recibo |
| Receber evento A, quantidade 5 | 10 | Origem continua sem ACK |
| Commit atômico confirmado | 15 | A registra antes 10 / depois 15 / timestamp de conclusão |
| ACK individual da origem | 15 | Broker pode retirar o original |
| A idêntico entregue novamente | 15 | Comparação durável retorna Duplicate(15) histórico, depois ACK |
| A conflitante | 15 | Preservar bytes conflitantes, confirmar roteamento, depois ACK |
| JSON inválido / evento não suportado | 15 | Sem chamada ao estoque; preservar bytes, confirmar, depois ACK |

`relay.outbox_events.processed` representa conclusão de publicação. `consumer.receipts` registra efeito de negócio local. ACK retira uma entrega e não é recibo de negócio. SQL `dead_letter` preserva falhas do relay; quarentena RabbitMQ preserva mensagens inválidas/conflitantes do consumidor. Sucesso de quarentena significa preservação, não processamento de estoque. Replay/purge de quarentena, endpoints produtores, efeitos externos e integração frontend continuam ausentes.

## Topologia de origem e quarentena

Inspeção compartilhada somente leitura encontrou `relay.local` classic durável, `x-queue-type=classic`, sem auto-delete, zero consumidores, binding `relay.events` → `relay.local` com chave `event` e sem política efetiva DLX/TTL. Nenhum consumo ou alteração foi feito. Declaração da origem exige classic explicitamente; incompatibilidade de tipo/durabilidade/exchange falha com diagnóstico sanitizado e orientação operacional. Não há fallback de remoção/recriação.

| Configuração | Padrão |
| --- | --- |
| CONSUMER_SOURCE_EXCHANGE | relay.events (direct durável) |
| CONSUMER_SOURCE_QUEUE | relay.local (classic durável, não exclusiva, sem auto-delete) |
| CONSUMER_SOURCE_ROUTING_KEY | event |
| CONSUMER_QUARANTINE_EXCHANGE | relay.quarantine (direct durável) |
| CONSUMER_QUARANTINE_QUEUE | relay.quarantine.inventory (durável, não exclusiva, sem auto-delete) |
| CONSUMER_QUARANTINE_ROUTING_KEY | inventory |
| CONSUMER_NAME | inventory.stock.v1 |

Exchanges/filas de origem e quarentena devem ser distintos e seguir relay.*; nomes/chaves cabem em short strings AMQP. A origem tem configuração explícita independente de RABBITMQ_EXCHANGE/QUEUE/ROUTING_KEY do publisher relay. Réplicas da mesma responsabilidade usam CONSUMER_NAME estável; outro nome tem deduplicação independente e pode aplicar outro efeito no mesmo estoque. Consumer tag identifica apenas a assinatura.

Não são configurados DLX, TTL, expiração de fila, purge ou replay. A declaração estabelece o binding exigido, mas mandatory/confirm não identifica nominalmente a fila de destino. O deployment deve restringir autoridade de configuração, impedir deriva de topologia/políticas e garantir o destino durável esperado sem rotas extras/ausentes. Políticas efetivas podem sobrepor premissas e não são inspecionáveis por basic.queue.declare: antes do rollout, conferir políticas/operator policies, bindings e permissões, incluindo ausência de descarte silencioso/drop-head/TTL/DLX. Não combinar DLX do broker e preservação da aplicação neste caminho. Durabilidade declarada não é backup nem teste de queda de energia.

## Formato de preservação v1 e limites

[QuarantineMessage](../../../backend/src/application/consumer.rs) carrega corpo original exato até 1 MiB, message_id original inclusive ausente/malformado até 255 bytes e sidecar JSON até 16 KiB incluindo metadados. O corpo publicado não muda; propriedades novas usam delivery_mode=2, content_type=application/octet-stream, type=relay.quarantine.v1 e um header `relay-preservation-v1` com JSON UTF-8. Expiração, delivery mode, user_id e demais propriedades originais são evidência; não são reaplicados como TTL/autoridade da quarentena.

Sidecar: format_version=1, consumidor lógico, source_exchange/source_routing_key reais, observed_at UTC, reason de enum fechado e original_properties. Timestamp é observação do cliente, não prova de commit. Motivos distinguem MalformedJson, InvalidContent, InvalidIdentity, Unsupported, IdentityConflict e StockOverflow. Envelope estruturalmente inválido é InvalidContent; sintaxe JSON inválida é MalformedJson. Processamento exige UUID canônico não nulo e message_id parseável correspondente. Não gerar identidade substituta para entrada inválida.

[Encoder/decoder verificado de propriedades](../../../backend/src/infrastructure/rabbitmq/properties.rs) preserva todos os campos BasicProperties e tipos de headers expostos pelo Lapin 4.12.0 instalado. Valores mantêm tags de enum, arrays/tabelas aninhados, larguras signed/unsigned, escala/valor decimal e timestamp. LongString/ByteArray usam base64; FloatBits/DoubleBits preservam bits IEEE, inclusive NaN, infinitos e zero negativo. Serialização JSON comum de float perderia distinções. Travessia de headers limitada a 512 nós cobrados e profundidade 16, com orçamento antecipado de bytes e conferência final do tamanho. Decode verifica limites e recodificação canônica; não autoriza replay. Lapin já converte aliases wire no modelo e materializa corpos/propriedades: isto não captura frames AMQP originais, chaves duplicadas de tabela ou distinções apagadas pelo cliente.

Headers originais, inclusive `relay-preservation-v1` forjado, ficam aninhados em original_properties e nunca são mesclados com metadados internos. Antes de confiar em um registro, autentique origem/permissões da quarentena: produtor autorizado a escrever ali poderia forjar o registro inteiro. Limites são conferidos antes de parse JSON/cópia do corpo; serde também limita recursão. O corpo já alocado e os buffers Lapin impedem tratar 1 MiB como teto rígido de memória do processo. Admissão/max-message-size do broker, frame size, prefetch positivo e capacidade são pré-requisitos operacionais.

Preservação exata fora dos limites nunca é truncada. O runtime cancela/fecha e sai normalmente com log de erro para recuperação operacional, deixando a origem recuperável; Supervisor não reinicia repetidamente exit 0 após RUNNING. Inspecionar origem retida, estabelecer protocolo externo explícito sem perda ou mudança suportada de limite e reiniciar explicitamente. Nenhum descarte automático foi autorizado.

## Matriz de decisão e incerteza

| Resultado primário | Preservação | Ação na origem / recuperação |
| --- | --- | --- |
| Applied após COMMIT confirmado | nenhuma | ACK individual |
| Duplicate idêntico verificado | nenhuma | ACK individual; sem novo efeito |
| IdentityConflict | publicação persistente mandatory, confirm sem return | ACK somente após preservação |
| Invalid / Unsupported / overflow | mesma preservação | ACK somente após preservação |
| Rejeição DB conhecida / Unavailable / InvalidStoredData | nenhuma | Sem ACK; fechar sessão e reconectar com pacing |
| CommitUncertain ou timeout da operação DB inteira | nenhuma | Sem ACK/quarentena; comparação durável posterior |
| Nack / return / indisponibilidade / timeout / desconexão na quarentena | falha ou incerteza | Sem ACK; aposentar publisher incerto, fechar e aguardar |
| Falha/timeout de ACK / acker inutilizável | manter resultado primário e evidência de preservação | Fechar e aguardar; redelivery possível |
| Cancelamento do consumidor / perda de conexão | sem resultado fabricado | Fechar/reconectar/reassinar com pacing |
| Preservação fora dos limites | bloqueada | Manter origem; parar para recuperação operacional |

HandlingOutcome mantém processamento/validação primários, preservação e ACK separadamente. Logs sanitizados não incluem corpo, credenciais, propriedades ou cadeias de causas. Timeout da operação DB inteira pode disputar com COMMIT e vira CommitUncertain conservadoramente. Sem retries no adapter ou quarentena cega por falha DB. Redelivery idêntico encontra recibo histórico ou aplica com segurança após rollback. Observação negativa não exclui commit em voo.

Publisher de quarentena reutiliza apenas o transporte bruto confirmado extraído de RabbitMqPublisher; não usa o contrato EventPublisher de envelope válido. Um gate limita publicação: Ack(None) sucede, Ack(return) é Unroutable, Nack é Rejected e confirmação perdida/ausente é Uncertain. Cancelamento após envio aposenta o publisher. ACK da origem usa BasicAckOptions{multiple:false} e acker original; Lapin 4.12.0 retorna false para acker usado/inutilizado, tratado como falha. Conclusão local do ACK não confirma recebimento no broker. Crash após confirm de preservação antes do ACK, ou ACK incerto, pode gerar outra cópia utilizável em quarentena. Não há deduplicação global prometida.

## Ciclo de vida, admissão e shutdown

Lapin 4.12.0 usa auto_recover=false e zero retries por padrão; fontes instaladas de confirms, Stream/Drop de Consumer, acker e propriedades de conexão foram inspecionadas. O ciclo externo conecta preservação antes de assinar origem, declara topologia compatível, define QoS por consumidor (global=false) e usa ACK manual. Só uma operação é admitida por vez; nenhuma tarefa é criada por entrega. Prefetch limita entregas pendentes, não operações ativas; outras mensagens prefetched ficam sem processamento em buffers cliente/broker.

Após falha/incerteza, assinatura/conexão é fechada e o runtime aguarda pacing interrompível antes de nova admissão. Fechamento torna entregas sem ACK elegíveis a redelivery; não há loop imediato de nack/requeue. BasicCancel impede novas entregas da assinatura, mas não reenfileira sozinho o trabalho pendente. Cancelamento do servidor/fim do stream provoca substituição. Falhas de setup são espaçadas e têm orientação sanitizada; schema ausente falha antes da assinatura. Publisher aposentado substitui a sessão inteira.

SIGINT/SIGTERM interrompe admissão da aplicação, cancela assinatura e drena o future atual dentro do orçamento. No prazo final, descarta a operação sem fabricar ACK/preservação. Fechamento explícito limitado recupera entregas atuais/prefetched pelas regras do broker; pool compartilha deadline absoluto de shutdown. Falhas/prazos de close são registrados. Sem hooks próprios de Drop assíncrono ou tarefas de cleanup desacopladas; Lapin tem Drop interno, que não é usado como prova de rollback/retirada. Detecção/redelivery no broker pode atrasar após falha de transporte. Shutdown durante setup pode esgotar drain antes do setup; deadlines limitam espera da aplicação, não cleanup instantâneo do broker.

| Configuração (ms, exceto contagens) | Padrão | Limites |
| --- | ---: | --- |
| CONSUMER_PREFETCH | 1 | 1..32 |
| CONSUMER_OPERATION_MS (cada fase DB / preservação / ACK) | 10000 | 1..60000 |
| CONSUMER_RECONNECT_MS | 5000 | 1..60000 |
| CONSUMER_DRAIN_MS | 15000 | 1..20000, menor que shutdown |
| CONSUMER_SHUTDOWN_MS | 25000 | 1..25000 |
| CONSUMER_POOL_MAX | 2 | 1..16 |
| CONSUMER_POOL_ACQUIRE_MS | 5000 | 1..60000 |
| CONSUMER_STATEMENT_MS | 5000 | 1..60000 |
| CONSUMER_LOCK_MS | 3000 | 1..60000 |
| RABBITMQ_CONNECT_TIMEOUT_MS / PUBLISH_TIMEOUT_MS | 10000 | 1..300000 |

Operation limita cada fase, não a soma de todo processamento; shutdown mantém orçamento absoluto. Alinhar limites DB/publicação ao orçamento. Roles PostgreSQL precisam dos [grants restritos da 4.2](milestone-4-2.md). Runtime RabbitMQ exige read na origem e write na quarentena; declarações/bindings também exigem configure/read/write apropriados no escopo relay.*. Credenciais de inicialização não atualizam volumes existentes.

## Supervisor e rollout compartilhado pendente

[Launcher versionado](../../../docker/supervisor/relay-consumer) executa consumer compilado. [normal.conf](../../../docker/supervisor/normal.conf) acrescenta consumer com autostart=false, autorestart=unexpected, execução como developer, grupos, TERM/30s e stdout/stderr. Autostart HTTP/relay preservado. Grace Compose passa a 95s para três filhos sequenciais de 30s e margem; contagem de filas não tem relação com programas. **start all e restart all incluem consumer mesmo com autostart=false: não usar no compartilhado.** Worker smoke existente agora usa controles HTTP/relay nomeados.

Rollout continua pendente. App vivo não foi reconstruído/recriado/recarregado: manager ainda conhece apenas HTTP/relay. Quatro migrations compartilhadas terminam em 20261012000000; replay e consumidor pendentes. Futuro rollout autorizado exige:

1. Inspecionar/fazer backup do estado compartilhado e aplicar migrations pendentes na ordem suportada; conferir schema, constraints, triggers e privilégios restritos. Sem reset de volumes.
2. Revisar conteúdo real da origem e identidade lógica; consumir relay.local altera entregas reais. Conferir topologia/políticas, capacidade, máximo de mensagem/frame e permissões efetivas. Não substituir classic por quorum.
3. Compilar três binários e reconstruir/recriar somente app quando autorizado, instalando launcher/config; consumer continua autostart=false. Mudança de ambiente exige recriação. Não executar start all.
4. Consultar status nomeado e iniciar apenas `consumer` após autorização. Monitorar outcomes sanitizados, estoque/recibos, capacidade de quarentena e entregas sem ACK. Para recuar, parar apenas consumidor; preservar efeitos/recibos e mensagens.

## Evidência e escopo exato da 4.4

[Testes de aplicação](../../../backend/tests/consumer_application.rs): ordem, validação sem persistência, todas as falhas de preservação, erros primários/secundários, timeout/cancelamento e limites. Testes de propriedades cobrem fidelidade binária/tipos, bits float e limites; relógio pausado verifica pacing interrompível. [Testes reais](../../../backend/tests/consumer_runtime.rs) usam recursos UUID exatos e PostgreSQL isolado: 10→15, duplicata, conflito, bytes malformados/não suportados e isolamento de header forjado; mandatory return; rejeição/indisponibilidade DB; perda de resposta COMMIT seguida de Duplicate; confirm de quarentena perdido com duas cópias sem ACK antecipado; acker inutilizado e recuperação; binário com prefetch/TERM bloqueado/restart; cancelamento do servidor/reassinatura; shutdown ocioso; topologia incompatível preservada; opt-in e controle Supervisor nomeado isolado. Cleanup remove apenas filas/exchanges exatos e verifica ausência; harness existente verifica remoção de cada banco. [Checks datados](../testing/validation-results.md) separam resultados locais de CI configurada.

A 4.4 deve executar e fechar a matriz ampliada de encerramento real de processos: antes de commit DB; após commit no servidor antes do ACK; após confirm de quarentena antes do ACK; ACK de origem perdido; consumidores concorrentes/duplicatas/eventos distintos; replay SQL preservando identidade após efeito concluído; shutdown/perda de conexão durante preservação e cancelamento; capacidade/políticas de quarentena cheia/indisponível e recuperação operacional de oversize. Exigir observações independentes DB/broker, cleanup exato, coordenação limitada, matriz bilíngue de aceitação e premissas de disponibilidade/liveness. Estes cenários finitos não demonstram queda de energia, prontidão de produção, ordenação, justiça ou encerramento completo do Marco 4.

Referências primárias conferidas em 16/10/2026: [confirms e ACK no mesmo canal](https://www.rabbitmq.com/docs/confirms), [prefetch positivo; zero significa ilimitado](https://www.rabbitmq.com/docs/consumer-prefetch), [ciclo do consumidor](https://www.rabbitmq.com/docs/consumers), [segurança/alternativas DLX](https://www.rabbitmq.com/docs/dlx). A fonte instalada do Lapin 4.12.0 rege o comportamento efetivo do cliente.
