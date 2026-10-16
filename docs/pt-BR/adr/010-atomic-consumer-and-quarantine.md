[Other language](../../en/adr/010-atomic-consumer-and-quarantine.md) | [Documentation catalog](../index.md)

# ADR 010 — Efeito atômico de estoque e quarentena pela aplicação

Status: aceita para adaptadores futuros; 4.1 implementa somente contratos.

Deduplicar duravelmente por (consumer_name,event_id), comparar conteúdo imutável e gravar incremento/recibo na mesma transação PostgreSQL. Selecionar publicação persistente mandatory em quarentena com confirms sem return e somente depois ACK original. Preservar relay.local clássica.

Alternativa: DLX do broker com encaminhamento at-least-once exige origem quorum configurada com dead-letter-strategy at-least-once, overflow reject-publish, DLX/políticas/permissões e recursos suportados. Retém origem até confirmação do alvo, mas muda requisitos da fila. DLX clássico padrão não garante preservação sem perda. Nunca redefinir clássica como quorum no lugar nem apagar/recriar recursos compartilhados; outra fila quorum exigiria decisão/rollout posterior explícito.

Quarentena pela aplicação é compatível com a fila clássica, preserva bytes/diagnóstico limitado e identidade ausente. Custa publisher separado, verificação do binding, monitoramento e recuperação ritmada. Crash após confirm antes de ACK ou ACK/confirm incerto pode duplicar publicações. Preservação at-least-once, sem garantia de quarentena exatamente uma vez. Sem purge/replay/TTL/descarte automático. Falha do alvo nunca autoriza ACK.

[Contratos, matriz de ACK, limites, estudo e aceite 4.2–4.4](../architecture/milestone-4-1.md).

Fontes primárias consultadas em 13/10/2026: [RabbitMQ confirms](https://www.rabbitmq.com/docs/confirms), [DLX](https://www.rabbitmq.com/docs/dlx), [quorum](https://www.rabbitmq.com/docs/quorum-queues), [PostgreSQL INSERT](https://www.postgresql.org/docs/18/sql-insert.html). Confirm e ACK do consumidor são independentes; verificar returns mandatory mesmo com confirm positivo. ON CONFLICT coordena dentro da transação local completa.

13/10/2026: [protocolo de banco 4.2 implementado e justificativa do trigger diferido](../architecture/milestone-4-2.md). Representação BYTEA v1; reserva/conclusão do recibo e estoque compartilham commit. Quarentena/runtime permanecem em 4.3.

16/10/2026: [4.3 implementa runtime consumidor opt-in e quarentena confirmada](../architecture/milestone-4-3.md). Rollout compartilhado e aceitação completa de crash 4.4 continuam pendentes.
