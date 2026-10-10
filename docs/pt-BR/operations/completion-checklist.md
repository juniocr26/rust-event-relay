# rust-event-relay: Checklist de conclusão do repositório

Revisão estática em 2026-10-10; sem execução runtime. Itens marcados indicam documentação concluída, não funcionalidades entregues ou lacunas encerradas.

- [x] Inventário: documentação e código/configuração/testes inspecionados; inventário original no manifest da biblioteca.
- [x] Reestruturação: categorias equivalentes por idioma; IDs/histórico ADR preservados; arquivos exigidos por ferramentas mantidos.
- [x] Revisão de conteúdo: mecanismos, contratos, alternativas, falhas e limites de evidência explicados.
- [x] Cobertura bilíngue: páginas mantidas equivalentes en/pt-BR; históricos identificados explicitamente.
- [x] Navegação: README e catálogo por idioma alcançam cada documento mantido.
- [x] Cobertura Engineering Library: explicações completas e respostas substanciais preservadas/ampliadas.
- [x] Validação de links/anchors/numeração: verificação final sem erros; 18 warnings de links restritos de fixtures registrados no relatório da biblioteca.

## Evidência inspecionada

- [src/application.rs](../../../src/application.rs)
- [src/config.rs](../../../src/config.rs)
- [src/telemetry.rs](../../../src/telemetry.rs)
- [src/persistence/ownership.rs](../../../src/persistence/ownership.rs)
- [src/infrastructure/postgres/mod.rs](../../../src/infrastructure/postgres/mod.rs)
- [src/infrastructure/rabbitmq.rs](../../../src/infrastructure/rabbitmq.rs)
- [migrations/20261009000000_add_delivery_ownership.up.sql](../../../migrations/20261009000000_add_delivery_ownership.up.sql)
- [compose.yaml](../../../compose.yaml)
- [tests/delivery_ownership.rs](../../../tests/delivery_ownership.rs)

## Cobertura de entrevista e contraparte na biblioteca

Liveness HTTP; envelope/UUID/JSON/timestamps; transação outbox/snapshots; lease/autoridade/expiração; confirms/incerteza; dispatch estático, testes isolados e recuperação.

[Self-contained dossier / Dossiê](../../../../engineering-library/docs/pt-BR/architecture/rust-event-relay.md) | [Interview / Entrevista](../../../../engineering-library/docs/pt-BR/interviews/rust-event-relay.md)

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
