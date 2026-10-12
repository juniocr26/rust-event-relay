[Other language](../../en/architecture/milestone-3-2.md) | [ADR 008](../adr/008-durable-failure-policy.md)

# Marco 3.2 — Estado durável de retry e transições PostgreSQL

Implementado em 12/10/2026. Este marco fornece persistência; integração da política na aplicação fica em 3.3. `deliver_once`, loop do worker e frontend permanecem sem alterações. Backoff/jitter continuam cálculos puros de 3.1; o adapter não escolhe aleatoriedade nem repete transações.

## Schema e backfill

Migration aditiva `20261012000000_add_durable_retry`, posterior à de ownership `20261009000000`. Migrations aplicadas não foram reescritas. Identidade, envelope, status, disponibilidade, attempt_count vitalício, last_error e leases ativos/expirados são preservados.

| Coluna | Representação e invariante | Inicial/backfill |
| --- | --- | --- |
| retry_cycle | INTEGER > 0 | 1 |
| cycle_baseline | INTEGER >= 0 e <= attempt_count | 0 |
| cycle_maximum | INTEGER > 0 | 10 |
| state_revision | BIGINT >= 0 | 0 |
| diagnostic_code | Código estático permitido ou NULL | NULL |
| diagnostic_at | TIMESTAMPTZ finito ou NULL | NULL |
| failure_evidence | Objeto JSONB permitido, UTF-8 serializado <= 2048 bytes, ou NULL | NULL |
| terminal_at | TIMESTAMPTZ finito se e somente se dead_letter | Instante de observação pela migration em dead_letter legado |
| terminal_reason | Motivo estático se e somente se dead_letter | legacy_dead_letter em terminal legado |

Pending/processed não recebem metadados terminais. Dead_letter existente permanece terminal: `terminal_at=statement_timestamp()` registra observação pela migration, **não reconstrói o horário histórico da falha**. last_error legado é mantido integralmente como histórico restrito; operações novas não o expõem, apagam ou copiam para evidência estruturada. Baseline zero inclui aquisições históricas: contador >= 10 já esgotou o ciclo. Dono ativo da última aquisição ainda pode concluir até expirar; nenhum orçamento é renovado silenciosamente.

CHECKs `ck_outbox_events_retry_history`, `ck_outbox_events_diagnostic` e `ck_outbox_events_terminal` complementam ownership/processed existentes. INTEGER/BIGINT limitam a representação. Código/horário/evidência ficam todos ausentes ou presentes; `IS TRUE` rejeita JSON null, evitando que resultado SQL NULL passe no CHECK. Novas decisões terminais exigem motivo/diagnóstico e horários correspondentes, inclusive evidência numérica de esgotamento quando aplicável. Terminal não tem dono nem processed_at. Constraints validam formato, não autorização nem histórico completo de transições.

Sem índice novo: transições exatas usam PK; aquisição usa índice pending/available existente com predicados residuais de ciclo/capacidade/revisão. Manutenção atua em um ID observado, sem índice especulativo de scanner.

Down remove somente nove colunas e seus CHECKs, preservando envelope, contador vitalício, status, last_error e disponibilidade. Perde informações de ciclo/revisão/diagnóstico/auditoria terminal. Foi testado somente em fixtures isoladas; não é estratégia de recuperação nem rollback compartilhado autorizado.

## Modelos, portas e autoridade

[Modelos de domínio](../../../backend/src/domain/retry.rs) validam histórico, delay e evidência; [portas focadas](../../../backend/src/persistence/retry.rs) usam futures Send nativos e dispatch estático; [adapter PostgreSQL](../../../backend/src/infrastructure/postgres/retry.rs) concentra locks/transações/driver. Sem CRUD genérico, replay ou dependência nova.

`RetryState` valida ciclo/baseline/máximo/revisão contra contador vitalício; consumo usa subtração verificada, revisão usa adição verificada. `OwnedOutboxEvent::retry_state()` fornece metadados da aquisição confirmada. Construtor anterior permanece disponível para chamadas sintéticas; aquisição produtiva anexa estado persistido validado via `with_retry_state`. Reader pending continua observação, não autoridade nem visão bruta completa para manutenção.

