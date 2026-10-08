# Handoff do Marco 1

Reliable Event Relay | Modelo durável de eventos

**Data de geração:** 08/10/2026 (America/Sao_Paulo). **Base revisada:** `7cba38d`, commit do Marco 1.7. Documentação de fechamento, reforço dos testes e este PDF são mudanças não commitadas. Não há hash futuro nem execução remota de CI atribuída a esta versão.

## Situação de entrega

**Marco 1 fechado nos critérios 1.1 a 1.8, sem bloqueadores restantes. Marco 2 não iniciado.** A validação desta entrega foi executada novamente; os resultados históricos do 1.7 foram preservados separadamente.

Projeto aberto de estudo e portfólio sobre entrega distribuída de eventos sob falhas. Rust é a ferramenta; o problema é preservar significado, identidade e fronteiras de recuperação. A implementação atual oferece envelope validado, schema outbox, contrato de observação e adapter PostgreSQL. O binário inicia HTTP e health, sem conectar ao banco ou entregar eventos.

| Entrega | Resultado no fechamento |
| --- | --- |
| 1.1 Envelope canônico | Identidade, tempo, metadados e payload validados/restaurados |
| 1.2 PostgreSQL local | Docker, healthcheck, acesso loopback e bind mount |
| 1.3 Migrações | SQL versionado, CLI explícito e histórico aplicado |
| 1.4 Schema outbox | Constraints, estados representáveis e índice pending |
| 1.5 Abstração | OutboxReader, leitura limitada, snapshots e erros |
| 1.6 Repositório | SELECT parametrizado e decodificação privada verificada |
| 1.7 Integração | PostgreSQL real, isolamento, prazos e limpeza |
| 1.8 Semântica | Transações, falhas, limites de durabilidade e recuperação |

Este fechamento não implementa produtores, claims, leases, transições, retries ou destinos. Persistência local não é uma garantia de entrega funcional. Decisões futuras permanecem abertas e não são bloqueadores do escopo documental do 1.8.

<!-- pagebreak -->

## Arquitetura e responsabilidades

Fluxo HTTP atual: `main` carrega configuração e tracing, abre listener e chama o ciclo de vida em `application.rs`. Health responde `ok`; testes verificam socket real e sinais Unix. O caminho de persistência existe como biblioteca, sem ligação ao bootstrap HTTP.

Dependência de compilação: infraestrutura PostgreSQL implementa OutboxReader e depende de persistência/domínio; contratos e domínio não dependem de SQLx. O chamador constrói, injeta e encerra PgPool. Não há pool global ou ambiente lido pelo repositório.

| Responsabilidade | Fronteira aplicada |
| --- | --- |
| Controllers | Delegam a casos de uso quando existe orquestração |
| Casos de uso | Controlam aplicação/negócio; processamento ainda futuro |
| Repositórios | Executam consultas e implementam contratos focados |
| Adapters externos | Comunicação com APIs/mensageria; ainda não implementada |
| Services e helpers | Negócio reutilizável e utilidades genéricas, respectivamente |
| Modelos | Dados, invariantes e conversões próprias |
| Infraestrutura | Driver, SQL, decodificação e dependências para dentro |

A resposta fixa de health não justifica camadas vazias ou classes de encaminhamento. A revisão não adicionou CRUD genérico, novos contratos de escrita ou abstrações sem comportamento.

## Decisões e custos

- ADR 001: Rust explicita propriedade/concorrência, com custo de aprendizado e compilação. Não elimina falhas lógicas de entrega.
- ADR 002: PostgreSQL suporta fronteira transacional local e JSONB. Operação stateful, retenção e contenção exigirão gestão; não houve benchmark comparativo.
- ADR 003: SQL versionado e CLI explícito dão histórico revisável. Rollback não é recuperação; tags não tornam base/OS imutáveis.
- ADR 004: Outbox durável evita dual write local quando usado pelo produtor na mesma transação. Publicação remota continua separada e pode duplicar.
- ADR 005: Contrato de leitura específico evita prometer autoridade de transição inexistente. Dispatch estático/futures Send dispensam boxing obrigatório; trait não é dyn-compatible.

Alternativas registradas: Go/Java para linguagem; MySQL/SQLite ou broker como armazenamento; mudanças manuais/ORM para schema; publicação antes/depois do commit, 2PC ou CDC para handoff; SQLx direto, CRUD genérico ou contrato completo de mutação para persistência. PostgreSQL/outbox e leitura focada foram escolhidos pelo escopo e fronteiras explícitas, não por desempenho medido. Detalhes e custos estão nos ADRs.

Nenhuma decisão material nova exigiu ADR no fechamento. Os cinco ADRs registram contexto histórico; estado atual está nesta entrega e na revisão.

<!-- pagebreak -->

