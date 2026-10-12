[Other language](../../en/architecture/milestone-3-1.md) | [ADR 008](../adr/008-durable-failure-policy.md)

# Marco 3.1 — Política durável de falhas e contratos de recuperação

Data: 2026-10-12. Política decidida para implementação em 3.2–3.6. Neste marco existem apenas cálculos puros; reagendamento, isolamento e replay duráveis continuam planejados. A conclusão do Marco 2 permanece válida dentro do seu escopo.

## Base e garantias

Foram inspecionados a aceitação do Marco 2, matrizes 2.3–2.6, ADR 007, domínio de entrega, contratos/erros de persistência, operações PostgreSQL, publisher/RabbitMQ, deliver_once/worker, migrations, testes de integração/recuperação e cenários Vue. Evidências antigas permanecem históricas; execuções atuais constam em testing/validation-results.

ID e envelope original permanecem estáveis. Token novo identifica aquisição; eventos com lease continuam pending. attempt_count conta aquisições confirmadas, inclusive crash antes do envio; adquirir não prova publicação iniciada. Conclusão exige confirmação do broker e autoridade no banco. Token não cerca publicação externa. CommitUncertain não prova commit nem rollback. Falha/incerteza de conclusão não autoriza republicação cega ou release compensatório. Release atual preserva available_at, last_error e contador. Espera do worker não é agenda durável. Os ports atuais não oferecem reconciliação genérica.

## Autoridade e recuperação

**O**: bloquear ID exato; depois consultar clock_timestamp() PostgreSQL. Exigir pending, token esperado, acquired_at <= agora < expires_at, ciclo/revisão esperados. Transição futura altera estado/diagnóstico/revisão e limpa lease atomicamente sem incrementar contador.

**M**: manutenção bloqueia linha bruta por ID sem OwnedOutboxEvent; revalida pending, revisão/snapshot esperados e lease ausente ou expirada pelo relógio pós-lock. Não sobrescreve dono ativo. Toda mutação relevante deve incrementar revisão; escritores externos precisam seguir contrato ou exigir comparação completa do snapshot.

**R**: leitura independente por ID confirma estado/token/ciclo presentes, nunca ausência de aceite externo. Recuperação atual depende de elegibilidade e token novo após expiração; pode duplicar. Ausência em fila não prova ausência de efeito no consumidor. Reconciliação futura exige fonte de evidência e port próprios.

## Matriz de decisões

Reagendar/isolar são propostas futuras; release de 2.4 permanece inalterado.