`RetryDelay` aceita microssegundos inteiros positivos até um dia; Duration submicrossegundo é recusado. Adição verificada rejeita overflow Chrono e limite finito inferior PostgreSQL; limite superior Chrono fica abaixo do PostgreSQL. Produção usa somente relógio do banco pós-lock; timestamp do caller não autoriza transição. Jitter e configuração padrão 1 s/300 s pertencem à aplicação.

`FailureEvidence` aceita apenas policy_version fixo adr008-v1, fase tipada, validator_version opcional fixo envelope-v1 e evidence_reference opcional UUID não nulo. Não aceita strings livres, payload, credenciais, connection string ou cadeia de fontes do driver. Evidência registra diagnóstico autorizado, não prova ausência de aceite/efeito consumidor. Linha bruta preservada é evidência restrita. Horário/motivo terminal e histórico retido de ciclo/contador preservam decisão sem tabela separada; auditoria durável de request de replay fica em 3.5.

`RetryOwner` combina ID/token com ciclo/revisão esperados. `ScheduleRetry`/`PreserveFailure` exigem autoridade pending ativa. Resultado `RetryTransition::Applied(valor)` ou `Conflict`, separado de erro de infraestrutura. ID inexistente/incorreto, token/ciclo/revisão antigos, status terminal ou lease inválido conflitam. Repetir após sucesso conflita; zero linhas nunca significa sucesso idempotente. Token protege escrita no banco, não envio RabbitMQ.

Toda mutação relevante avança revisão uma vez: aquisição, conclusão, release comum, agenda e preservação terminal. Aquisição retorna nova revisão apenas após COMMIT reconhecido. Conclusão/release mantêm contrato ID/token e OwnershipLost; com lock, token identifica a aquisição atual, sem obrigar callers existentes a usar request novo. Dono final pode concluir/release sem renovar orçamento nem apagar diagnóstico. Escritores externos devem cumprir protocolo de revisão; SQL direto sem auditoria não é recuperação concorrente suportada.

## Transições exatas

Cada operação usa transação curta Read Committed. Operações possuídas bloqueiam ID sem predicado temporal, depois consultam clock_timestamp() em statement separado e exigem acquired_at <= agora < expires_at. Horários gravados/predicados usam essa amostra. Nenhum I/O com broker dentro da transação.

- `schedule_retry`: valida ownership/ciclo/revisão e orçamento restante de ciclo **e capacidade**; grava available_at=agora_banco+delay, diagnóstico/evidência/revisão; limpa lease atomicamente; preserva pending, contador, ciclo e envelope. Aquisição final não agenda tentativa adicional. Caller só pode usar porta após falha contextual definitiva; aceite incerto não autoriza agenda.
- `preserve_failure`: grava decisão autorizada, terminal_at=diagnostic_at=agora_banco, dead_letter/revisão; limpa dono, mantém processed_at NULL e histórico/envelope. Motivos de orçamento/capacidade exigem esgotamento durável correspondente. Categoria de driver/publisher não determina permanência automaticamente.
- Aquisição: pending vencido, lease ausente/expirado, contador < i32::MAX, contador-baseline < máximo e revisão < i64::MAX. Seleciona uma linha FOR UPDATE SKIP LOCKED, restaura envelope, amostra tempo pós-lock e revalida antes de incremento verificado/novo lease/revisão. Contador 9 permite slot 10; contador 10 não permite slot 11. None não prova ausência de backlog.
- `preserve_exhausted`: um ID exato, ciclo/revisão esperados, FOR UPDATE SKIP LOCKED, sem reconstruir OwnedOutboxEvent. Revalida pending vencido e lease ausente/expirado pós-lock; motivo storage_counter_exhausted se contador=i32::MAX, senão acquisition_budget_exhausted se consumo >= máximo. Não adquire/envia, repara envelope ou sobrescreve dono ativo. Linha bloqueada conflita; caller observa novamente, sem presumir fairness. Dois mantenedores não aplicam a mesma revisão.

Manutenção exige observação independente de ID/ciclo/revisão, sem scanner/lista, batch configurável ou polling. Pode preservar envelope inválido **somente porque metadados demonstram esgotamento**, não por validação poison. Scanner/diagnóstico geral fica em 3.4. Ownership inválido exige reparo separado autorizado, nunca autoridade inventada.

