[Outro idioma](../../en/architecture/milestone-3-3.md) | [ADR 008](../adr/008-durable-failure-policy.md)

# Marco 3.3 — Integração da política durável de falhas

Implementado em 2026-10-13 a partir de `e6f8a2f`; base da Engineering Library `303ed86`. Nenhum AGENTS.md aplicável ao projeto foi encontrado. O relatório anterior mencionado não foi fornecido; contratos, histórico e documentação datada de 3.1/3.2 foram verificados no checkout. Resultados anteriores permanecem históricos. [Validação executada](../testing/validation-results.md#marco-33--2026-10-13).

## Contrato e decisões contextuais

[deliver_once](../../../backend/src/application/delivery.rs) executa uma aquisição, no máximo uma publicação do envelope original inalterado e uma transição selecionada. Usa ciclo, baseline, máximo, revisão e token retornados. Nunca reinicia o contador vitalício. Publicação confirmada permite conclusão inclusive na aquisição final. Somente conclusão Applied reconhecida significa entrega bem-sucedida; agendamento e preservação terminal não significam entrega.

| Resultado/contexto | Ação no evento | Ação do worker |
| --- | --- | --- |
| None | Nenhuma publicação/transição | Pausa ociosa |
| Erro na aquisição, inclusive CommitUncertain | Sem claim utilizável ou ação posterior | Pausa de falha |
| Publicação confirmada | Conclui uma vez; mantém Applied/OwnershipLost/erro separado | Continua apenas em Applied; demais usam pausa de falha |
| Publicação Uncertain, inclusive final | Retém lease; nenhuma transição | Substitui publisher e usa pausa de reconexão |
| Atestado explícito de serialização permanentemente inválida por evento | Preserva serialization_permanent uma vez | Pausa de falha; mantém publisher utilizável |
| Falha definitiva no orçamento final | Preserva acquisition_budget_exhausted; storage_counter_exhausted tem precedência no limite i32 | Pausa de falha ou substituição para Unavailable |
| Unroutable/Rejected com orçamento | Agenda uma vez com jitter | Pausa de falha e retoma admissão geral |
| Unavailable comprovadamente antes do envio, com orçamento | Libera uma vez para recuperar infraestrutura, conforme ADR 008 | Substitui publisher; pausa de reconexão |
| Serialization sem atestado contextual | Retém lease: adaptador não distingue defeito global de problema permanente/corrigível por evento | Pausa de falha; informa categoria e retenção |

`PublishError::permanent_serialization(FailureEvidence)` representa atestado explícito confiável do chamador/adaptador sobre defeito de codificação reproduzido por evento. Construtores genéricos não autorizam esse motivo. RabbitMQ não fornece tal evidência hoje. Não inventamos validador, diagnóstico de topologia global, permanência de rejeição ou inferência de poison. Unroutable/Rejected autorizam agendamento; indisponibilidade exige recuperação de infraestrutura. Liberação comum nunca contorna a disponibilidade durável desses eventos. Categoria, decisão por evento e ciclo de vida do publisher são separados.

`PublicationFailed { claim, error, handling }` preserva erro/fonte sanitizados independentemente de `FailureHandling`: LeaseRetained, Released, Scheduled, Terminal ou PolicyFailed. Scheduled/Terminal reutilizam `Result<RetryTransition<T>, PersistenceError>`: Applied, Conflict, erro comum e CommitUncertain ficam explícitos. Released mantém Applied/OwnershipLost. Nenhuma transição é repetida; falha de conclusão não dispara compensação. Persistência falha/incerta não apaga a causa primária nem autoriza republicação.

## Equal jitter e relógio autoritativo

[Cálculos puros e amostragem](../../../backend/src/application/retry_policy.rs) reutilizam o teto verificado de RetryBackoff: min(300.000.000 µs, 1.000.000 µs × 2^(n−1)), com n=contador vitalício−baseline persistido. Intervalo uniforme inclusivo em microssegundos inteiros: [ceil(teto/2), teto]. `selected_delay(n, offset)` verifica limites e soma; `deliver_once_with_delay` permite seleção determinística. A seleção ocorre uma única vez por decisão de agendamento, nunca em conclusão, incerteza, retenção ou preservação terminal.

rand 0.8.8 já existia transitivamente no lockfile e foi promovido a dependência direta `rand = "0.8"`, sem baixar nova versão. `thread_rng().gen_range(0..=teto-piso)` usa amostragem inteira limitada com rejeição, sem módulo ingênuo. O gerador local à thread recebe semente do sistema operacional e não atravessa await nem concede autoridade. Testes determinísticos verificam extremos inclusivos, cap, aquisição zero/overflow e offsets inválidos, sem estatística frágil. [Fonte do rand](https://github.com/rust-random/rand/blob/0.8.5/src/distributions/uniform.rs). PostgreSQL calcula available_at com relógio próprio após lock; relógio da aplicação nunca autoriza escrita.

## Pausas e admissão de manutenção

Worker mantém uma operação em voo, tokens novos, verificação do publisher, esperas interrompíveis e os limites existentes de drain/encerramento. Toda falha de publicação, mesmo agendamento/preservação reconhecidos, usa RELAY_FAILURE_MS antes da admissão geral. Isso limita falhas repetidas e permite trabalho independente, sem esperar um evento específico. PostgreSQL impede aquisição antes de available_at. RELAY_RECONNECT_MS regula substituição do recurso; RELAY_IDLE_MS regula polling vazio. Resultados definitivos mantêm publishers utilizáveis; reconexão não reconcilia publicação anterior.

Origem dos candidatos: observação independente pelo operador de metadados esgotados fornece `RELAY_MAINTENANCE_CANDIDATES=UUID:cycle:revision,...`, vazio por padrão. Parsing rejeita UUID nil, ciclo/revisão inválidos, IDs repetidos, mais de 32 entradas ou 4096 bytes. Campo privado validado expõe slice somente para leitura. Compose encaminha configuração opcional; não houve restart compartilhado. Nenhuma fila ilimitada ou contrato de descoberta foi criado.

Antes de cada tentativa geral, após admissão de publisher utilizável, consome no máximo um candidato e chama preserve_exhausted pelo mesmo limite de drain. Máximo de 32 tentativas por execução; após Conflict/lock ignorado/erro/CommitUncertain, entrega normal permanece possível. Consome antes de await e não repete automaticamente nessa execução. Manutenção aguarda recuperação do broker: limitação deliberada de cobertura. Shutdown interrompe novas admissões.

Revisão fresca depende da observação independente; nunca é adivinhada ou incrementada pelo worker. PostgreSQL revalida ID/ciclo/revisão, pendência/disponibilidade e ausência de dono ativo usando SKIP LOCKED. Dono da aquisição final conserva a oportunidade de concluir. Conflict não identifica sozinho lock, dono ativo ou revisão alterada; observe novamente antes de nova execução explicitamente configurada. Restart relê entradas e pode tentá-las novamente: remova entradas antigas e reobserve antes de reiniciar. Não há cursor durável, descoberta automática, garantia de justiça ou cobertura geral do backlog esgotado. Incerteza/crash final pode exigir manutenção fornecida explicitamente; scanning pertence a 3.4.

## Cancelamento e incerteza

| Limite | Efeito possível / recuperação autorizada |
| --- | --- |
| Aquisição ou após claim reconhecido | Lease/contador podem persistir; sem claim inventado ou cleanup; somente nova autoridade futura |
| Publicação | Aceitação pode ocorrer; adaptador aposenta canal incerto; sem transição posterior da invocação descartada |
| Antes de agendamento/preservação | Cancelar antes de poll do próximo await impede nova chamada; claim anterior pode permanecer |
| Durante agendamento/preservação | Mudança durável pode preceder resultado cancelado; sem compensação destacada ou repetição cega |
| Conclusão | Aceitação confirmada; conclusão pode persistir; sem liberação/agendamento/terminalização/republicação |
| Manutenção por ID | Commit terminal pode ocorrer mesmo sem resultado; candidato consumido não é repetido automaticamente |

[Cancelamento no Tokio select](https://docs.rs/tokio/latest/tokio/macro.select.html#cancellation-safety) descarta future, sem provar rollback. Ocorre em await; decisão/amostragem síncronas não são fase interrompível separadamente. Não há mutação assíncrona em Drop ou cleanup destacado. Após incerteza, somente elegibilidade durável e autoridade válida permitem progresso, com risco de duplicata da identidade original; não há reconciliação genérica.

## Evidência e limites

Fakes controlados cobrem ausência de claim, envelope inalterado, resultados de conclusão, conflitos/erros de agendamento, terminalização final/permanente, causa primária, amostragem única, cancelamento e pausas/admissão/ciclo do publisher. Proxies reais de PostgreSQL testam perda de reconhecimento no caso de uso com publisher Rejected injetado, sem alegar NACK real. Remoção de rota no broker real produz Unroutable, agenda, impede aquisição antecipada e permite recuperação com token novo após correção; falha de rota final preserva dead_letter sem processed_at. Recuperação incerta existente demonstra duplicatas possíveis. Worker real configurado protege dono final ativo e entrega outro evento; nova observação após expiração autoriza manutenção em outra execução. Consulte validação para resultados efetivamente executados.

Sem mudanças em SQL, migrations, protocolo RabbitMQ, frontend ou consumidor. Migrations compartilhadas de ownership/retry continuam pendentes; relay permanece parado. Sem reset, alteração de credenciais, rollout, ativação, staging, commit ou push. Nenhuma alegação de scanner geral, replay ou fechamento do milestone.

3.4 deve implementar descoberta limitada de candidatos raw, validação de envelopes/evidência de poison e isolamento de invalidade comprovada sob autoridade de manutenção sem construir envelope owned válido; preservar histórico terminal, proteger donos ativos, definir revisão fresca/contenção/limites de justiça e evitar obstrução repetida de trabalho independente por linhas inválidas. Deve incluir descoberta de metadados esgotados, pois configuração exata não cobre backlog geral. Shapes inválidos de identidade/ownership exigem reparo auditado autorizado separadamente, nunca autoridade inventada. Replay autenticado/auditado fica em 3.5; aceitação/fechamento completos em 3.6.
