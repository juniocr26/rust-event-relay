[English](../../en/adr/004-use-postgresql-transactional-outbox-schema.md) | [README](../../../README.pt-BR.md)

# ADR 004 — Usar Tabela Outbox Transacional PostgreSQL para Handoff Durável de Eventos

## Status

Aceito — 2026-10-07, Marco 1.4 (apenas schema).

## Contexto

EventEnvelope e infraestrutura de migrações existem. Produtores precisarão de handoff durável participando atomicamente da transação local de negócio. Commit no banco e publicação remota são independentes, com lacunas de falha. Entrega pelo relay é futura.

## Decisão

Introduzir relay.outbox_events por nova migração reversível SQLx, preservando arquivos anteriores. Armazenar identidade/metadados do produtor com UUID, TEXT, TIMESTAMPTZ, JSONB e UUIDs nullable. Usar BIGINT limitado para schema_version, preservando todo NonZeroU32. Adicionar TEXT pending/processed/dead_letter com CHECKs, tentativas, disponibilidade, conclusão e erro sanitizado. Sem processing ou lease durável sem protocolo de recuperação. Usar PK de identidade e um índice parcial de disponibilidade pending. Down exclui só tabela com RESTRICT, preservando namespace.

Produtores são responsáveis pela futura transação no mesmo banco com mudança de negócio e inserção outbox; relay entrega registros depois. Aplicação Rust permanece inalterada, sem ler/gravar tabela.

## Alternativas consideradas

- Publicar após commit: crash antes da publicação perde notificação.
- Publicar antes do commit: falha no banco deixa evento sem estado de negócio correspondente.
- Transação distribuída / 2PC: coordenação/disponibilidade e suporte dos participantes complicam operação; desnecessário neste estudo.
- CDC/replicação lógica: observa mudanças commitadas, mas adiciona infraestrutura operacional/replicação e contratos de mapeamento; possível exploração futura.
- Outbox transacional durável: handoff explícito versionado em uma transação local, ao custo de ciclo de vida das linhas e futura entrega/recuperação.

## Consequências

Schema neutro de transporte e voltado ao produtor; repositórios, destinos e workers ficam para marcos futuros. Migrações imutáveis controlam evolução. Metadados mínimos permitem representar retries/falhas terminais sem executá-los. Claims pending poderão usar locks transacionais; leases entre transações exigem recuperação explícita. Constraints nomeadas protegem invariantes da linha, sem impor histórico de transições.

## Trade-offs

JSONB enfraquece tipagem relacional de negócio e normaliza representação; tempo tem limites de precisão/zona. UUIDs não representam tokens externos arbitrários. BIGINT ocupa mais espaço que INTEGER, mas preserva intervalo Rust. Índices custam armazenamento/escrita; sem índices especulativos de agregado/payload. Outbox reduz inconsistência dual-write local, não garante exatamente uma vez: publicação pode duplicar, exigindo idempotência e testes de crash futuros. Down é destrutivo após dados reais; sem afirmação de recuperação sofisticada em produção ou desempenho.

Veja [schema completo e limitações](../outbox-schema.md).