## Envelope e compatibilidade de armazenamento

EventEnvelope tem nove campos privados: id, event_type, aggregate_type, aggregate_id, schema_version, occurred_at, correlation_id, causation_id e payload. `new` gera UUID v7 e tempo UTC atual; restore e Serde preservam valores históricos sem gerar nova identidade. UUID v7 não garante ordem global de negócio.

Nomes e agregados vazios ou somente whitespace são rejeitados. Espaços em valores não vazios são preservados. Versão zero é inválida. Correlation/causation são UUIDs opcionais, sem suporte direto a qualquer token externo. Payload é JSON flexível, não schema de negócio validado pelo relay.

| Representação | Limite / conversão |
| --- | --- |
| UUID e TEXT | Identidade preservada; formato de agregado pertence ao produtor |
| NonZeroU32 / BIGINT | CHECK permite 1 a 4294967295; conversão assinada verificada |
| Tentativas u32 / INTEGER | Banco termina em i32::MAX; escrita futura precisa checar largura |
| DateTime UTC / TIMESTAMPTZ | Instante em microssegundos; offset original/nanossegundos não preservados |
| Value / JSONB | Conteúdo semântico; formatação/ordem de chaves normalizadas |
| JSON numérico | arbitrary_precision preserva inteiros grandes, decimais longos e 1e1000 armazenado |

Schema `relay.outbox_events` tem 15 colunas, 11 NOT NULL, chave primária e sete checks. Estados: pending, processed e dead_letter. Processed exige processed_at não nulo; os demais exigem nulo. Attempt_count é não negativo. Índice parcial pending usa available_at, created_at e id.

Essas constraints não validam história de transições ou propriedade. PostgreSQL aceita campos só com espaços, tempos infinitos e datas além do Chrono; decoder/domínio rejeitam essas linhas quando selecionadas. Isso é uma diferença explícita de representação, não motivo para enfraquecer constraints.

Duas migrações up criam namespace e tabela em ordem. Down usa RESTRICT, mas DROP TABLE destrói dados. SQL em `migrations/` é fonte versionada; `_sqlx_migrations` é metadado no banco, persistido fisicamente com o cluster.

<!-- pagebreak -->

## Fronteiras de transação e leitura

