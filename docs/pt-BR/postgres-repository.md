[English](../en/postgres-repository.md) | [README](../../README.pt-BR.md)

# Repositório PostgreSQL — Marco 1.6

`PostgresOutboxRepository::new(pool)` recebe `PgPool` injetado. O chamador constrói/configura o pool, por exemplo com `PgPoolOptions::connect_with` e `PgConnectOptions`, e controla seu encerramento. Métodos de consulta não leem ambiente nem criam conexões globais. Bootstrap HTTP continua sem banco. Infraestrutura implementa `OutboxReader` existente e depende de persistência/domínio; SQLx não entra nesses módulos. `StoredEvent` e decodificação são privados. Modelos validam/convertem dados sem consultas. ADR 005 permanece suficiente e inalterado.

SQLx 0.8.6 é fixado na mesma versão do CLI Docker, com defaults desabilitados e somente postgres, runtime-tokio, uuid, chrono e json. Rust 1.95.0 é a versão validada. Não há macros, metadados offline, migrações pela aplicação ou backend TLS. Conexão Docker local funciona sem TLS; implantação que exigir TLS precisará habilitá-lo/configurá-lo deliberadamente. Veja [documentação SQLx](https://docs.rs/sqlx/0.8.6/sqlx/).

## Consulta e limites

Uma consulta parametrizada seleciona colunas explícitas do envelope e attempt_count/available_at em `relay.outbox_events`. Filtra `status = 'pending' AND available_at <= $1`, ordena por `available_at, created_at, id`, conforme índice pending, e limita com `LIMIT $2`. Uma instrução observa um snapshot PostgreSQL. Ordem é detalhe de implementação, sem garantia de ordem de entrega. Não há locks, claims, reservas, gravações ou segunda consulta. Leitores podem receber os mesmos eventos; metadados podem ficar desatualizados imediatamente.

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
| Outros erros, incluindo tabela ausente, permissão, cancelamento e falhas transacionais | OperationFailed |
| Limite de pedido acima de i64 | OperationFailed antes de I/O |

Fallback conservador é OperationFailed. Nenhuma categoria promete sucesso de retry; política pertence ao chamador. Fontes SQLx/domínio/conversão permanecem tipadas. Display/Debug públicos mostram classificação estática; repositório não emite logs. Nunca registrar pools/opções, credenciais, payloads ou cadeias de fontes brutas; diagnóstico exige redação de dados sensíveis.

Testes unitários dispensam PostgreSQL. [Smoke opt-in](../../tests/postgres_read_smoke.rs) cria banco isolado e usa fixtures fora da API produtiva: leitura vazia, corte/status, limite, identidade/tempo/JSON/UUID, repetição sem mutação e rejeição de timestamp/modelo inválidos e preservação numérica JSON exata. Veja [execução e limpeza](testing.md). Não conclui suíte completa do Marco 1.7 nem análise ampla de falhas/concorrência/transações do 1.8. Escritas, leases, transições, entrega e garantia exatamente uma vez permanecem fora do escopo.