Overflow de revisão resulta OperationFailed sem alteração parcial; aquisição exclui revisões não incrementáveis. Casos excepcionais exigem desenho explícito de reparo/ampliação, não wrap/reset. Capacidade vitalícia esgotada não é corrigida por replay comum.

## Falhas e cancelamento

| Fronteira | Resultado e significado durável |
| --- | --- |
| Validação/aritmética/statement antes do COMMIT | OperationFailed/InvalidStoredData ou erro classificado do driver; sem alteração parcial/retry oculto |
| Rejeição ordinária explícita do COMMIT pelo servidor | Falha conhecida pelo mapping existente; transação rejeitada |
| Reconhecimento perdido, erro de conexão/completion-unknown/shutdown no COMMIT | CommitUncertain; agenda/terminal podem já estar persistidos |
| Conflito de autoridade/ciclo/revisão/elegibilidade | Conflict, nenhuma mutação; repetição não é sucesso |
| Future cancelado/descartado | Pode suprimir resultado durável; descarte não comprova rollback imediato |

Após efeito incerto, observar estado exato independentemente; não existe reconciliação genérica nova. Agenda incerta pode já ter escolhido horário e liberado dono; recomputar/repetir cegamente pode sobrescrever histórico ou duplicar envio. Terminalização incerta pode já ter isolado a linha. Conclusão falha/incerta continua sem autorizar release compensatório nem republicação cega.

## Rollout, testes e próximos marcos

Nenhuma migration compartilhada ou inicialização do relay compartilhado foi executada. Docker/PostgreSQL/broker existentes foram reutilizados, sem reset/alteração de credenciais como efeito de 3.2. A tarefa explícita anterior de senha é separada. Preflight somente leitura do binário agora exige colunas de retry antes de publisher/polling. Diretórios da configuração fonte Supervisor foram corrigidos para `/app/backend`; validação não exigiu restart/rebuild compartilhado.

Antes de rollout separadamente autorizado: parar admissão/escritores antigos; inspecionar histórico/mounts; contar pending esgotados incluindo leases ativos/expirados; revisar diagnóstico legado/acesso bruto; aplicar migrations em ordem e implantar binários/escritores compatíveis. Registrar que baseline zero não concede orçamento novo. Down/reset não são recuperação. Sem ativação automática ou alegação de prontidão produtiva.

[Testes SQL](../../../backend/tests/postgres_retry.rs) usam somente bancos UUID do harness, com limpeza exata: backfill/down, CHECKs/NULL/overflow/evidência, relógio pós-lock/expiração, autoridade errada/antiga, repetição, limite exato/concorrência, conclusão final, manutenção/dono ativo/contenção/envelope inválido preservado, capacidade/revisão, rejeição statement/COMMIT deferido, reconhecimento de COMMIT perdido com observação independente, cancelamento no lock e após COMMIT do servidor com reconhecimento retido. [Testes puros](../../../backend/tests/retry_state.rs) cobrem entradas/precisão/limites temporais. Regressões de ownership/repositório/schema e recuperação integrada permanecem distintas. Veja [execuções registradas](../testing/validation-results.md).

```bash
docker compose exec -T app cargo test --locked --test postgres_retry -- --ignored --test-threads=4
```

3.3 integra decisões contextuais definitivas, orçamento persistido e jitter explicitamente não enviesado em deliver_once/worker, preservando leases incertos e semântica de conclusão; deve definir admissão limitada de manutenção pela porta de linha exata. 3.4 adiciona diagnóstico/scanner geral de poison; 3.5 replay autenticado/auditado com identidade original; 3.6 aceitação integrada/fechamento. Frontend permanece ilustrativo e inalterado; 3.2 não comprova pipeline de retry implantado.

Fontes primárias: [CHECK PostgreSQL](https://www.postgresql.org/docs/18/ddl-constraints.html), [amostragem temporal](https://www.postgresql.org/docs/18/functions-datetime.html), [limites de timestamp](https://www.postgresql.org/docs/18/datatype-datetime.html).