O produtor deve gravar mudança de negócio e outbox na mesma transação local. O relay independente não implementa essa transação nem oferece OutboxWriter. Commits separados permitem que somente uma gravação aconteça; broker remoto não participa da atomicidade local. Base conceitual: [transações PostgreSQL 18](https://www.postgresql.org/docs/18/tutorial-transactions.html).

Confirmação de COMMIT perdida após desconexão pode deixar resultado incerto. Esta é análise de engenharia sobre commit e resposta separados, sem injeção de falha realizada. Chave duplicada não prova payload, metadados e efeitos iguais. Reconciliar a operação e definir idempotência do produtor seguem responsabilidades futuras; gerar novo ID em todo retry pode duplicar o evento lógico.

OutboxReader recebe corte UTC explícito e BatchSize positivo. Adapter faz um SELECT, pending com available_at até o corte inclusivo, LIMIT verificado como i64. Limite excessivo representável falha antes de I/O. Ordenação disponibilidade/criação/ID é detalhe testado do adapter, não garantia de entrega do contrato.

Snapshot depende do isolamento efetivo. Em Read Committed começa na instrução; níveis mais fortes podem usar snapshot da transação. A sessão local foi observada em Read Committed. Adapter não força isolamento nem abre transação explícita de várias instruções. Base: [isolamento PostgreSQL 18](https://www.postgresql.org/docs/18/transaction-iso.html).

Não há FOR UPDATE/SHARE, claim, reserva ou mutação de ciclo de vida. SELECT ainda tem locks normais de tabela, como ACCESS SHARE. Dois leitores podem observar a mesma linha; metadados podem ficar obsoletos imediatamente. Repetição pode ver dados novos, embora não grave. Lote vazio não prova backlog vazio ou permanentemente esgotado.

Contagem não limita bytes do payload nem concorrência de workers. Configurações produtivas de pool, consulta, polling e admissão não existem no adapter. Prazos do harness são de teste, não defaults da aplicação.

Teste comprova observação repetida de fixtures estáveis, comparação completa antes/depois e dois leitores sem propriedade. Não comprova toda sequência de escritas concorrentes; semântica geral de snapshot vem da documentação oficial.

<!-- pagebreak -->

## Falhas atuais e janelas futuras

A [matriz completa](failure-and-transaction-semantics.md) distingue integração real, testes unitários, código/fontes oficiais e análise futura. Síntese:

| Ponto de falha | Observação / estado | Dono da recuperação / evidência |
| --- | --- | --- |
| Banco indisponível; pool fechado/timeout | Unavailable; leitor não grava outbox | Chamador/composição e operador; classificação unitária, pool fechado pelo contrato |
| Tabela ausente / erro SQL | OperationFailed com fonte; sem reparo automático | Operador de schema/deploy; tabela ausente testada em PostgreSQL isolado |
| Dado inválido selecionado | Lote inteiro falha; linha permanece | Dono dos dados/operador; campos em branco e tempos inválidos testados |
| Future cancelada | Pode não haver resultado; sem claim durável | Chamador/pool; término no servidor não comprovado pelo cancelamento local |
| Reinício do processo | Snapshot em memória desaparece; nenhum claim a retomar | Aplicação futura/operador; crash abrupto não injetado |
| Publicação futura antes de confirmar no banco | Destino pode agir e linha continuar pending; retry duplica | Worker/consumidor idempotente; somente análise, publisher inexistente |
| Processed futuro antes de publicar | Linha é excluída da leitura apesar de possível não entrega | Desenho do worker; janela de perda analisada, transição inexistente |

Linha inválida elegível pode bloquear repetidamente lotes com linhas válidas. Não há skip, correção, quarentena ou dead-letter automático. Linhas inválidas excluídas por corte/status não afetam lote elegível, conforme integração. Corrigir fixture permite leitura válida seguinte.

Categorias não definem retry nem garantem sucesso ao repetir. Display/Debug são sanitizados; fontes tipadas preservam diagnóstico e podem revelar detalhes sensíveis. Cadeias de fontes precisam de redação antes de logs. Não há política de retries no repositório.

Cancelamento do cliente não prova término imediato da consulta; cancelamento PostgreSQL pode chegar depois da conclusão. Pool::close SQLx espera conexões emprestadas voltarem/fecharem, sem interromper forçosamente toda consulta. Timeout de aquisição não limita consulta inteira. São semânticas documentadas, não medições de prazo da aplicação.

Persistência e schema não bastam para entrega funcional. Publicação/ack separados criam janelas de duplicação ou perda. Consumidor idempotente precisará coordenar deduplicação e efeitos de negócio; implementação e testes permanecem futuros.

<!-- pagebreak -->

## Durabilidade, operação e decisões abertas

Estado commitado depende também de configuração PostgreSQL, armazenamento e implantação. Commit assíncrono pode confirmar antes do flush durável do WAL. Na sessão local, fsync, synchronous_commit e full_page_writes foram lidos como on. Isso não audita power loss, integridade de storage, réplicas ou SLA.

Bind mount `.dockerized-postgres/` preserva arquivos locais após recriação normal, mas não é backup. Retenção, backups, ensaios de restore, capacidade e topologia de produção exigem projeto operacional. Migração down não recupera dados; reaplicar schema não restaura linhas removidas. Neste fechamento não houve reset, rollback compartilhado, alteração de credenciais ou restart do PostgreSQL para simular falhas.

## Testes, isolamento e configuração

Compose: app usa postgres:5432; host usa 127.0.0.1:5433. Harness aceita POSTGRES_HOST/PORT explícitos; CI usa 127.0.0.1:5432. POSTGRES_USER/PASSWORD/DB são obrigatórios no opt-in. Papel precisa de CREATEDB e propriedade dos bancos descartáveis. Credenciais não constam deste handoff.

Cada caso cria banco `relay_it_<UUID hexadecimal>`, com nome controlado/citado, migrações up versionadas e fixtures explícitas. Prazos: conexão/aquisição 10 s, instrução 5 s, lock 3 s, caso 45 s e operações administrativas/limpeza 10 s. Leitores sincronizam por barreira, sem sleeps fixos no target PostgreSQL.

Pool fecha antes de remover exatamente o banco criado; ausência é verificada no catálogo. Limpeza após panic e erro retornado tem testes próprios. Processo terminado abruptamente, servidor indisponível na limpeza ou prazo excedido pode deixar fixture isolada. Identifique nome/propriedade exatos antes de remoção manual; nunca faça limpeza ampla.

## O que ainda exige decisão

- Claims/leases, propriedade concorrente e retomada após falha.
- Autoridade de transição, estado esperado e repetição de confirmações.
- Retry/backoff, limites operacionais e política de indisponibilidade.
- Dead-letter, quarentena, replay e retenção.
- Idempotência do produtor e consumidor com efeitos atômicos.
- Ordem por agregado e trade-offs com paralelismo.

São questões para a próxima fase, não decisões resolvidas por esta entrega. Não há broker, benchmark ou framework especulativo de crash testing.

<!-- pagebreak -->

## Validação executada nesta entrega

Comandos Cargo via `docker compose exec -T app`; flags rustdoc fornecidas com `-e`. Base da revisão 7cba38d; resultados abaixo correspondem às mudanças de fechamento, não ao relatório anterior. Rust 1.95.0, PostgreSQL 18.6 e SQLx CLI/driver 0.8.6.

| Comando / verificação | Resultado atual |
| --- | --- |
| cargo fmt --check | Passou |
| cargo clippy --locked --all-targets --all-features -- -D warnings | Passou |
| cargo test --locked | 23 passaram; 15 casos PostgreSQL ignorados |
| cargo test --locked --test postgres_repository -- --ignored --test-threads=1 | 15 passaram |
| cargo test --locked --test postgres_repository -- --ignored --test-threads=4 | 15 passaram; isolamento paralelo |
| cargo build --locked | Passou |
| RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps | Passou |
| git diff --check | Passou |
| scripts/check-postgres.sh | Health, TCP host, configuração e identidade autenticada passaram |
| sqlx migrate info | Duas migrações instaladas; sem pendências |
| inspect_outbox_schema.sql | Catálogo de colunas/constraints/índices/histórico inspecionado, somente leitura |
| Fixture SQL de schema isolada | 25 assertions, rollback e zero linhas restantes |
| Catálogo de bancos temporários | Zero nomes controlados relay_it restantes |

Suíte padrão permanece independente de banco. Suite opt-in valida elegibilidade/limites, todos os campos restaurados, JSON numérico, observação sem mutação, falhas/fontes e limpeza. O job CI dedicado preserva PostgreSQL 18.6 e credenciais descartáveis; utiliza o mesmo target com quatro threads.

Não validados: execução Rust nativa no host, CI remoto GitHub, implantação produtiva, backup/restore real, desempenho, crash abrupto, prazo de cancelamento no servidor, pool esgotado na integração e toda sequência de escritas concorrentes. Sem bloqueadores dos critérios atuais. Resultado de testes não é prova de entrega, capacidade ou recuperação produtiva.

PDF tem texto selecionável, acentos, paginação e renderização de todas as páginas revisada. Fonte Markdown e gerador Python ficam versionáveis; dependências de relatório não foram adicionadas ao Rust.

<!-- pagebreak -->

## Achados, correções e retomada

Não houve defeito produtivo confirmado nem mudança em código produtivo, Cargo.lock ou migrações. Revisão corrigiu documentação e reforçou duas áreas de evidência:

| Achado | Severidade / ação |
| --- | --- |
| Roteiro/status ainda pendente | Média documental; estado reconciliado nos dois idiomas |
| Leitura descrita como sem locks | Média documental; distingue locks normais e propriedade |
| Histórico aplicado fora do data dir | Média documental; separa SQL fonte de metadados do banco |
| JSON complexo sem esperado integral explícito | Baixa de cobertura; assertion da fixture inteira com expoente normalizado |
| Schema SQL fora da suíte isolada | Baixa de validação; caso ignorado executa 25 assertions existentes |

Para retomar antes do Marco 2: leia revisão e semântica, confira o diff não commitado e a documentação histórica de validação. Preserve fronteiras atuais e determine explicitamente a próxima tarefa; não use snapshots como permissão para entregar concorrentemente. O desenho futuro precisará definir propriedade e autoridade de transição antes de prometer comportamento de worker.

Commit sugerido, ainda não executado: `docs: close milestone 1 with failure and transaction semantics`.

## Mapa de referências do repositório

| Referência | Conteúdo |
| --- | --- |
| src/domain/event.rs; src/persistence/ | Envelope, contrato, snapshots e erros |
| src/infrastructure/postgres/mod.rs; migrations/ | Consulta, decoders, mapeamento e SQL up/down |
| tests/postgres_repository.rs; tests/support/; tests/sql/ | Evidência real, isolamento e constraints |
| compose.yaml; Dockerfile; docker/sqlx.py; scripts/check-postgres.sh | Topologia, ferramentas e diagnóstico |
| .github/workflows/ci.yaml | Checks independentes e integração dedicada |
| docs/pt-BR/milestone-1-review.md; validation-results.md; failure-and-transaction-semantics.md; adr/001-005 | Revisão, resultados, matriz completa, fontes oficiais e decisões |

Fontes oficiais de apoio: PostgreSQL 18 [isolamento](https://www.postgresql.org/docs/18/transaction-iso.html), [locks](https://www.postgresql.org/docs/18/explicit-locking.html), [protocolo/cancelamento](https://www.postgresql.org/docs/18/protocol-flow.html), [WAL/commit](https://www.postgresql.org/docs/18/wal-async-commit.html) e [backup](https://www.postgresql.org/docs/18/backup.html); SQLx 0.8.6 [pool](https://docs.rs/sqlx/0.8.6/sqlx/struct.Pool.html) e [opções](https://docs.rs/sqlx/0.8.6/sqlx/pool/struct.PoolOptions.html). A documentação de semântica distingue fontes externas, código/testes e inferências futuras.
