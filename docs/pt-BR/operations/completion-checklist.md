**Estado local atual — Marco 4.5 (2026-10-16):** [Rollout verificado e recuperação](milestone-4-5-rollout.md). Seis migrations aplicadas; artefatos atuais HTTP/relay/consumer instalados e ativos; consumer autostart=true. Backup protegido/restore isolado e demonstração real 10 → 15 → 15 verificados. API replay desabilitada; frontend continua simulação. Observações anteriores abaixo preservam suas datas.

Contrato operacional atual (17/10/2026): [5.2 prontidão/atualidade e rollout validado 5.1](milestone-5-2.md). Entradas anteriores abaixo preservam seu escopo histórico.

## Marco 4.4 — verificação e fechamento atuais — 16/10/2026

Marco 4 fechado com limitações não bloqueantes explícitas. Evidência real de estoque/deduplicação, ACK/quarentena e recuperação é separada do rollout compartilhado pendente. Frontend de apresentação verificado; integração ao vivo com backend não implementada. Seções históricas abaixo mantêm o escopo das respectivas datas. [Matriz de verificação](../operations/milestone-4-4-review.md) | [Inventário de ambiente](../docker/environment-inventory.md).

#
Worker atual, configuração, cancelamento e rollout autorizado: [Marco 2.5](../architecture/milestone-2-5.md). Seções anteriores mantêm escopo histórico.

 rust-event-relay: Checklist de conclusão do repositório

## Marco 2.4 — 2026-10-12

O caso de uso de entrega executa aquisição reconhecida → publicação do envelope original → completion somente após aceitação confirmada. Falhas definitivas liberam uma vez; incerteza mantém lease. Resultados preservam falha principal e cleanup separadamente. HTTP continua independente; não há worker composto. Consulte [Marco 2.4](../architecture/milestone-2-4.md) para resultados, cancelamento e limites de recuperação. Próximo: 2.5 composição/operação do worker; 2.6 experimentos ponta a ponta.

Seções anteriores preservam o escopo do marco/revisão registrado.

Revisão estática em 2026-10-10; sem execução runtime. Itens marcados indicam documentação concluída, não funcionalidades entregues ou lacunas encerradas.

- [x] Inventário: documentação e código/configuração/testes inspecionados; inventário original no manifest da biblioteca.
- [x] Reestruturação: categorias equivalentes por idioma; IDs/histórico ADR preservados; arquivos exigidos por ferramentas mantidos.
- [x] Revisão de conteúdo: mecanismos, contratos, alternativas, falhas e limites de evidência explicados.
- [x] Cobertura bilíngue: páginas mantidas equivalentes en/pt-BR; históricos identificados explicitamente.
- [x] Navegação: README e catálogo por idioma alcançam cada documento mantido.
- [x] Cobertura Engineering Library: explicações completas e respostas substanciais preservadas/ampliadas.
- [x] Validação de links/anchors/numeração: verificação final sem erros; 18 warnings de links restritos de fixtures registrados no relatório da biblioteca.

## Evidência inspecionada

- [src/application.rs](../../../backend/src/application.rs)
- [src/config.rs](../../../backend/src/config.rs)
- [src/telemetry.rs](../../../backend/src/telemetry.rs)
- [src/persistence/ownership.rs](../../../backend/src/persistence/ownership.rs)
- [src/infrastructure/postgres/mod.rs](../../../backend/src/infrastructure/postgres/mod.rs)
- [src/infrastructure/rabbitmq.rs](../../../backend/src/infrastructure/rabbitmq.rs)
- [migrations/20261009000000_add_delivery_ownership.up.sql](../../../backend/migrations/20261009000000_add_delivery_ownership.up.sql)
- [compose.yaml](../../../compose.yaml)
- [tests/delivery_ownership.rs](../../../backend/tests/delivery_ownership.rs)

## Cobertura de entrevista e contraparte na biblioteca

Liveness HTTP; envelope/UUID/JSON/timestamps; transação outbox/snapshots; lease/autoridade/expiração; confirms/incerteza; dispatch estático, testes isolados e recuperação.

Self-contained dossier / Dossiê (`engineering-library/docs/pt-BR/architecture/rust-event-relay.md`, external repository) | Interview / Entrevista (`engineering-library/docs/pt-BR/interviews/rust-event-relay.md`, external repository)

## Aplicabilidade das categorias

| Categoria | Tratamento / justificativa |
| --- | --- |
| payments | Não aplicável: pagamentos ausentes. No Payment Lab, conciliação/Stripe são apenas direção futura. |
| api | Apenas GET /health fixo; contrato em README/architecture. |
| deployment | Supervisor/migrações/atualização/rollback em operations/docker. |
| observability | Tracing JSON/liveness, sem readiness ou métricas, em operations/security. |
| benchmarks | Não aplicável: sem medições de desempenho verificadas adequadas a gráficos; nenhuma executada. |

Categorias presentes no índice contêm conteúdo mantido; categorias tratadas em outros locais não recebem pastas vazias. ADR/comparações conservam histórico; nenhuma motivação ou data histórica nova foi inventada.

## Lacunas restantes dependentes de evidência

SQL/concorrência de ownership, worker, efeitos produtor/consumidor, linhas inválidas, retry/recuperação, TLS/HA e desempenho.


## Marco 2.6 — Escopo atual

[Experimentos integrados](../architecture/milestone-2-6.md) e [revisão do Marco 2](../operations/milestone-2-review.md) atualizam a aceitação sob premissas explícitas. Rollout compartilhado continua pendente; registros anteriores mantêm seu contexto histórico.


## Política do Marco 3.1

Marco 3.1 define matriz de falhas, contratos de agenda/orçamento/isolamento/replay e próximos marcos. Somente cálculos puros foram implementados; persistência futura e cenários frontend continuam planejados. [Policy](../architecture/milestone-3-1.md) | [ADR 008](../adr/008-durable-failure-policy.md).

## Marco 3.3 — integração atual

Integração de entrega/worker implementada; seções anteriores mantêm seu escopo histórico datado. [Contratos atuais, jitter, matriz contextual, manutenção e cancelamento](../architecture/milestone-3-3.md). Scanning geral fica em 3.4, replay auditado em 3.5 e aceitação completa em 3.6; rollout compartilhado permanece pendente.

## Rollout local autorizado — 2026-10-13

[Operação local atual](local-relay-rollout.md): http/relay com autostart, migrations ownership/retry instaladas explicitamente, topologia autenticada e controles manuais/interativos verificados. Descrições pendentes/paradas anteriores permanecem históricas. Sem alegação de prontidão produtiva ou conclusão de 3.4–3.6.


## Marco 3.4 — descoberta/isolamento atuais

Descoberta limitada e isolamento com autoridade atual implementados; replay permanece 3.5 e fechamento integrado 3.6. Rollout local preservado; esta alteração não tem migration ou reinício compartilhado. [Marco 3.4 — descoberta/isolamento atuais](../architecture/milestone-3-4.md).


## Aceitação atual do Marco 3.6 — 13/10/2026

[Auditoria completa, matriz rastreável, contagens executadas, lacunas operacionais e leitura guiada](milestone-3-review.md). Marco 3 fechado com limitações explícitas não bloqueantes; rollout replay compartilhado, idempotência consumer, frontend/backend real e prontidão produtiva permanecem separados. Seções anteriores preservam o escopo histórico datado.


## Marco 4.1 — contratos implementados; runtime planejado

[Estoque, deduplicação, quarentena e 4.2–4.4](../architecture/milestone-4-1.md) | [ADR 010](../adr/010-atomic-consumer-and-quarantine.md).
