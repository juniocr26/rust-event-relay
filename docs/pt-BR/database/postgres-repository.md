[English](../../en/database/postgres-repository.md) | [README](../../../README.pt-BR.md)


# Repositório PostgreSQL — Marco 1.6

## Extensão atual — Marco 2.2

[Estado de entrega e posse](../architecture/milestone-2-2.md) e [ADR 007](../adr/007-durable-delivery-ownership.md) definem recuperação por lease durável e contratos separados de adquirir/concluir/liberar. Migração nova `20261009000000_add_delivery_ownership` adiciona token/acquired_at/expires_at nullable com lease coerente apenas em pending. Seções de marcos anteriores abaixo descrevem escopo original; afirmações antigas de posse/contador indefinidos são substituídas pelo ADR 007. Adapters produtivos de mutação ficam para 2.3; SELECT do reader e publisher preservados.


`PostgresOutboxRepository::new(pool)` recebe `PgPool` injetado. O chamador constrói/configura o pool, por exemplo com `PgPoolOptions::connect_with` e `PgConnectOptions`, e controla seu encerramento. Métodos de consulta não leem ambiente nem criam conexões globais. Bootstrap HTTP continua sem banco. Infraestrutura implementa `OutboxReader` existente e depende de persistência/domínio; SQLx não entra nesses módulos. `StoredEvent` e decodificação são privados. Modelos validam/convertem dados sem consultas. ADR 005 permanece suficiente e inalterado.

SQLx 0.8.6 é fixado na mesma versão do CLI Docker, com defaults desabilitados e somente postgres, runtime-tokio, uuid, chrono e json. Rust 1.95.0 é a versão validada. Não há macros, metadados offline, migrações pela aplicação ou backend TLS. Conexão Docker local funciona sem TLS; implantação que exigir TLS precisará habilitá-lo/configurá-lo deliberadamente. Veja [documentação SQLx](https://docs.rs/sqlx/0.8.6/sqlx/).

## Consulta e limites

Uma consulta parametrizada seleciona colunas explícitas do envelope e attempt_count/available_at em `relay.outbox_events`. Filtra `status = 'pending' AND available_at <= $1`, ordena por `available_at, created_at, id`, conforme índice pending, e limita com `LIMIT $2`. Cada SELECT observa snapshot consistente sob isolamento efetivo da conexão; em Read Committed começa na instrução. Ordem é detalhe de implementação, sem garantia de ordem de entrega. Não há locks de propriedade das linhas (FOR UPDATE/SHARE), claims, reservas, gravações de ciclo de vida ou segunda consulta. Locks normais de tabela do SELECT continuam aplicáveis; veja [falhas/transações](../architecture/failure-and-transactions.md). Leitores podem receber os mesmos eventos; metadados podem ficar desatualizados imediatamente.

Limite usize usa `i64::try_from`: acima do BIGINT assinado retorna OperationFailed com fonte tipada antes de I/O, sem truncar ou saturar. Não há teto arbitrário nem reserva de memória pelo máximo solicitado. Alocação acompanha linhas retornadas. Contagem não limita bytes de payload nem concorrência total; configuração operacional pertence ao chamador.

## Restauração fiel

Linha privada usa `EventEnvelope::restore(EventEnvelopeParts)` e o mesmo TryFrom da restauração Serde. EventType valida seu nome; domínio valida agregados/versão. API mínima independente do banco preserva IDs, tempos, payload e UUIDs opcionais sem chamar `new`. Espaços em campos não vazios permanecem. Versão BIGINT converte com checagem para u32 e zero é rejeitado; tentativas INTEGER convertem para u32 com checagem. Qualquer linha selecionada inválida falha o lote inteiro com InvalidStoredData, sem resultados parciais.

Decoder privado interpreta microssegundos binários desde época PostgreSQL de 2000 com aritmética Chrono verificada. Sentinelas infinity e datas finitas fora do intervalo Chrono falham; fallback textual interpreta offsets. Isso evita aritmética não verificada do SQLx 0.8.6: smoke inicial revelou panic em infinity. Decoder JSONB privado verifica versão binária 1 e rejeita JSON inválido/versões desconhecidas. serde_json arbitrary_precision preserva inteiros grandes, decimais longos e expoentes grandes sem arredondamento float; JSON null é válido. JSONB já normaliza formatação/ordem de chaves: preserva-se valor armazenado, não texto original. TIMESTAMPTZ preserva instante com precisão de microssegundos, não grafia do fuso nem nanossegundos originais.

## Erros e diagnóstico

| Falha | Categoria |
| --- | --- |
| I/O, TLS, pool fechado/timeout, worker encerrado | Unavailable |
| SQLSTATE classe 08, classe 28, 53300 e 57P01/02/03 | Unavailable |
| Decodificação selecionada, modelo inválido, timestamp/JSON incompatível, conversão numérica armazenada | InvalidStoredData |
| Outros erros, incluindo tabela ausente, permissão, cancelamento reportado pelo servidor e falhas transacionais | OperationFailed |
| Limite de pedido acima de i64 | OperationFailed antes de I/O |

Fallback conservador é OperationFailed. Nenhuma categoria promete sucesso de retry; política pertence ao chamador. Fontes SQLx/domínio/conversão permanecem tipadas. Display/Debug públicos mostram classificação estática; repositório não emite logs. Nunca registrar pools/opções, credenciais, payloads ou cadeias de fontes brutas; diagnóstico exige redação de dados sensíveis.

Testes unitários dispensam PostgreSQL. A [suíte de integração do Marco 1.7](../../../tests/postgres_repository.rs) exercita OutboxReader público contra PostgreSQL real em bancos independentes e isolados. Cobre elegibilidade/limites/desempates do adapter, todos os campos restaurados, JSON exato armazenado, observação por dois leitores sem mutação, dados selecionados inválidos e recuperação, pool fechado e tabela ausente. Display/Debug seguem sanitizados e fontes tipadas permanecem disponíveis. Veja [configuração, limpeza e limitações](../testing/strategy.md). Marco 1.8 [documenta falhas/transações](../architecture/failure-and-transactions.md); injeção de crash/recuperação permanece validação futura; escritas de produtores, claims, leases e entrega permanecem não implementados.