| Fronteira | Conhecido/incerto e claim | Publicação / alteração durável possível | Ação e autorização futura | Predicados / evidência |
| --- | --- | --- | --- | --- |
| Conexão/setup publisher indisponível | Sem aquisição/claim; causa pode ser infraestrutura/configuração | Sem envio ou escrita de evento | Recuperar infraestrutura; espera limitada; sem reagendar/isolar evento | Saúde/correção da configuração; aquisição nova |
| Aquisição Unavailable/OperationFailed | Sem claim retornado/envio; falha ordinária não é COMMIT ambíguo | Sem aquisição reconhecida; cancelamento exige avaliação de fase | Esperar/reparar banco; sem mutação por esse resultado | Evidência da fase e R |
| Aquisição CommitUncertain | Token conhecido, claim utilizável ausente; commit desconhecido | Lease/contador podem ter sido gravados; sem envio desta chamada | Não reutilizar token/publicar/release/reagendar; observar ou aguardar expiração | R; somente aquisição futura nova fornece autoridade |
| Dados selecionados inválidos | Restauração falhou antes da mutação; sem envelope possuído | Sem envio/aquisição pelo adapter inspecionado | Diagnóstico bruto; M isola invalidade persistente comprovada | M e validação; erro atual não expõe ID, exige scanner limitado |
| Serialização | Claim existente; antes do envio; conteúdo ou implementação | Sem publicação; aquisição confirmada | Defeito global: reparar/release O; defeito permanente comprovado: isolar O; corrigível/incerto: reagendar O com orçamento | Reprodução/versão do validador; categoria sozinha não prova poison |
| Publicação Unavailable | Claim; contrato garante antes do envio | Sem envio; aquisição consumida | Substituir infraestrutura/release O preservando agenda; orçamento continua consumido | Contrato before-send e resultado independente do release |
| Unroutable | Claim; rota exigida falhou, topologia pode ser corrigida | Broker viu envio; aceite roteado exigido ausente | Padrão reagendar O; corrigir topologia; defeito global comprovado permite suspender admissão/release O | Retorno/rota; não inferir efeito do consumidor |
| Rejected | Claim; rejeição explícita, causa variável | Envio ocorreu; aceite não confirmado | Padrão reagendar O; isolamento só com defeito permanente independente | Rejeição + evidência sanitizada; O/orçamento |
| Publicação Uncertain | Snapshot do claim; aceite desconhecido | Aceite possível; aquisição confirmada | Reter lease/substituir publisher; sem release/reagendamento/terminal imediato | R/expiração; duplicação possível; sem reconciliação atual |
| Publicação confirmada + OwnershipLost | Aceite conhecido; claim antigo sem autoridade | Outro dono/estado pode existir | Parar ação do claim; sem release/reagendar/isolar/republicar | R; autoridade do dono atual |
| Confirmada + falha ordinária de conclusão | Aceite conhecido; autoridade não presumida | Conclusão não reconhecida; rejeição de servidor pode provar rollback, erro de acesso depende da fase | Sem compensar/reagendar/republicar; reparar/observar/expiração | R; recuperação pending pode duplicar |
| Confirmada + CommitUncertain | Aceite conhecido; conclusão desconhecida | processed pode estar confirmado no banco | Sem enviar/release/reagendar; observar ou elegibilidade normal | R processed impede aquisição; pending não prova ausência de aceite |
| Release falhou / OwnershipLost | Preservar falha primária; snapshot não renova autoridade | Lease pode continuar; dono perdido não pode alterar | Esperar/reparar/observar; sem repetir cleanup cegamente | R; nova aquisição com predicados normais |
| Release CommitUncertain | Falha primária conhecida; limpeza incerta | Ownership pode ter sido limpo; contador preservado | Sem repetir cleanup/reagendamento cego | R; falta de dono não autoriza escrita por ID |
| Cancelamento/crash | Resultado tipado pode faltar; fase desconhecida | Aquisição/envio/transição podem ter ocorrido | Sem cleanup no Drop; expiração/token novo; verificar orçamento antes de enviar | R + evidência de fase; duplicação possível |
| Orçamento esgotado | Contador vitalício menos baseline >= máximo; não conta envios | Aceite anterior pode existir | M terminaliza sem dono/lease expirada antes de adquirir; O após falha definitiva final; sucesso final ainda conclui | Revalidar ciclo/orçamento; acquisition_budget_exhausted |
| Limite de armazenamento | attempt_count=i32::MAX, separado da política | Efeitos históricos desconhecidos | M terminaliza storage_counter_exhausted; sem wrap/saturação/processed; replay comum não resolve capacidade | M; histórico preservado; ampliação de tipo exige migration própria |
| COMMIT futuro de agenda/isolamento incerto | Reconhecimento ausente | Agenda/terminal podem ter sido gravados | R e predicados atuais; sem nova mutação/envio cego | Testes de falha 3.2; ports ainda ausentes |

NoEvent continua idle; Published/Applied continua. Todas as categorias atuais de aquisição, publicação, conclusão e ReleaseOutcome estão cobertas. Worker atual substitui publisher em Unavailable/Uncertain e aplica pacing às outras falhas; integração da nova política pertence a 3.3.

## Agenda durável

Padrão: reagendar Unroutable/Rejected definitivos e serialização específica corrigível ou sem prova permanente, enquanto houver orçamento. Defeito permanente comprovado isola. Indisponibilidade before-send e defeitos globais de encoder/topologia exigem recuperação da infraestrutura; não provam invalidade do evento. Aquisições já confirmadas continuam consumidas, inclusive corrida entre checagem da conexão e aquisição.

Máximo padrão: **10 aquisições por ciclo**, inclusivo. Contagens 0–9 permitem adquirir; 10 permite concluir a última aquisição já feita, mas não reagendar nem adquirir a 11ª. Falha definitiva final terminaliza; incerteza retém lease e manutenção só terminaliza após expiração. Evento terminal pode ter sido aceito pelo broker: terminal significa interrupção pela política, não prova de não entrega. Crash antes do envio final consome a última vaga.

Backoff padrão: base **1.000.000 µs**, teto **300.000.000 µs**. Para aquisição n do ciclo (base 1), ceiling=min(teto,base*2^(n-1)); dobrar com aritmética verificada até o teto. Equal jitter uniforme em microssegundos inteiros de ceil(ceiling/2) a ceiling; piso positivo evita zero. Configuração válida: 0 < base <= teto <= 86.400.000.000 µs; orçamento 1..i32::MAX. Sem retry hints; contrato futuro precisaria limites explícitos sem contornar teto/orçamento.

