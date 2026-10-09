[English](../en/outbox-schema.md) | [README](../../README.pt-BR.md)


## Extensão atual — Marco 2.2

[Estado de entrega e posse](milestone-2-2.md) e [ADR 007](adr/007-durable-delivery-ownership.md) definem recuperação por lease durável e contratos separados de adquirir/concluir/liberar. Migração nova `20261009000000_add_delivery_ownership` adiciona token/acquired_at/expires_at nullable com lease coerente apenas em pending. Seções de marcos anteriores abaixo descrevem escopo original; afirmações antigas de posse/contador indefinidos são substituídas pelo ADR 007. Adapters produtivos de mutação ficam para 2.3; SELECT do reader e publisher preservados.
# Schema outbox — Marco 1.4

## Objetivo e responsabilidade

`relay.outbox_events` é a representação relacional durável inicial de EventEnvelope e metadados mínimos do relay. Este marco define apenas schema: o binário Rust não insere nem processa eventos. Produtores são responsáveis pela identidade e deverão escrever estado de negócio e evento outbox na **mesma transação PostgreSQL local**. O relay posteriormente lê e entrega registros duráveis. Banco não gera IDs nem timestamps de ocorrência substituindo valores do produtor.

Dual write pode perder evento se estado de negócio confirma e produtor cai antes de publicar; publicar primeiro pode expor evento cuja transação depois falha. Registrar ambas mudanças duráveis em uma transação local elimina essa lacuna de handoff. Entrega ainda tem janelas de falha entre publicação/confirmação e possíveis duplicatas; futuro processamento pelo menos uma vez exige idempotência dos consumidores. Schema sozinho não executa transações, claims ou garantias desse fluxo.

```text
BEGIN
  atualizar estado de negócio
  inserir relay.outbox_events (...)
COMMIT
```

## Colunas e mapeamento do envelope

| Coluna | Tipo PostgreSQL | Aceita NULL | Padrão | Semântica |
| --- | --- | --- | --- | --- |
| id | UUID | Não | Nenhum | Identidade gerada pelo produtor; chave primária |
| event_type | TEXT | Não | Nenhum | Nome lógico definido pela aplicação |
| aggregate_type | TEXT | Não | Nenhum | Tipo genérico do agregado de origem |
| aggregate_id | TEXT | Não | Nenhum | Identificador genérico; UUID, inteiro, ULID, string externa ou legada |
| schema_version | BIGINT | Não | Nenhum | Intervalo completo NonZeroU32, 1..4294967295 |
| occurred_at | TIMESTAMPTZ | Não | Nenhum | Instante de ocorrência do evento de negócio |
| correlation_id | UUID | Sim | NULL | Metadado opcional de correlação |
| causation_id | UUID | Sim | NULL | Identidade opcional do evento/comando causador |
| payload | JSONB | Não | Nenhum | Valor JSON específico, incluindo JSON null |
| created_at | TIMESTAMPTZ | Não | now() | Timestamp da transação de inserção outbox |
| status | TEXT | Não | pending | Estado do ciclo de vida do relay |
| attempt_count | INTEGER | Não | 0 | Contador não-negativo de tentativas de infraestrutura |
| available_at | TIMESTAMPTZ | Não | now() | Instante mínimo de elegibilidade para processamento futuro |
| processed_at | TIMESTAMPTZ | Sim | NULL | Instante de conclusão bem-sucedida do relay |
| last_error | TEXT | Sim | NULL | Diagnóstico operacional futuro sanitizado |

As primeiras nove colunas são o evento canônico; as seis restantes são metadados de infraestrutura. Mapeamento: Uuid → UUID; EventType/String → TEXT; NonZeroU32 → BIGINT limitado; DateTime<Utc> → TIMESTAMPTZ; Value → JSONB; Option<Uuid> → UUID nullable. O envelope Rust permanece inalterado.

