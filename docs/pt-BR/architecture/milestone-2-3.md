[English](../../en/architecture/milestone-2-3.md) | [README](../../../README.pt-BR.md)

# Marco 2.3 — Operações PostgreSQL de posse da entrega

Implementado em 2026-10-12 a partir de `bbc2c1f` limpo. [ADR 007](../adr/007-durable-delivery-ownership.md) continua autoritativo; modelos, ciclo de vida e assinaturas públicas preservados. Consulte [validação executada](../testing/validation-results.md#marco-23--2026-10-12) e [registro de validação 2.3](../testing/validation-results.md#marco-23--2026-10-12).

## Escopo e contratos implementados

`PostgresOutboxRepository` implementa `OutboxAcquirer`, `OutboxCompleter` e `OutboxReleaser`, além da observação preservada. SQL/transações/classificação ficam em [postgres/ownership.rs](../../../src/infrastructure/postgres/ownership.rs); decoding verificado do envelope é compartilhado com o reader em [postgres/mod.rs](../../../src/infrastructure/postgres/mod.rs). Domínio/persistência independem de SQLx/Lapin; futures Send nativas e despacho estático continuam. Nenhuma dependência ou migração SQL mudou. Ciclo de vida do pool e prazos produtivos pertencem ao chamador.

O contrato real de 2.2 adquire **no máximo um** evento, com pedido consumido contendo token novo e duração positiva em microssegundos inteiros. Não recebe cutoff ou limite de lote. `EligibleRead`/`BatchSize` pertencem à observação; nenhuma API de lote foi acrescentada para atender contrato presumido. Release recebe apenas `OwnedEventKey`, preservando disponibilidade e last_error, sem diagnóstico recebido ou cálculo de backoff. São decisões existentes, não campos adicionais parcialmente implementados.

| Operação | Predicado sobre linha bloqueada no instante de decisão | Mutação confirmada |
| --- | --- | --- |
| Acquire | pending, disponibilidade vencida, sem lease ou lease expirado, contador abaixo de i32::MAX | Token do pedido, acquired_at, expires_at, contador + 1; mantém pending |
| Complete | ID/token corretos, pending, acquired_at <= tempo < expires_at | processed, processed_at=tempo, três campos de posse NULL |
| Release | Mesmo predicado de autoridade | Três campos de posse NULL; pending, disponibilidade, contador e diagnóstico preservados |

Completion persiste a decisão de publicação confirmada do chamador. Não chama RabbitMQ nem comprova aceitação por conta própria; processed não significa processamento do consumidor. Eventos ausentes, terminais, sem posse, com token incorreto, expirados e operações repetidas retornam `OwnershipLost`, sem sucesso presumido ou exceção de infraestrutura. Envelope, created_at e metadados não relacionados são preservados. Contador mede aquisições confirmadas por commit, inclusive posteriormente expiradas, não envios; commit incerto também pode ter incrementado o valor duravelmente.

## Transações, locks e relógios

Cada mutação escolhe Read Committed explicitamente em transação SQLx curta. Aquisição consulta `clock_timestamp()` como **cutoff de seleção** e busca primeiro candidato por disponibilidade/created_at/ID com `LIMIT 1 FOR UPDATE SKIP LOCKED`. Disponibilidade/expiração inclusivas e limite estrito do contador filtram candidatos. Linhas bloqueadas podem ser ignoradas; `None` significa que esta chamada não adquiriu trabalho, sem provar backlog vazio ou justiça de ordenação. Lease vencendo depois do cutoff pode ficar para outra chamada.

Após obter lock, aquisição restaura envelope por decoding verificado e consulta outro instante autoritativo do banco. Revalida estado, disponibilidade, expiração e contador na linha bloqueada: esse é o instante de decisão da elegibilidade e dos timestamps de posse. Soma de duração e incremento são verificados antes de mutar. Contadores esgotados permanecem pending/observáveis e excluídos da aquisição, sem overflow, saturação ou política terminal automática. Dado selecionado inválido falha toda a chamada de uma linha antes de mutar; não se ignora silenciosamente em favor de outro candidato. Candidato inválido persistente pode impedir progresso.

Completion/release fazem `FOR UPDATE` apenas por ID, permitindo espera. Só depois consultam o relógio em outra instrução e executam UPDATE condicional com o mesmo instante. Lock permanece até mutação/commit; outra aquisição não substitui autoridade nesse intervalo. Igualdade com expires_at rejeita; relógio anterior a acquired_at também. `now()`/tempo do início da transação não autoriza após esperar lock. Não se garante relógio monotônico. Consulte [funções de tempo PostgreSQL](https://www.postgresql.org/docs/18/functions-datetime.html), [locks/SKIP LOCKED](https://www.postgresql.org/docs/18/sql-select.html) e [Read Committed](https://www.postgresql.org/docs/18/transaction-iso.html).

Validade é decidida pelo instante pós-lock, não pelo commit/retorno. Posse fica durável no commit; latência de confirmação e resposta consome lease, podendo retornar posse já expirada. Nenhum lock/transação/conexão retirada do pool escapa para publicação externa. Transações curtas evitam espera remota do broker, mas não impõem timeout produtivo nem demonstram baixa latência sob DDL/contenção. Não há benchmark, garantia de justiça ou custo do plano medido.

## Falhas, commit incerto e cancelamento

Antes de tentar COMMIT, erros usam classificação existente Unavailable/InvalidStoredData/OperationFailed. Transação SQLx aberta descartada enfileira rollback; limpeza eventual não é confirmação imediata de rollback. Nenhuma falha retorna claim bem-sucedido ou causa retry oculto.

No COMMIT, rejeição ordinária reportada pelo servidor usa mapeamento existente: constraint diferida rejeitada, por exemplo, é OperationFailed e faz rollback. Erros de transporte/protocolo/worker, código ausente, classe SQLSTATE 08, 40003 e shutdown/unready 57P01/02/03 são conservadoramente CommitUncertain. Classificação é específica da fronteira de commit, sem transformar todo erro de consulta em incerteza. Código ausente não comprova rollback. Fontes tipadas continuam acessíveis; formatação pública é sanitizada e métodos não emitem logs. Consulte [códigos PostgreSQL](https://www.postgresql.org/docs/18/errcodes-appendix.html).

Aquisição incerta retorna erro, nunca dados de posse ordinários. Não publicar às cegas nem repetir token antigo: inspecionar lease durável operacionalmente ou permitir expiração e novo pedido. Após completion incerto, processed pode estar persistido; presumir falha ou republicar cegamente não se justifica. Release incerto também não prova posse retida. Chamador/operador reconciliam; orquestração fica para 2.4, sem inventar novo port de reconciliação.

Descartar future antes de COMMIT não estabelece rollback imediato. Durante COMMIT, cancelamento pode suprimir resultado enquanto banco confirma; future cancelada nem sempre retorna CommitUncertain. Rollback enfileirado pelo SQLx não desfaz commit já concluído. Teste de cancelamento cobre apenas transição bloqueada antes de mutação, não cancelamento durante commit ou morte abrupta. Tokens protegem transições do banco, **não publicação RabbitMQ**: worker antigo ainda pode enviar, e aceitação seguida de completion falho/incerto pode duplicar na recuperação.

## Evidência e trabalho restante

[postgres_ownership.rs](../../../tests/postgres_ownership.rs) tem 12 casos PostgreSQL reais isolados: elegibilidade/limite de uma linha, token/tempo/contador, exclusão de lease ativo e observação preservada; transições válidas/repetidas/token incorreto/ausência/terminal; expiração/recuperação/tokens antigos; aquisição concorrente disjunta; SKIP LOCKED; dado inválido/overflow sem mutação; contador máximo; rejeição antes de acquired_at; retorno expirado; rejeições imediata/diferida; cancelamento durante lock wait.

Teste de espera observa estado Lock em pg_stat_activity, verifica início transacional anterior à expiração, aguarda relógio do banco alcançar expiração e só então libera lock. Exercita completion e release; snapshots de atividade são atualizados explicitamente, com polling limitado para coordenar, sem presumir expiração por sleep fixo. Igualdade exata da expiração é demonstrada pelos testes determinísticos existentes do modelo; integração demonstra expiração ao redor de esperas reais, sem igualdade controlada. Relógio do banco real não foi alterado.

Proxy de conexão PostgreSQL de teste encaminha frames e descarta resposta CommandComplete COMMIT do servidor. Em aquisição, completion e release, adapter retorna CommitUncertain enquanto conexão independente observa estado confirmado. Aquisição incerta não devolve evento com posse ordinário; lease durável impede reaquisicão imediata e fixture expirada recupera com token/contador novos. Repetição de completion/release após transição incerta confirmada retorna OwnershipLost. Experimento perde resposta **depois** de observar conclusão no servidor: prova que resultado incerto confirmado é possível, não que toda incerteza implica commit ou rollback. Proxy usa apenas banco/conexão isolados, com conexão de teste explicitamente sem TLS; sem reiniciar serviços ou interceptar tráfego compartilhado.

Cancelamento durante commit, perda de processo/rede antes de observar conclusão, aquisição após espera DDL/reversão de relógio, todos os schedules de escrita/isolamento, desempenho produtivo e perda de energia continuam não verificados. Fixture anterior a acquired_at testa limite inferior de autoridade, não reversão real do relógio.

Testes aplicam migração apenas em bancos de fixture com nomes exatos e verificam limpeza. Metadados de desenvolvimento consultados sem escrita em 2026-10-12 mostram duas migrações originais instaladas e ausência de ownership_token; migração de posse permanece pendente. HTTP/runtime/serviços/dados preservados. CI inclui nova suíte; execução remota não alegada.

**Próximo: 2.4, orquestração do use case de entrega** — uma aquisição → publicação por EventPublisher → completion apenas na aceitação confirmada; demais resultados exigem release/reconciliação explicitamente definidos. Reutilizar ports, ID estável e categorias do publisher; decidir como expor perda de posse, persistência incerta e cancelamento sem retry cego. Polling, registro/reconexão de worker no Supervisor, política retry/backoff/DLQ, produtores, idempotência do consumidor e experimentos ponta a ponta continuam fora desse escopo.
