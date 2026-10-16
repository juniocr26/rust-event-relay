[English](../../en/architecture/milestone-4-2.md) | [Catálogo](../index.md)

# Marco 4.2 — persistência atômica de estoque no PostgreSQL

Implementado em 13/10/2026, base Rust `4d3734c79b3d635a91a4e7d415a903b18b43a610` e Library `281231333240e20a9704e15bb9fbcdaeb5ed151c`; ambos começaram limpos. Entrega o efeito local de banco previsto em 4.1. Consumo RabbitMQ, execução de ACK, publicação de quarentena, endpoint produtor e frontend continuam fora do escopo. Rollout compartilhado é separado.

## Representação imutável v1

O [adaptador](../../../backend/src/infrastructure/postgres/consumer.rs) armazena versão SMALLINT 1 e BYTEA com JSON UTF-8 do EventEnvelope validado: ID/tipo, tipo/ID de agregado, versão, instante, correlação/causação e payload. Não inclui propriedades de transporte, campos desconhecidos descartados pela restauração canônica ou histórico do relay. A igualdade compara modelos restaurados e validados, sem digest, Debug ou comparação de texto arbitrário.

Ordem de objetos e espaços JSON não importam; strings são exatas, arrays preservam ordem, timestamps comparam instantes UTC, IDs opcionais ausentes/null viram None. Payload aceita somente warehouse_id/sku/quantity, com inteiro positivo i64; decimal/expoente e números fora do limite rejeitam. BYTEA evita normalização numérica de JSONB, restrição de NUL do TEXT e perda de nanossegundos pelo TIMESTAMPTZ. A conclusão do recibo usa timestamp do banco e o decoder seguro existente.

Conteúdo limitado a 1..16384 bytes; identificadores ASCII de 1..128 bytes, sem trim/case folding. Encoding verifica tamanho e round-trip antes de conexão/mutação; entrada não suportada retorna Invalid(Unsupported). Conteúdo armazenado inválido, versão desconhecida, ID/efeito divergente, conclusão inválida ou soma incoerente retornam InvalidStoredData explicitamente. JSON genérico suportado pelo envelope não amplia o contrato do inventário.

## Schema e justificativa do trigger

[Migração aditiva 20261014000000](../../../backend/migrations/20261014000000_add_atomic_consumer.up.sql): consumer.stock com PK warehouse_id/sku e BIGINT não negativo; consumer.receipts com PK consumer_name/event_id, UUID não nulo lógico, conteúdo/versionamento, quantidade positiva, stock_before/stock_after e completed_at finito. Collation C mantém chaves exatas. Apenas índices das PKs são necessários. FK diferida permite reservar recibo antes de criar estoque ausente. Recibos são retidos indefinidamente. Down perde todo estoque/deduplicação/histórico e permite reaplicar eventos antigos; somente testar em banco descartável.

A reserva começa com os três campos de conclusão NULL. CHECK permite esse estado temporário ou conclusão coerente, com soma NUMERIC para não causar overflow no próprio CHECK. NOT NULL/CHECK imediato impediria reservar antes do lock; tabela separada exigiria outro ciclo de vida, e procedure com inserção final substituiria o protocolo Rust escolhido. Mantemos AFTER INSERT/UPDATE como constraint trigger DEFERRABLE INITIALLY DEFERRED. Ele consulta a linha final, sem confiar no NEW incompleto da inserção. Guard imediato proíbe trocar identidade, alterar recibo concluído, DELETE e TRUNCATE; FK impede excluir estoque referenciado.

Recibo completo sozinho não prova incremento: campos coerentes, FK e transação do adaptador vinculam o efeito. Escritor SQL privilegiado ainda pode inventar recibos/alterar estoque; owner/superuser pode desabilitar triggers. Não é auditoria inviolável. PUBLIC não recebe privilégios. Deployment concede USAGE no schema, SELECT/INSERT/UPDATE(units) no estoque e SELECT/INSERT/UPDATE(stock_before,stock_after,completed_at) nos recibos, sem DELETE/TRUNCATE/DDL. Nenhuma mudança de credencial/role ou topologia RabbitMQ. Corrupção de teste desabilita o guard apenas no banco exato descartável e o restaura.

## Transação real: 10 + 5

READ COMMITTED explícito: INSERT de recibo ON CONFLICT DO NOTHING RETURNING event_id coordena a chave única. Concorrente espera commit/rollback. Perdedor executa SELECT separado com snapshot novo, restaura recibo concluído e compara envelope completo. Uma CTE única não garante enxergar a linha conflitante ausente do snapshot inicial. Ausência/dado ilegível gera falha, sem fingir Duplicate. Conteúdo igual retorna Duplicate com resultado histórico; conteúdo diferente retorna IdentityConflict. Nenhum desses caminhos altera estoque.

