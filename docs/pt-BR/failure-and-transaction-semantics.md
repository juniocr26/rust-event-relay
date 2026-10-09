[English](../en/failure-and-transaction-semantics.md) | [README](../../README.pt-BR.md)


## Extensão atual — Marco 2.2

[Estado de entrega e posse](milestone-2-2.md) e [ADR 007](adr/007-durable-delivery-ownership.md) definem recuperação por lease durável e contratos separados de adquirir/concluir/liberar. Migração nova `20261009000000_add_delivery_ownership` adiciona token/acquired_at/expires_at nullable com lease coerente apenas em pending. Seções de marcos anteriores abaixo descrevem escopo original; afirmações antigas de posse/contador indefinidos são substituídas pelo ADR 007. Adapters produtivos de mutação ficam para 2.3; SELECT do reader e publisher preservados.
# Semântica de falhas e transações - Marco 1.8

O Marco 1 fornece representação durável e adapter somente leitura, sem engine de entrega funcional. Este documento conclui o critério de semântica/documentação sem implementar escrita de produtores, propriedade, transições ou destinos. Base revisada: `7cba38d`; mudanças de fechamento não commitadas. Veja [revisão](milestone-1-review.md) e [validação atual](validation-results.md#marco-18-e-fechamento-do-marco-1-2026-10-08).

## Fronteira do produtor

Mudança de negócio e inserção em `relay.outbox_events` precisam participar da **mesma transação local do produtor**. Atomicidade une essas gravações; commits separados permitem que somente uma operação tenha sucesso. A base é a [semântica transacional PostgreSQL 18](https://www.postgresql.org/docs/18/tutorial-transactions.html). O relay independente não controla essa transação nem expõe OutboxWriter. Broker remoto ou outro banco fica fora dessa fronteira.

Perda da confirmação de COMMIT impede o produtor de concluir se houve sucesso. Esta é uma inferência de engenharia sobre conclusão no banco e confirmação ao cliente separadas no [protocolo PostgreSQL](https://www.postgresql.org/docs/18/protocol-flow.html#PROTOCOL-FLOW-SIMPLE-QUERY), sem injeção de falha executada. Recuperação deve reconciliar operação de negócio e evento pretendidos. Chave primária duplicada comprova identidade existente, sem comprovar payload, metadados ou efeitos iguais. Idempotência do produtor, comparação de conteúdo e reconciliação seguem abertas. Gerar nova identidade indiscriminadamente ao repetir pode duplicar eventos lógicos.

## Fronteira atual de leitura

Adapter executa um SELECT parametrizado pelo PgPool injetado, sem transação explícita de várias instruções nem ajuste de isolamento. Cada SELECT observa snapshot consistente sob o isolamento efetivo da conexão. Em Read Committed, snapshot começa na instrução; isolamento mais forte pode usar snapshot anterior da transação. Veja [isolamento PostgreSQL 18](https://www.postgresql.org/docs/18/transaction-iso.html). Validação local observou Read Committed; adapter não exige essa configuração em toda implantação.

Elegibilidade exige pending e `available_at <=` corte UTC explícito do chamador, retornando no máximo o limite positivo. Adapter ordena por disponibilidade, criação e ID para seleção determinística com fixtures inalteradas; contrato e UUID v7 não prometem ordem de entrega de negócio. Não executa `FOR UPDATE/SHARE`, claim, reserva ou gravação de ciclo de vida. SELECT simples ainda adquire locks normais de tabela, como ACCESS SHARE, e pode interagir com DDL; “somente leitura” não significa “sem locks”. Veja [locks PostgreSQL 18](https://www.postgresql.org/docs/18/explicit-locking.html).

Outros leitores podem observar o mesmo evento. Metadados podem ficar obsoletos imediatamente. Repetir leituras não muda ciclo de vida, mas pode observar linhas alteradas. Lote vazio significa ausência de linhas elegíveis naquele snapshot, sem comprovar backlog vazio ou permanentemente esgotado. Limite por contagem não limita bytes do payload nem concorrência total. Pool, prazos de consulta, polling e admissão pertencem à composição/aplicação; adapter não os configura. Prazos do harness são configurações de teste, não defaults produtivos.

## Matriz de falhas

Evidência: **I** = integração atual com PostgreSQL real; **U** = teste unitário/contrato sem banco; **S** = semântica oficial ou código inspecionado; **F** = análise futura, sem entrega executada. Nomes em [postgres_repository.rs](../../tests/postgres_repository.rs) e [postgres/mod.rs](../../src/infrastructure/postgres/mod.rs).

| Ponto de falha / crash | Resultado observável | Implicação durável | Comportamento atual | Responsável pela recuperação | Evidência | Trabalho adiado |
| --- | --- | --- | --- | --- | --- | --- |
| Banco inacessível; I/O/TLS/autenticação/shutdown | Sem lote bem-sucedido; erros de acesso mapeados para Unavailable | Leitor não grava; não infere commits de outros atores | Erro classificado com fonte; sem retry automático | Chamador e operador do banco | U: classificação; S: mapeamento. Probe indisponível do 1.7 é histórico | Reconexão, admissão e retry |
| Pool fechado / prazo de aquisição excedido | PoolClosed / PoolTimedOut vira Unavailable | Sem transição outbox por esta leitura | Pool pertence ao chamador; retorna erro | Composição / chamador | U: pool fechado pelo contrato e mapeamento de timeout; S: SQLx. Sem injeção de pool esgotado na integração | Configuração operacional e recuperação |
| Tabela ausente ou outro erro SQL | OperationFailed; tabela ausente preserva fonte SQLx Database | Adapter não grava outbox | Leitura não cria nem repara schema | Operador de schema/deploy e chamador | I: missing_table_preserves_typed_database_failure; U: SQLSTATE | Controles de deploy e retry transacional |
| Linha selecionada inválida na decodificação/domínio | Lote inteiro falha com InvalidStoredData; sem resultado parcial | Linha inválida permanece; sem quarentena | Decodificação/restauração privada verificada; fonte mantida | Dono dos dados/produtor e operador | I: campos em branco, infinity, fora do Chrono; correção permite leitura válida | Reparo/quarentena autorizados e política de incidente |
| Future cancelada ou chamador encerra | Chamador pode não receber lote/erro; future descartada não informa conclusão no servidor | Não cria claim durável nem transição | Somente SELECT; sem protocolo aplicativo de cancelamento | Chamador/pool; banco limita recursos | S: consulta e cancelamento PostgreSQL; sem teste de prazo de cancelamento | Prazos produtivos e observação do servidor |
| Processo relay reinicia | Snapshots em memória desaparecem; aplicação atual reinicia somente HTTP | Leitor não alterou linhas commitadas; sobrevivência depende do armazenamento | Sem retomada de worker ou estado claimed | Futuro orquestrador / operador | S: main e SELECT; I: linhas inalteradas. Sem injeção de crash/restart abrupto | Recuperação de workers e draining |
| Publicação futura tem sucesso; crash antes do commit de confirmação no banco | Destino pode ter agido; retry posterior pode duplicar | Linha pode continuar pending apesar do sucesso externo | Sem publisher ou método de confirmação | Futuro worker e consumidor idempotente | F: análise de fronteiras; sem teste | Propriedade, autoridade de confirmação, deduplicação |
| Confirmação futura processed commitada antes da publicação | Leitura exclui linha mesmo sem entrega | Conclusão durável pode esconder perda de entrega | Essa transição não está implementada | Desenho futuro do worker | F: análise; I: filtro processed | Sequência publicação/confirmação; evitar janela de perda |
| Publicação futura / COMMIT do produtor perde resposta | Resultado incerto, não necessariamente falha conhecida | Operação externa/commit pode ter acontecido | Sem protocolo de writer/destino | Produtor ou futuro worker reconcilia | S/F: conclusão e resposta separadas; sem injeção | Reconciliação idempotente e efeitos do consumidor |

Linha inválida persistentemente elegível pode falhar todo lote que a selecione, bloqueando repetidamente progresso das outras linhas válidas selecionadas. Adapter nunca ignora, repara ou põe em quarentena automaticamente. Linhas inválidas excluídas por status/corte não afetam a leitura elegível, conforme testes. Isso não autoriza exclusão ampla de dados.

Categorias expressam significado da falha, sem definir retry nem garantir sucesso ao repetir. Erros SQL desconhecidos caem em OperationFailed. Display/Debug mostram classificação/presença de fonte; fontes SQLx, domínio e conversão ficam acessíveis por Error::source. Cadeias podem conter credenciais/payload: sanitizar antes de registrar. Veja [mapeamento](postgres-repository.md).

## Cancelamento e pool

Cancelamento PostgreSQL usa pedido de protocolo separado e pode não agir antes de a consulta terminar; cancelamento do cliente não comprova término imediato no servidor. Veja [cancelamento PostgreSQL 18](https://www.postgresql.org/docs/18/protocol-flow.html#PROTOCOL-FLOW-CANCELING-REQUESTS). Não se infere prazo garantido ao descartar future SQLx.

SQLx 0.8.6 documenta que [Pool::close](https://docs.rs/sqlx/0.8.6/sqlx/struct.Pool.html#method.close) impede novas aquisições e espera conexões emprestadas voltarem/fecharem; não cancela forçosamente todas as operações emprestadas. [PoolOptions::acquire_timeout](https://docs.rs/sqlx/0.8.6/sqlx/pool/struct.PoolOptions.html#method.acquire_timeout) limita aquisição, não consulta inteira. Composição produtiva precisa escolher prazos separados. São semânticas documentadas do driver, não novo comportamento da aplicação.

## Estado durável, armazenamento e recuperação

Estado commitado e durabilidade da implantação são preocupações diferentes. [Commit síncrono/assíncrono](https://www.postgresql.org/docs/18/wal-async-commit.html) afeta se confirmação de sucesso antecede flush durável do WAL. [fsync, synchronous_commit e full_page_writes](https://www.postgresql.org/docs/18/runtime-config-wal.html) interagem com a correção do armazenamento. Validação leu os três como on; isso é observação de configuração, sem auditoria de perda de energia/storage, garantia de réplica ou SLA produtivo.

Bind mount preserva arquivos locais em recriação normal; **não é estratégia de backup**. Backups, ensaios de restauração, retenção, capacidade, controles de acesso e topologia precisam de desenho operacional separado. PostgreSQL descreve [backup/restore](https://www.postgresql.org/docs/18/backup.html). Testes e persistência Docker não demonstram durabilidade produtiva, desempenho ou recuperação de crash.

SQL versionado é histórico fonte; metadados `_sqlx_migrations` aplicados são estado do banco e persistem fisicamente com o cluster. Migrações down removem objetos com RESTRICT; após existir dados outbox, remover tabela os destrói. Rollback não é recuperação, e reaplicar migrações não restaura linhas ou credenciais excluídas. Nenhum rollback/reset compartilhado foi executado no fechamento.

Harness fecha pools antes de remover exatamente seu banco gerado, confere ausência e exercita limpeza após panic/erro. Término abrupto, limpeza falha ou perda do servidor ainda podem deixar banco isolado. Identifique nome/propriedade exatos antes da limpeza manual; nunca remova bancos em massa. Não houve teste de término abrupto.

## Evidência e decisões abertas

Testes atuais comprovam invariantes do envelope, armazenamento representável, restauração selecionada, corte inclusivo/limites, filtros, observações repetidas inalteradas, ausência de mutação, observação por dois leitores, erros/fontes e limpeza isolada. Fixture de schema versionada adiciona 25 assertions com rollback. Descrições de snapshot/locks/durabilidade vêm da documentação oficial versão 18; SQLx é referenciado especificamente em 0.8.6. Suíte não enumera toda sequência concorrente nem injeta cada falha da matriz.

Tabela durável e API de observação não fornecem garantia de entrega funcional. Publicação antes da confirmação no banco pode duplicar em retry; confirmação antes de publicar pode perder entrega. Idempotência do consumidor precisa coordenar deduplicação com efeitos de negócio. Claims/leases, propriedade concorrente, autoridade/estado esperado de transições, retry/backoff, dead-letter/quarentena/replay, limites de bytes/contagem/concorrência, retenção, duplicatas do produtor e ordem por agregado seguem abertos. Documento não escolhe nem implementa essas decisões, e Marco 2 não foi iniciado. ADRs 001-005 continuam aplicáveis; fechamento documental não exige novo ADR.