**BIGINT é deliberado:** INTEGER PostgreSQL termina em 2147483647 e não armazena todo u32 válido. CHECK positivo/limitado preserva 1..4294967295 sem reduzir silenciosamente o modelo. Repositórios futuros devem converter valor assinado com checagem de limites. Veja [intervalos inteiros](https://www.postgresql.org/docs/18/datatype-numeric.html).

Nomes de eventos permanecem flexíveis (order.created, payment.completed, inventory.reserved); sem enum PostgreSQL ou migração para cada tipo lógico. IDs de agregados não têm restrição UUID nem limite arbitrário de tamanho. Correlação/causalidade UUID correspondem aos IDs atuais, mas integrações podem precisar de tokens não-UUID; esse trade-off continua documentado e inalterado.

`occurred_at` é tempo de negócio, `created_at` é inserção e `processed_at` é conclusão bem-sucedida do relay. `now()` é início da transação: defaults created_at/available_at correspondem na mesma transação, sem garantia de amostragem de relógio por linha. TIMESTAMPTZ preserva instantes, não zona textual original; saída depende da timezone da sessão. Precisão PostgreSQL em microssegundos não preserva nanossegundos arbitrários Chrono. Persistência futura deve explicitar precisão/instantes finitos. Veja [tipos de data/hora](https://www.postgresql.org/docs/18/datatype-datetime.html).

JSONB valida sintaxe e aceita objetos, arrays, escalares e JSON null; SQL NULL é rejeitado. Permite consultas/índices futuros, com tipagem relacional mais fraca e risco de acoplar infraestrutura ao schema de negócio. Não valida schema do payload. Normaliza formatação/ordem de chaves e não preserva chaves duplicadas; restrições text/JSONB como caractere zero impedem armazenar toda string Rust sem alteração. Persistência futura deve expor esses erros. Nenhum índice GIN/de expressão especulativo existe. Veja [tipos JSON](https://www.postgresql.org/docs/18/datatype-json.html).

## Invariantes duráveis

| Invariante | Rust | PostgreSQL |
| --- | --- | --- |
| Strings evento/agregado não vazias | Sim; rejeita também somente whitespace Unicode | CHECKs rejeitam strings exatamente vazias |
| Versão positiva dentro de u32 | NonZeroU32 | CHECK de intervalo BIGINT |
| Campos obrigatórios do envelope | Construção/restauração tipada | NOT NULL |
| Metadados UUID opcionais | Option<Uuid> | UUID nullable |
| Identidade persistida única | Gera UUID v7; sem registro global de unicidade | Chave primária UUID |
| Tentativas não-negativas e ciclo válido | Sem comportamento relay | CHECKs nomeados |
| Timestamp de conclusão se e somente se processed | Sem comportamento relay | CHECK nomeado |

CHECKs: `ck_outbox_events_event_type_non_empty`, `ck_outbox_events_aggregate_type_non_empty`, `ck_outbox_events_aggregate_id_non_empty`, `ck_outbox_events_schema_version_positive`, `ck_outbox_events_attempt_count_non_negative`, `ck_outbox_events_status` e `ck_outbox_events_processed_at`. PK: `pk_outbox_events`. Strings SQL somente whitespace ficam deliberadamente para validação do produtor/aplicação; reproduzir trim Unicode completo Rust em SQL traz complexidade desnecessária. Banco não reescreve identificadores.

## Ciclo de vida e claims futuros

`pending` inclui trabalho inicial e retries agendados; `available_at <= now()` descreve elegibilidade, não consulta implementada. `processed` registra conclusão e exige processed_at. `dead_letter` representa falha terminal sem tabela DLQ separada. Pending/dead_letter exigem processed_at NULL. attempt_count e last_error permitem futuras tentativas/inspeção; nenhum código incrementa ou preenche esses campos. Diagnósticos devem ser sanitizados, evitando payloads sensíveis e stack traces desnecessários.

Sem estado failed separado: falha recuperável permanece pending com disponibilidade futura; esgotamento/falha permanente poderá virar dead_letter. Sem processing: mudar status sozinho não recupera worker que caiu. Workers futuros poderão coordenar locks transacionais com FOR UPDATE SKIP LOCKED; claim atravessando I/O remoto/fronteiras transacionais exigiria propriedade, expiração/recuperação explícitas e possivelmente nova migração. Sem SQL de claim, leases, loops ou imposição de transições. CHECKs validam formato da linha, não sequência legal de transições ou incrementos.

## Índices e ordenação

- `pk_outbox_events`: B-tree único em id para busca e prevenção de duplicatas. UUID v7 do produtor pode melhorar localidade versus IDs aleatórios, mas não garante ordem de negócio nem substitui occurred_at.
- `ix_outbox_events_pending_available`: B-tree parcial `(available_at, created_at, id) WHERE status = 'pending'`. Suporta corte de elegibilidade e polling ordenado/limitado com desempate determinístico; exclui estados terminais. Não reivindica trabalho nem garante justiça de agendamento.

Índices adicionam armazenamento e amplificação de escrita; mudança de status mantém participação no parcial. Sem índices especulativos para agregado, ocorrência, correlação ou payload. Índices por agregado/chave virão apenas com semântica de workers e padrões medidos. Sem afirmação de throughput ou prontidão de produção.

## Migração e rollback

Novo par com histórico imutável:

```text
20261007175358_create_outbox_events.up.sql
20261007175358_create_outbox_events.down.sql
```

Up cria tabela, CHECKs e um índice de polling no namespace relay existente. Down exclui apenas tabela com RESTRICT; índices/constraints próprios são removidos junto. Preserva relay/histórico anterior e recusa dependências externas. SQLx executa migrações PostgreSQL transacionalmente por padrão. Após dados reais, rollback perde eventos permanentemente: inspecione/faça backup; correção para frente pode ser mais segura. Não é reset do banco.

Veja [validação e comandos](testing.md#validação-do-schema-outbox--marco-14), [resultados reais](validation-results.md), [responsabilidade das migrações](database-migrations.md) e [ADR 004](adr/004-use-postgresql-transactional-outbox-schema.md). Sem colunas de destino/roteamento, repositório, publicador, retries executados, tabela DLQ ou workers.
