[Other language](../../en/architecture/milestone-4-1.md) | [Documentation catalog](../index.md)

# Marco 4.1 — Contratos do consumidor de estoque

Implementado em 13/10/2026: modelos puros validados, portas de aplicação e testes de política. Planejados: schema/adaptador PostgreSQL, runtime consumidor e topologia de quarentena. Frontend continua ilustrativo. Demonstração local; não é integração comercial de estoque nem transação de produtor/outbox de negócio.

## Efeito, identidade e comparação

Tipo inventory.units_received, versão 1, payload exatamente warehouse_id, sku e quantity. Identificadores, inclusive aggregate type/id e nome lógico, têm 1–128 bytes ASCII alfanuméricos, ponto, underscore ou hífen, sem normalização. Quantidade é inteiro JSON positivo até i64::MAX; 5.0, expoentes e strings são inválidos. Estoque BIGINT não negativo; linha ausente começa em zero. Soma verificada: overflow aborta toda a operação e exige preservação/revisão.

Chave única (consumer_name,event_id), nome estável inventory.stock.v1 compartilhado entre réplicas. Outro consumidor pode aplicar efeito próprio. ID diferente, mesmo payload, é outro evento; identidade de operação comercial fica fora do escopo. Delivery tag, processo, conexão, redelivered, aquisições e ciclo de replay não substituem identidade. UUID do envelope não nulo e AMQP message_id parseável e correspondente são obrigatórios; identidade ausente/malformada/divergente é preservada sem inventar ID.

Representação verificada v1 compara todos os campos imutáveis do envelope: ID, tipo, aggregate type/id, versão, instante, correlation/causation e payload. Ordem de objetos/whitespace JSON não importa; strings e ordem de arrays são exatas, timestamps comparam instantes UTC, opcionais ausentes/null restauram None. serde_json arbitrary_precision preserva representação numérica; somente inteiro é suportado. Campos extras no payload rejeitam; extras no envelope seguem a restauração existente e são ignorados. Headers de transporte e redelivery ficam fora. Sem hash, Debug ou comparação ingênua de strings.

Mesmo ID/conteúdo: Duplicate; conteúdo alterado: IdentityConflict. A comparação futura não pode depender só de igualdade JSONB que apaga distinções numéricas. Persistir versão e conteúdo sem perdas. Duplicate retorna recibo original confirmado, com stock_after histórico, não estoque atual nem contagem de tentativas.

[Modelos](../../../backend/src/domain/consumer.rs), [portas/política](../../../backend/src/application/consumer.rs), [testes puros](../../../backend/tests/consumer_contract.rs).

## Transação proposta — 4.2

Schema consumer separado: receipts com PK (consumer_name,event_id), versão/conteúdo imutável, item, quantidade, stock_after e tempo de processamento; stock com PK (warehouse_id,sku), BIGINT CHECK units >= 0. Não usar relay.outbox_events.processed como recibo. Retenção inicial indefinida; apagar recibos pode permitir reaplicação.

Transação READ COMMITTED: INSERT de ownership do recibo ON CONFLICT DO NOTHING RETURNING. Índice único coordena cópias concorrentes; concorrente espera commit/rollback. Sem inserção, novo statement lê recibo confirmado e compara; ausência/decodificação inválida não prova Duplicate. Proprietário faz INSERT de estoque ausente ou ON CONFLICT UPDATE com soma verificada RETURNING, e completa recibo na mesma transação. Recibo proposto possui finalized=false inicialmente e resultados nullable durante ownership. Constraint trigger DEFERRABLE INITIALLY DEFERRED relê o recibo atual no commit e exige finalized=true, stock_after não nulo/não negativo e timestamp não nulo. Update do estoque e finalização precedem COMMIT; placeholder confirmado é proibido. Papel consumidor não pode apagar recibos; todas as escritas do adaptador usam essa transação. Testar trigger diferido/privilégios em 4.2.

Eventos distintos serializam pelo update da linha de estoque. Falha entre escritas/overflow/abort confirma nenhum dos dois. Terminação libera trabalho não confirmado; descarte de future sozinho não prova rollback. CommitUncertain exige recuperação pelo mesmo ID/conteúdo durável, sem compensação ou incremento independente. Atomicidade cobre somente banco local, não HTTP, email ou notificações.

## Matriz de ACK manual

| Evidência | Ação |
| --- | --- |
| Applied com commit reconhecido / Duplicate verificado | ACK individual no canal/tag original |
| Falha transitória antes do commit | Manter recuperação, pausar admissão e usar backoff/reconexão limitados |
| CommitUncertain / cancelamento durante commit | Não ACK; recuperar mesma identidade via operação durável |
| IdentityConflict / inválido / não suportado / overflow | Preservar em quarentena antes do ACK |
| Quarentena confirmada sem retorno | ACK original; falha/incerteza do ACK pode causar redelivery |
| Return/nack/indisponível/timeout/cancelamento da publicação | Não ACK; recuperação ritmada e aposentar canal incerto |

Commit seguido de ACK perdido não repete efeito graças à deduplicação. Sem nack/requeue imediato ilimitado ou ACK coletivo que retire mensagens não preservadas. Cancelamento após admissão/send não prova rollback/preservação. Shutdown futuro drena trabalho limitado e fecha canal para recuperação; runtime ausente em 4.1.

