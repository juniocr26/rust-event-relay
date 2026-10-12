[English](../../en/operations/milestone-2-review.md) | [README](../../../README.pt-BR.md)

# Revisão de aceitação do Marco 2 — 2026-10-12

**Fechado no escopo implementado, com evidência limitada. Rollout compartilhado permanece pendente separadamente.** A pipeline é uma **fundação de entrega pelo menos uma vez sob premissas explícitas**. Não constitui prova de entrega universal nem efeitos exactly-once no consumidor. Nenhum defeito material no escopo implementado foi confirmado e permaneceu aberto nesta matriz finita.

## Requisitos e evidências

| Requisito | Implementação | Evidência e limite |
| --- | --- | --- |
| Posse durável | [ADR 007](../adr/007-durable-delivery-ownership.md); aquisição commitada/token/count/lease no PostgreSQL | [Regressões de posse](../../../backend/tests/postgres_ownership.rs) e [A/B do 2.6](../architecture/milestone-2-6.md): pending commitado sobrevive SIGKILL do processo exato. Sem prova de power loss. |
| Confirmar antes de concluir | [deliver_once](../../../backend/src/application/delivery.rs); mandatory/confirm no publisher | B para depois do sucesso contratual e antes de completion; Uncertain não conclui; trigger de completion só é alcançado depois da confirmação. Aceitação broker não significa sucesso de negócio do consumidor. |
| Identidade estável | EventEnvelope original/message_id AMQP | Baseline e todas as duplicatas observadas mantêm IDs inseridos; sem produtor/transação de negócio implementados. |
| Recuperação incompleta | Elegibilidade pelo relógio SQL após expiração; AcquireRequest novo | A/B, rejeição/deadline SQL, shutdown e incerteza recuperam com count 2/token novo. Experimentos novos não editam expiry. Exige disponibilidade eventual e worker ativo/reiniciado. |
| Proteção concorrente | Transações curtas/SKIP LOCKED/tempo pós-lock/predicados de token | Regressões concorrentes e dois binários reais com tokens distintos duráveis/leases vivos/barreira de completion. Contagem broker não é usada sozinha; sem fairness. |
| Autoridade antiga | Pending/token exato/intervalo não expirado | A complete/release rejeitados depois de B adquirir; token B preservado. Publisher anterior ainda envia: tokens SQL não fazem fencing no RabbitMQ. |
| Incerteza externa | Lease mantido/adapter aposentado/substituição pelo worker | Confirm retido com mensagem aceita observada independentemente; desconexão do proxy próprio causa substituição sem replay do claim. Outras incertezas podem significar aceitação ou rejeição. |
| Incerteza SQL | CommitUncertain conservador, sem compensação cega | Caso de uso confirmado e perda de resposta COMMIT em completion: processed/count 1 por pool independente; restart NoEvent/sem replay. Demonstra um commit incerto efetivamente durável. |
| Ciclo de vida operável | Binário separado/gate schema/pacing/reconexão/shutdown limitado | Smokes binário/Supervisor; outage handshake/restauração; TERM idle, INT reconnect, TERM completion bloqueado; regressões determinísticas de budgets. Apenas schedules cooperativos testados. |
| Janelas de duplicação | Aceitação externa fora da transação SQL | B/C, rejeição completion, timeout SQL e shutdown repetem ID original. Sender antigo também envia. Tratamento de duplicatas do consumidor continua separado. |

## Premissas e exclusões

Eventos de origem devem estar previamente commitados de forma durável, válidos e elegíveis. PostgreSQL/RabbitMQ precisam cumprir comportamento suportado de transações/confirmações/storage. Filas/exchanges duráveis e mensagens persistentes estão configurados; power loss, corrupção, HA replicado/TLS produtivos não foram estabelecidos. Disciplina do relógio SQL e tempo restante de lease importam; acquire pode retornar lease já expirado, sem renovação.

Infraestrutura deve voltar a funcionar, worker deve executar/reiniciar e falhas do evento devem resolver antes do esgotamento do contador. Rejeição permanente, dados malformados/poison e count esgotado podem impedir progresso. Sem retry durável/backoff/jitter, quarantine, transição dead-letter ou replay. Sem prova universal de liveness, fairness, ordem de negócio ou throughput. Fixtures não demonstram atomicidade negócio/outbox; ACK do observador não demonstra efeito de negócio/idempotência. Consumidores precisam tratar duplicatas adequadamente.

Budgets de drain/cleanup não cobrem power loss, SIGKILL, kernel parado ou todo schedule de cancelamento. Perda antes do COMMIT observado, resultados incertos arbitrários e HA/TLS produtivos são lacunas de evidência, não defeitos novos demonstrados. Casos finitos conferem conjuntos exatos de IDs, banco e broker independentemente; terminal failure/descarte/dead-letter nunca contam como entrega bem-sucedida.

## Operação e decisões do Marco 3

Migração compartilhada `20261009000000` pendente; http RUNNING, relay STOPPED/autostart=false. Evidência/migração isolada não autoriza rollout, e rollout pendente não invalida aceitação isolada. Sem recriação de serviços, purge compartilhado ou término de sessões alheias. Handoff (historical handoff unavailable in this checkout) registra validações e continuidade.

Antes do Marco 3: definir falhas retryable/permanentes/incertas; scheduling/backoff/jitter duráveis e semântica de attempts; isolamento poison/representação quarantine-dead-letter; replay auditável preservando identidade/autoridade; reconciliação após ACK desconhecido; aceitação de migrações/operação e autorização separada do rollout compartilhado. Idempotência de negócio do consumidor e endpoints produtores precisam de escopo próprio. Interface gráfica opcional somente após concluir o núcleo; nenhuma foi adicionada.