Aplicação escolhe atraso validado com aleatoriedade explícita; repositório consulta UTC PostgreSQL após lock, verifica soma e grava available_at=agora+atraso junto com diagnóstico/revisão/limpeza de lease na mesma transação O. Cálculo puro com relógio explícito modela fórmula, não autoriza SQL por horário da aplicação. Rejeitar precisão inferior a microssegundo, overflow e timestamp fora da interseção Chrono/PostgreSQL; nunca arredondar silenciosamente. Lease expirada retorna OwnershipLost sem alterar linha; configuração inválida falha antes da mutação. Agenda/diagnósticos/contadores/ciclo confirmados sobrevivem restart; COMMIT incerto exige observação, não recalcular/repetir cegamente. Pacing continua proteção de admissão/infraestrutura e não encurta available_at. Sem loop de retry em adapters/repositórios.

## Orçamentos e schema futuro

Aquisições são observáveis atomicamente, mas cobram crash/outage antes do envio. Contador de publicações descreveria melhor envios, porém não é mensurável atomicamente com PostgreSQL sem novo protocolo de intenção/ack e ainda teria incerteza. Orçamento por replay permite recuperação controlada preservando histórico. Escolha mínima: aquisições por ciclo com baseline persistido, sem contador especulativo de envio ou reset vitalício.

Migration aditiva 3.2 proposta: ID do ciclo, baseline, máximo do ciclo, revisão; código/hora/metadados sanitizados de diagnóstico; hora/motivo terminal e evidência preservada conforme necessário. Inicializar linhas existentes com baseline **0**, máximo 10 e revisão 0: aquisições históricas contam; linhas já >=10 precisam manutenção sem novo envio. Não conceder orçamento novo implicitamente usando baseline=contador atual. Estados terminais/processed existentes permanecem. Preservar envelope, contador, histórico diagnóstico e available_at até transição autorizada. Auditar quantidade esgotada antes de ativar; nenhuma migration compartilhada neste marco. Persistir máximo evita alteração por restart/configuração. Replay usa baseline=contador vitalício atual e orçamento explicitamente auditado, respeitando capacidade física independente.

Aquisição deve verificar atomicamente due/pending/lease/ciclo/capacidade antes de incrementar. Agenda revalida orçamento O; isolamento não incrementa. Linhas excluídas por esgotamento precisam M para não ficar invisíveis indefinidamente. Definir representabilidade de tempo/ports e validar concorrência/COMMIT em 3.2.

## Isolamento e diagnósticos

Escolha: estado existente **dead_letter** na própria outbox com evidência/auditoria estruturadas. Evita mover/apagar eventos e criar duas autoridades de lifecycle. Tabela separada de quarentena favoreceria separação de acesso/evidência bruta, mas adiciona cópia atômica e consistência de replay; registro lateral de auditoria pode complementar a linha preservada. DLQ do broker não isola evento que nunca publicou; quarentena da aplicação é decisão no banco.

Evidência permanente: falha reproduzível de validação ou incompatibilidade específica documentada do encoder. Categoria, duração de outage ou falha de topologia não bastam. Esgotamento é motivo operacional distinto de poison. Códigos: invalid_stored_envelope, serialization_permanent, acquisition_budget_exhausted, storage_counter_exhausted. Metadados allowlist: versão de política/validador, fase, referência de evidência; sem payload, driver text, credenciais ou URI. Limite proposto: 2 KiB JSON UTF-8 serializado; rejeitar excesso em vez de truncar. Linha bruta é evidência protegida, não diagnóstico público.

Linha válida usa O; envelope irrestaurável usa M sem OwnedOutboxEvent. Erro atual de aquisição não expõe ID: 3.4 precisa scanner limitado de linhas brutas, validando/isolando cada uma em transação curta própria. Deve tratar inválidas/esgotadas sem obstrução repetida de trabalho válido. SKIP LOCKED não garante fairness. Ownership/identidade corrompidos exigem reparo privilegiado auditado em janela de manutenção; não inventar envelope, apagar evidência ou limpar dono ativo. Envelope irrestaurável fica preservado terminal e replay é recusado até reparo separado autorizado/auditado; replay normal não altera conteúdo.

Marco 3 não apaga automaticamente por retenção. Restringir linha/evidência bruta a operadores e expor motivos sanitizados. Prazo de retenção, privacidade/remoção legal e autorização de operadores são pré-requisitos de implantação. Isolamento mantém processed_at NULL e não conta sucesso de entrega.