## Quarentena gerenciada pela aplicação — decisão selecionada

Preservar fila clássica relay.local, sem redefinir como quorum, apagar/recriar ou alterar políticas. Consumidor demonstrativo futuro usa essa fila explicitamente; outra responsabilidade precisará de fila própria pois consumidores na mesma fila competem. Planejados exchange direto durável relay.quarantine, fila clássica durável relay.inventory.quarantine e binding exclusivo inventory.stock.v1. Publicação persistente, mandatory e publisher confirms sem basic.return correlacionado; verificar binding/alvo exatos antes de consumir, pois mandatory sozinho prova roteamento para alguma fila. Nenhuma topologia criada agora.

Corpo original byte a byte, diagnóstico por enum fechado e propriedades originais em sidecar JSON versionado sem perdas, com tipos AMQP explícitos, binários base64 e valores aninhados tipados; preservar presença de message_id. Novas propriedades usam persistência/formato v1; expiration original fica só como evidência, sem TTL aplicado à quarentena. Encoder/decoder Lapin é requisito 4.3; modelo puro valida objeto/tamanho, não codificação AMQP completa. Sem exceptions, credenciais ou trechos de payload em diagnósticos/logs. Canal/tag são contexto, nunca IDs.

Limites: corpo 1 MiB, sidecar JSON UTF-8 16 KiB, message_id 255 bytes, motivo enum. Sem truncar: excesso mantém original sem ACK, pausa consumo e alerta para preservação externa explícita. Alinhar prefetch, memória, tamanho máximo do broker e capacidade antes do rollout. 4.1 não impõe limite novo ao produtor existente.

Crash após confirm antes do ACK, ou ACK original incerto, pode publicar duas cópias iguais. Confirm perdido também permite repetição. Publicação e ACK não são transação única. Manter cópias como evidência; agrupar por identidade válida é informativo, não autorização para remover. ID ausente/malformado não permite deduplicação segura. Fake puro publica duas vezes; experimento real fica em 4.4.

Alvo ausente/indisponível/cheio, return/nack/confirm desconhecido: não ACK, pausar admissão e recuperar com ritmo limitado. requeue=false não garante preservação. Papel futuro precisa read na origem/write no exchange; provisionador configure e permissões de binding. Configuração inicial atual aceita relay.*; verificar permissões e políticas efetivas em 4.3.

Sem purge/replay automático, TTL, expiração de fila ou descarte por drop-head. Limites futuros somente reject-publish com alertas/ritmo, verificando políticas existentes. Ausência de remoção não resolve crescimento: operador monitora bytes/disco/capacidade e backup, autoriza remoção/replay auditados explicitamente. Replay/remoção da quarentena e reparo de identidade/conteúdo precisam protocolos separados. API SQL de replay não controla mensagens do broker nem desfaz estoque.

## Próximas fatias e aceite

| Fatia | Entregas/evidência |
| --- | --- |
| 4.2 | Schema/constraints/adaptador atômico; armazenamento v1 sem perdas e erros de commit classificados. PostgreSQL isolado real: cópias concorrentes, conflito, eventos distintos no mesmo item, overflow, falha entre escritas, terminação e COMMIT perdido com observação independente. |
| 4.3 | Runtime RabbitMQ próprio, ACK individual, prefetch/admissão/shutdown limitados, topologia/permissões verificadas, sidecar tipado, mandatory/confirm/return e recuperação ritmada. Broker isolado: alvo ausente/indisponível, identidade/conteúdo inválido preservado, nenhum ACK antecipado. |
| 4.4 | Experimentos reais: duplicatas concorrentes, falha antes do commit, crash após commit antes do ACK, eventos distintos concorrentes, CommitUncertain, preservação inválida, crash após confirm da quarentena, ACK perdido e replay com ID já processado. Observação independente, limpeza exata e fechamento bilíngue. |

Pré-requisitos exatos de 4.2: finalizar persistência v1 com distinções numéricas, trigger diferido de recibo completo, privilégios, mapeamento de erros incluindo overflow, retenção de recibos, harness CREATE DATABASE e proxy de falhas existentes. Nenhuma migration compartilhada para validar isoladamente.

## Estudo: 10 + 5 = 15

Implementado: evento canônico em relay.outbox_events; publicação significa aceite do broker. Relay processed não significa efeito de estoque. Planejado: RabbitMQ entrega manualmente a inventory.stock.v1; transação grava recibo e soma cinco em consumer.stock: 10 + 5 = 15. Reentrega de A encontra mesmo recibo/conteúdo: permanece 15. Evento B distinto soma cinco: 20. ACK perdido altera entregas, não unidades.

SQL dead_letter preserva falha do relay antes da conclusão de publicação; quarentena do consumidor preserva mensagens que ele não aplica com segurança. Replay SQL abre ciclo do relay preservando identidade/conteúdo; não desfaz estoque nem faz ID processado somar novamente. Replay do broker é separado e planejado. [Guia existente](../guides/project-guide.md), [revisão do Marco 3](../operations/milestone-3-review.md). Modelos/testes puros implementados; efeitos no banco/runtime do exemplo planejados.

Continuação de 13/10/2026: [4.2 implementa efeito PostgreSQL e deduplicação](milestone-4-2.md). O texto anterior conserva o escopo de 4.1; runtime de broker permanece planejado.
