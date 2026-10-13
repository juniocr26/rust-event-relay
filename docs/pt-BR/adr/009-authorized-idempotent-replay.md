[Other language](../../en/adr/009-authorized-idempotent-replay.md)

# ADR 009 — Replay autorizado, idempotente e auditável

Aceita em 13/10/2026, Marco 3.5. Base Rust `3df5942f1dfb944fc294dcbc0d27d2f0daeedd16`; Engineering Library `fd61b2123aa7a6b59679c603f70bbd7d459c5c2a`. Ambos estavam limpos. Foram consultados ADRs 007/008, marcos 3.1–3.4, contratos/modelos, PostgreSQL, worker, HTTP, migrations e harnesses isolados de falha.

## Fronteira de confiança

API HTTP administrativa opt-in com bearer, sem plataforma de identidade nem CLI baseada apenas em string de ator/conexão ao banco. Configuração confiável associa credencial aleatória de 256 bits em hexadecimal ao ator estável, allow_replay e teto de orçamento 1–100. O cliente não informa ator/admin no JSON. Configuração vazia/ausente desabilita rotas. Credencial válida sem permissão não pode executar, repetir nem consultar replay. Escopo global outbox:replay, sem tenancy; operador autorizado pode atuar sobre qualquer evento elegível, mas consulta apenas recibos do próprio ator/escopo. Proteger configuração/rede e usar credenciais individuais; revogação exige remoção da configuração e ativação explícita. Não há plataforma completa de rotação/identidade.

Toda entrada do caso de uso verifica autorização antes de persistência; consulta filtra ator/escopo no SQL. Request ID adivinhado não revela recibo alheio. Colisão com request de outro ator retorna conflito genérico, sem conteúdo de auditoria. Modelos/ports com futures Send nativos independem de HTTP/SQLx; transporte autentica, aplicação valida/autoriza, infraestrutura bloqueia/persiste. Sem login/botão frontend.

## Contrato e elegibilidade

POST /operator/replays exige UUIDs não nulos de request/evento, revisão esperada >=0, ciclo >0, motivo ASCII imprimível obrigatório de até 256 bytes/não branco, orçamento 1–100 dentro do teto autorizado, delay 0–86400000 ms e ticket UUID opcional não nulo. Campos desconhecidos rejeitados, corpo JSON até 4096 bytes. Associação canônica: campos JSON explícitos mais ator/escopo, comparação estrutural JSONB; strings exatas, ticket ausente/null equivalentes. UUID de request global, não por ator/evento.

Somente dead_letter preservado com razões legacy_dead_letter, serialization_permanent, invalid_stored_envelope, acquisition_budget_exhausted ou storage_counter_exhausted e validação atual bem-sucedida. Rejeitar ausente/pending/processed/revisão ou ciclo obsoletos. Restaurar envelope pelo contrato atual de reader/acquirer, representação JSONB/timestamps suportada, histórico coerente, ausência da tupla inteira de ownership/processed_at e consistência de evidência/diagnóstico/razão terminal. Versão positiva u32 é metadado do envelope, não catálogo de payload de negócio. Conteúdo inválido/representação não suportada não sofre reparo. Evento isolado só pode voltar após reparo autorizado separadamente ou interpretação suportada que permita validação. Replay não realiza esse reparo; identidade/histórico/ownership inseguros exigem protocolo próprio auditado.

Novo ciclo incrementa uma vez; baseline recebe contador vitalício atual; orçamento escolhido é persistido; disponibilidade usa clock_timestamp PostgreSQL após lock mais delay. Revisão avança com capacidade verificada. Preservar ID, timestamps originais/criação, versão, agregado, correlation/causation e JSONB exato; nunca reescrever envelope nem resetar attempt_count. Replay não cria/reutiliza token; aquisição normal seguinte usa token novo. Limpar campos terminais/diagnósticos/last_error para pending após preservar valores anteriores na auditoria restrita.

Contador vitalício i32::MAX impede aquisição futura representável e rejeita replay; ciclo i32::MAX também. Revisão exige espaço para replay, uma aquisição e conclusão (<= i64::MAX−3). Orçamento limita aquisições, sem prometer que todas cabem na capacidade restante; aquisição continua verificando storage. Ampliação numérica requer política/migration separada, sem reset/wrap/saturação.

## Atomicidade e resultados

Transação curta Read Committed e classificação existente. Ordem: advisory lock transacional hashtextextended do UUID request com seed 3500; ler request durável; bloquear evento exato FOR UPDATE. Colisão de hash apenas serializa requests distintos; PK UUID garante identidade. Uma operação bloqueia um request e um evento; manutenção/ownership não obtêm locks de replay. Não inverter ordem nem introduzir lote multi-evento. Sem I/O do broker/retry transacional oculto.

Após espera, revalidar linha atual e amostrar relógio em statement separado depois do lock do evento. Ator/escopo/intenção idênticos retornam recibo anterior, mesmo com evento já avançado. Intenção alterada retorna IntentConflict. Requests distintos para mesma revisão disputam lock do evento e só um vence. Descoberta/manutenção revalidam pending e revisão; snapshot terminal antigo não autoriza mutação de evento reativado. Worker pode adquirir logo após commit, antes da resposta HTTP.

Migration nova 20261013000000 grava recibo, intenção explícita, ator/motivo e evidência terminal anterior atomicamente com ciclo. decided_at é horário da decisão pós-lock, não instante exato de COMMIT; retornar recibo só após commit reconhecido. Auditoria preserva razão/horário, diagnóstico/evidência/last_error, ciclo/baseline/orçamento/revisão predecessores e contador vitalício. HTTP não devolve payload, motivo livre nem evidência anterior. Aplicação não possui UPDATE/DELETE; trigger rejeita UPDATE/DELETE/TRUNCATE, PUBLIC sem acesso. Papel produtivo precisa SELECT/INSERT em auditoria e privilégios necessários de evento, sem DDL/owner. Owner/superuser local pode desabilitar trigger/excluir objetos: imutável para aplicação, não à prova de adulteração. Sem retenção automática.

Rejeições não geram auditoria transacional: diagnóstico operacional sanitizado. Falha anterior à autenticação/transação não possui garantia de auditoria durável. Rejeição conhecida do servidor, inclusive deferred no COMMIT, desfaz ambos os writes. Perda de reconhecimento é CommitUncertain. Cancelamento pode ocultar commit; Drop SQLx 0.8.6 enfileira rollback, sem provar rollback imediato nem desfazer commit. GET /operator/replays/{request_id} observa recibo com autenticação/autorização. Resultado negativo é visibilidade atual, não prova que transação em andamento não comitará. Repetição exata autorizada coordena request lock/recibo; nunca gerar automaticamente novo UUID após incerteza.

Recibo significa trabalho reabilitado, não aceitação RabbitMQ/efeito de consumidor. Replay não publica imediatamente, desfaz efeitos anteriores nem garante novo efeito; idempotência de consumidor é independente.

## Referências

[OWASP autorização](https://cheatsheetseries.owasp.org/cheatsheets/Authorization_Cheat_Sheet.html), [isolamento PostgreSQL 18](https://www.postgresql.org/docs/18/transaction-iso.html), [confiabilidade RabbitMQ](https://www.rabbitmq.com/docs/reliability). Comportamento versionado de Drop SQLx e limite JSON Axum 0.8.9/core 0.5.6 inspecionado no cache de fontes; comparação em tempo constante de comprimentos iguais usa subtle 2.6.1. [Procedimento operacional](../operations/replay.md) e [implementação/aceitação](../architecture/milestone-3-5.md).
