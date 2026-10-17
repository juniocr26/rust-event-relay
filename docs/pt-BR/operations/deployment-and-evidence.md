[Registro histórico: instalação e endereços atuais](../operations/application-scenarios.md)

**Deploy/evidência atual — 17/10/2026:** [5.3](milestone-5-3.md) registra CI remoto exato 5.2, rollout nominal controlado, hashes carregados, medições isoladas finitas e demonstração preservada. 5.3 é trabalho local de testes/docs/comentário, sem deploy. Registros anteriores mantêm seu escopo original.

## Marco 4.4 — verificação e fechamento atuais — 16/10/2026

Marco 4 fechado com limitações não bloqueantes explícitas. Evidência real de estoque/deduplicação, ACK/quarentena e recuperação é separada do rollout compartilhado pendente. Frontend de apresentação verificado; integração ao vivo com backend não implementada. Seções históricas abaixo mantêm o escopo das respectivas datas. [Matriz de verificação](../operations/milestone-4-4-review.md) | [Inventário de ambiente](../docker/environment-inventory.md).

#
Worker atual, configuração, cancelamento e rollout autorizado: [Marco 2.5](../architecture/milestone-2-5.md). Seções anteriores mantêm escopo histórico.

 Implantação, observabilidade e lacunas de evidência

[English](../../en/operations/deployment-and-evidence.md) | [Português brasileiro](deployment-and-evidence.md)

Revisão estática do código: 2026-10-10. Fatos implementados, teoria geral e mudanças hipotéticas são separados abaixo. Comandos runtime não foram executados.

## Atualização — 2026-10-12

[Marco 2.3](../architecture/milestone-2-3.md) implementa transições condicionais com evidência real de contenção/expiração. Migração de posse continua pendente no desenvolvimento, confirmada por metadados sem escrita; apenas bancos isolados foram migrados. Não há fencing no broker, orquestração ponta a ponta, desempenho produtivo ou experimento de confirmação COMMIT perdida. Revisão abaixo é histórica (2026-10-10).

## Revisão histórica


Compose fornece PostgreSQL, RabbitMQ e app de desenvolvimento gerenciado pelo Supervisor. O HTTP depende de configuração/tracing/listener; não cria PgPool nem RabbitMqPublisher. Health das dependências pode condicionar startup, mas `/health` continua liveness do processo. Supervisor permite 30 s de drain HTTP e Compose concede 35 s ao encerramento do gerenciador. Mudanças exigem stop/build/start; não há reload automático ou worker implantado. Bind/volume preserva recriações comuns, não perda do host/armazenamento.

Migrações SQLx são versionadas e aplicadas explicitamente, não pelo startup HTTP. Preserve IDs e conteúdo aplicado. A migração de ownership tem fonte e evidência isolada; o registro anterior informa que continuava pendente no banco compartilhado de desenvolvimento. Esta revisão não consultou esse banco nem atualizou seu estado. Down remove colunas de ownership; downs anteriores podem destruir outbox. Down e reaplicar schema não restauram linhas. Rollout real exige compatibilidade e backup/restore verificados separadamente, não reset destrutivo.

Testes padrão validam domínio, mapeamento e contratos, além de HTTP/socket. Testes PostgreSQL opt-in criam bancos de fixture com nomes exatos e verificam limpeza; testes de broker exercitam roteamento e incerteza. Alguns testes independentes do banco usam sockets locais, portanto independência não significa ausência de I/O. Workflow CI é evidência de configuração, não prova de execução remota. Resultados históricos continuam em testing/validation-results. Nenhum build/teste Rust, comando Docker, migração, chamada ao broker ou benchmark foi executado nesta revisão.

Desempenho tem limites de linhas e uma publicação em andamento, mas número de linhas não limita bytes de payload. `fetch_all` materializa lote; dado selecionado inválido falha o resultado inteiro e pode bloquear leituras repetidas. Não há quarentena, política de backpressure, frequência polling, medição de vazão, endpoint de métricas ou dimensionamento produtivo de pool entregue. Lacunas: SQL/concorrência de ownership, orquestração worker, experimentos de duplicata/recuperação, política de linha inválida, TLS/HA produtivo, retenção/restore e desempenho. Nada disso já foi comprovado por liveness verde.


## Marco 2.6 — Escopo atual

[Experimentos integrados](../architecture/milestone-2-6.md) e [revisão do Marco 2](../operations/milestone-2-review.md) atualizam a aceitação sob premissas explícitas. Rollout compartilhado continua pendente; registros anteriores mantêm seu contexto histórico.

## Rollout local autorizado — 2026-10-13

[Operação local atual](local-relay-rollout.md): http/relay com autostart, migrations ownership/retry instaladas explicitamente, topologia autenticada e controles manuais/interativos verificados. Descrições pendentes/paradas anteriores permanecem históricas. Sem alegação de prontidão produtiva ou conclusão de 3.4–3.6.