Vencedor cria estoque ausente em zero ON CONFLICT DO NOTHING, trava a linha FOR UPDATE, valida valor atual e soma com Stock::receive. Atualiza estoque, conclui recibo e só retorna Applied após COMMIT reconhecido. Ordem de locks: identidade, depois estoque; uma chave de cada por chamada. Sem cálculo sobre leitura antiga, SKIP LOCKED, I/O de broker ou retries automáticos. Eventos distintos serializam no mesmo estoque. Consumidores lógicos diferentes têm deduplicação independente e podem incrementar o estoque compartilhado; responsabilidades distintas precisarão de filas próprias.

Fixture 10 + evento A de 5 = estoque 15 e recibo A(15). A duplicado mantém 15. Evento B distinto soma 5 e chega a 20. A duplicado depois de B ainda retorna recibo histórico 15, sem mudar saldo atual 20. A conflitante não muda nada. Ausência começa em zero. Overflow faz rollback de reserva/criação de estoque e retorna Invalid(Overflow). Falha entre escritas ou rejeição diferida de COMMIT desfaz ambos.

## Prazos, falhas e cancelamento

SQLx 0.8.6 foi inspecionado: commit aguarda o transaction manager; Drop de transação aberta enfileira rollback. Cancelamento não desfaz COMMIT já recebido pelo servidor. Reutilizamos map_commit_error: rejeição conhecida vira PreCommit; resposta perdida/desconhecida vira CommitUncertain. Erros de comando, aquisição/locks/deadlines, deadlock 40P01 e serialização são PreCommit sanitizado. InvalidStoredData é categoria explícita nova; nenhuma falha autoriza ACK. Sem retry oculto ou lookup adicional.

Chamador configura acquire_timeout, statement_timeout, lock_timeout e prazo global/admissão/shutdown. Adaptador não substitui deadlines da sessão. Harness usa aquisição 10s, locks 3s, comandos 5s, observações/proxy limitados; fixture verifica lock_timeout 100ms. Não há garantia de fairness/throughput.

Reprocessar a mesma identidade após incerteza pode descobrir Duplicate durável ou aplicar se houve rollback. Observação negativa não exclui commit em voo. Future não consultado não começa mutação; cancelamento durante espera ou após escritas suprime resultado sem prova geral de rollback. Proxy observa COMMIT do servidor e retém resposta: testes inspecionam estoque/recibo independentemente. Isso não prova perda de energia ou recuperação de processo morto.

Política futura permite ACK para Applied/Duplicate. Conflito/inválido precisa preservação confirmada e roteada antes do ACK. Falha/incerteza/cancelamento exige recuperação com pacing. Nenhum adaptador de ACK existe aqui.

## Evidência e próximo marco

[Dez testes PostgreSQL isolados](../../../backend/tests/postgres_consumer.rs) cobrem preservação de envelope/histórico relay durante upgrade; estoque/histórico/scopes/overflow; concorrência igual/conflitante/distinta; constraints/completude/imutabilidade/down-up; rejeição de comando/commit; round-trip/ordem/números/corrupção; espera observada/visibilidade do recibo vencedor/rollback do primeiro writer/deadline; perda de resposta e cancelamento após COMMIT; cancelamento antes da admissão/durante espera. Harness verifica remoção dos bancos exatos. Proxies são próprios do teste, sem sleeps frágeis. [Checker de migrações](../../../scripts/check-migrations.py) verifica seis upgrades/checksums SQLx, no-op e down/re-up. [Validação datada](../testing/validation-results.md) separa execução local de configuração CI.

Pré-requisitos exatos de 4.3: runtime próprio de ACK manual individual; prefetch/admissão/shutdown limitados e recuperação pausada; provisionamento/verificação da topologia durável exata de quarentena e permissões; sidecar AMQP lossless versionado com tags de tipo e bytes originais; publicação persistent/mandatory com confirms/returns correspondentes; limites de mensagens/memória/disco e protocolo de oversize; testes isolados de destino ausente/indisponível/cheio e ausência de ACK antecipado. Preservar relay.local classic; rollout compartilhado de migrações/topologia/runtime depende de autorização separada. Sem consumo compartilhado, replay/purge de quarentena, TTL/descarte automático ou efeitos externos. 4.4 cobre crash/redelivery/ACK incerto e fechamento integrado.

Referências primárias consultadas em 13/10/2026: [INSERT](https://www.postgresql.org/docs/18/sql-insert.html), [isolamento](https://www.postgresql.org/docs/18/transaction-iso.html), [constraints](https://www.postgresql.org/docs/18/ddl-constraints.html), [triggers diferidos](https://www.postgresql.org/docs/18/sql-createtrigger.html), [SQLx 0.8.6](https://docs.rs/sqlx-core/0.8.6/src/sqlx_core/transaction.rs.html).
