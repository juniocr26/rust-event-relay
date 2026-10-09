[English](../en/milestone-2-2.md) | [README](../../README.pt-BR.md)

# Marco 2.2 — Estado de entrega e posse de eventos

Implementado em 2026-10-09 a partir de `5eac9ff` limpo. Sem AGENTS.md aplicável ao projeto (instruções no cache de dependências não governam os arquivos do projeto). Sem commit/push. Registros dos marcos 1/2.1 são históricos; veja [validação nova](validation-results.md#marco-22--2026-10-09).

[ADR 007](adr/007-durable-delivery-ownership.md) escolhe lease durável e transações curtas em vez de reter conexão/locks durante I/O no broker. Snapshot é separado da autoridade: token identifica aquisição, ID identifica envelope imutável. Token no banco não impede publicação RabbitMQ nem oferece exactly-once.

## Tipos e contratos implementados

| Arquivo | Objetivo |
| --- | --- |
| src/domain/delivery.rs | Duração/token/lease validados, metadados pending/processed/dead_letter, regras puras de adquirir/concluir/liberar |
| src/persistence/ownership.rs | AcquireRequest de uso único, OwnedEventKey, OwnedOutboxEvent e contratos separados OutboxAcquirer/Completer/Releaser |
| src/persistence/error.rs | CommitUncertain para futuros adapters de mutação; mapeamento do reader preservado |
| migrations/20261009000000_add_delivery_ownership.*.sql | Campos nullable de posse e constraint coerente; reversível sem remover eventos |
| tests/delivery_ownership.rs | Invariantes, expiração, dono antigo, limites, identidade e futures Send/despacho estático |
| tests/delivery_ownership_schema.rs | PostgreSQL isolado: migração, preservação, constraints, reader e rollback |
| .github/workflows/ci.yaml | Novo teste de schema no job PostgreSQL; sem alegar execução remota |
| tests/lifecycle.rs | Verificação assíncrona limitada de listener indisponível após duas falhas da sondagem síncrona imediata de shutdown |
| tests/persistence_contract.rs | Cobertura de erro/fonte sanitizada estendida a CommitUncertain |

Campos privados/restauração verificada mantêm invariantes sem SQLx/Lapin/linhas SQL nos contratos/domínio. AcquireRequest gera token novo e não implementa Clone/Copy; caller pode guardar token para diagnóstico antes de consumir a requisição. OwnedEventKey carrega ID e token esperado, sem prometer lease ainda ativo. OwnedOutboxEvent exige contador positivo limitado e disponibilidade até aquisição. DeliveryState puro não contém envelope nem regenera ID. Modela regras, não repositório produtivo ou caso de uso de orquestração.

## Ciclo

| Operação | Predicado com instante do banco após lock da linha | Resultado atômico |
| --- | --- | --- |
| Adquirir | pending, disponibilidade vencida, lease ausente ou expires_at <= instante, contador < i32::MAX | Novo token, acquired_at=instante, expires_at=instante+duração, contador+1; permanece pending |
| Concluir | pending, ID/token correspondentes, acquired_at <= instante < expires_at | processed, processed_at=instante, todos campos de posse NULL |
| Liberar | Mesmo predicado de posse | pending, posse NULL; disponibilidade/contador preservados |

Posse começa com aquisição commitada conhecida. No limite exato da expiração o dono perde autoridade mesmo sem nova aquisição. Pending expirado pode ser readquirido diretamente, substituindo token; sem reaper/estado adicional. Terminais não têm lease; apenas processed tem processed_at. Políticas de terminal/replay continuam ausentes. Constraints de forma não comprovam histórico legal de transições.

Attempt_count conta aquisições commitadas, incluindo crash antes da publicação. Valores históricos existentes são preservados. Overflow retorna AttemptLimit no modelo; aquisição futura exclui candidatos esgotados, sem wrap/saturação. Resultado vazio não prova backlog vazio. Available_at é primeira elegibilidade de agendamento; liberar não agenda retry. Acquired_at/expires_at descrevem aquisição, não criação do evento. Processed_at registra conclusão no banco após aceitação confirmada pelo broker, nunca trabalho do consumidor. PostgreSQL é autoridade, com uma amostra clock_timestamp() após lock de linha; duração/lease em microssegundos e soma verificada. Relógio do caller não autoriza gravações.

OwnershipLost é resultado normal distinto de erro sanitizado de infraestrutura. Repetição de conclusão/liberação retorna OwnershipLost. COMMIT desconhecido é CommitUncertain, não rollback conhecido; cancelamento após despacho também pode deixar incerteza. Aquisição não confirmada não autoriza publicar; expiração permite recuperar. Conclusão/liberação incerta exige reconciliação ou recuperação, sem presumir sucesso/posse retida. Só aceitação confirmada autoriza chamada futura de conclusão; todo PublishErrorKind impede sucesso. Aceitação seguida de conclusão falha/incerta pode duplicar na recuperação mantendo ID original.

## Compatibilidade e limite

Migrações antigas preservadas. Linhas antigas recebem três campos NULL sem perda de identidade/metadados/contador. Campos todos ausentes ou coerentes: token não nulo, lease finito, pending, contador positivo, available_at <= acquired_at < expires_at. Constraint terminal de timestamp permanece. Índice parcial de disponibilidade serve ao scan planejado com filtros residuais de expiração/contador; sem índice especulativo. 2.3 deve medir desempenho e testar linhas bloqueadas.

Reader continua observando pending por disponibilidade, inclusive linhas com lease/esgotadas, sem conferir posse ou modificá-la. Publisher e runtime HTTP preservados. Dados compartilhados não são migrados para validação. Teste isolado aplica schema original, insere linha antiga, compara todas colunas anteriores, aplica migração nova, verifica constraints/reader imutável/rollback. Harness fecha pools, remove seu banco gerado exato e verifica ausência. Suíte PostgreSQL antiga conserva fixtures das duas migrações originais para validar reader compatível com schema anterior.

**2.3:** operações PostgreSQL produtivas de adquirir/concluir/liberar com transações limitadas, relógio/predicados após lock, seleção SKIP LOCKED, contadores verificados, retorno após commit, conflitos/erros/commit incerto e integração real de donos concorrentes, expiração durante espera e rejeição de dono antigo. Modelos/fakes não comprovam concorrência do banco. **2.4:** adquirir → publicar → persistir; **2.5:** polling/composição/reconexão/programa relay Supervisor; **2.6:** crashes/recuperação ponta a ponta. Sem agenda de retries, backoff, quarentena ou política dead-letter. Sem endpoint produtor, CDC ou alegação de entrega ponta a ponta.

## Comandos de validação

Inspecione serviços existentes com docker compose ps. Cargo usa app existente; não aplicar migrações compartilhadas apenas para testar.

```bash
docker compose exec -T app cargo fmt --check
docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings
docker compose exec -T app cargo test --locked
docker compose exec -T app cargo test --locked --test delivery_ownership_schema -- --ignored
docker compose exec -T app cargo test --locked --test postgres_repository -- --ignored --test-threads=4
docker compose exec -T app cargo test --locked --test rabbitmq_publisher -- --ignored
docker compose exec -T app cargo test --locked --lib infrastructure::rabbitmq::tests::closed_owned_connection_is_unavailable -- --ignored
docker compose exec -T app cargo build --locked
docker compose exec -T -e RUSTDOCFLAGS='-D warnings' app cargo doc --locked --no-deps
git diff --check
```

Harness exige credenciais explícitas e CREATE DATABASE; falha em vez de pular integração solicitada. Morte abrupta pode deixar banco isolado impresso: remover só nome exato próprio após inspeção. Veja [testes](testing.md) e [resultados executados](validation-results.md).