## Replay futuro — 3.5

Elegíveis: somente dead_letter da aplicação com envelope original restaurável; pending/processed/live owner são recusados. Intenção explícita autenticada informa revisão, motivo, request ID e ciclo esperados. Bloquear/revalidar estado/revisão/sem dono; gravar atomicamente actor, reason, horários de solicitação/aplicação pelo banco, ciclos anterior/novo, baseline/contador/orçamento anteriores e novo máximo. Preservar ID/envelope/contador vitalício; baseline novo=contador atual, ciclo novo, available_at=relógio pós-lock; limpar metadados terminais do estado atual preservando auditoria. Recusar esgotamento físico; ampliar capacidade requer desenho separado. Nenhum endpoint/UI/tool agora.

Request ID único com auditoria permite retornar resultado confirmado para repetição idêntica; reutilização divergente conflita. Requests concorrentes na mesma revisão permitem uma transição. CommitUncertain exige consulta por request ID/estado/ciclo, nunca request novo cego. Essa observação será implementada em 3.5; ports atuais não oferecem. Reparo e replay são intenções diferentes. Deduplicação do consumidor pelo ID original pode intencionalmente produzir **nenhum efeito adicional** se registro e efeito já foram confirmados atomicamente. Replay renova intenção de entrega, não garante novo efeito ou exactly-once.

## Fatias restantes e aceitação

| Marco | Entradas / entregas | Aceitação / pré-requisitos |
| --- | --- | --- |
| 3.2 | ADR 008/007 e política; migration versionada, modelos de ciclo/diagnóstico, ports agenda/terminal, aquisição limitada e contrato mínimo M | Backfill/migrations isolados; relógio pós-lock, O/M, fronteiras, identidade/histórico, stale owner/concorrência/perda de COMMIT; sem rollout compartilhado. Definir cobertura de revisão/faixa temporal; scanner bruto fica 3.4 |
| 3.3 | Ports 3.2; integração contextual deliver_once/worker, agenda definitiva, pacing/replacement e admissão da manutenção | Cobrir todos os outcomes/erros secundários/cancelamentos; sem release incerto ou compensação; agenda sobrevive restart; evidência explícita |
| 3.4 | Autoridade M; scanner bruto, evidência/validação, observação sanitizada | Isolar inválido sem owned envelope, preservar dono ativo/evidência e progresso de válidos; tratar esgotados sem sucesso fictício; acesso/retenção dependem da implantação |
| 3.5 | Terminal preservado, ciclo/revisão, auth e lookup de request | Mesmo ID, novo ciclo/histórico; requests repetidos/concorrentes/incertos; recusar inválido/esgotado físico; sem editar conteúdo; dedup explicado sem alegar consumidor implementado |
| 3.6 | 3.2–3.5 e harness isolado PostgreSQL/RabbitMQ | Experimentos de restart/due/final-slot crash/incerteza de envio/conclusão/agenda/replay, progresso poison e auditoria; evidência datada/aceitação; frontend ilustrativo |

Gating de aquisição e contrato mínimo de manutenção de esgotados entram em 3.2 para não abandonar linhas invisíveis; orquestração chega em 3.3/3.4. Nenhuma configuração é ativada em 3.1.

## Frontend e limite da evidência

Vue/cenários continuam inalterados e sem infraestrutura. Cenários ilustrativos futuros podem explicar due persistido versus espera, última aquisição/esgotamento, quarentena preservada, replay/ciclo novo com ID original e dedup sem efeito. Ação/status/progresso precisam ser distinguíveis; quantidade de botões depende do contexto. Simulação não valida capacidade backend.

[Cálculos puros](../../../backend/src/application/retry_policy.rs) e [testes](../../../backend/tests/retry_policy.rs) verificam fronteiras de orçamento/histórico, teto exponencial/jitter, precisão e overflow. Não provam agenda durável, SQL concorrente, quarentena, replay ou recuperação. Sem mudanças de dependências/migrations/adapters/use case/worker/frontend neste marco.


Contagens anteriores à aquisição por ownership não podem ser reconstruídas; baseline zero preserva conservadoramente seu impacto no orçamento. Ports futuros de agenda/terminal executam uma transação e retornam Applied ou OwnershipLost quando O não corresponde, preservando categorias PersistenceError incluindo CommitUncertain. Repetição reconhecida com token/revisão antigos é recusada, não sucesso idempotente. Conflito de revisão M não altera linha. Aritmética de revisão/contador de auditoria deve ser verificada; esgotamento recusa transição sem wrap.
