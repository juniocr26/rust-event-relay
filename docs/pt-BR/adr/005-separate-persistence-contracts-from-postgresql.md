[English](../../en/adr/005-separate-persistence-contracts-from-postgresql.md) | [README](../../../README.pt-BR.md)

# ADR 005 — Separar Contratos de Persistência do Relay da Infraestrutura PostgreSQL

## Status

Aceito — 2026-10-07, Marco 1.5.

## Contexto

Envelope, schema durável e migrações existem. Aplicação futura precisa de semântica explícita sem SQLx/linhas do banco. Relay standalone não controla transações de negócio do produtor. Schema tem pending/processed/dead_letter, sem processing/lease durável; leitura não pode ser divulgada como claim.

## Decisão

Adicionar OutboxReader focado somente leitura, BatchSize positivo, EligibleRead UTC explícito e snapshots PendingOutboxEvent. Separar identidade de evento de tentativas unsigned/disponibilidade UTC. Leitura limitada sem efeitos e com snapshot consistente, sem exclusividade ou ordem de negócio. Adiar mutações/claims até resolver autoridade, concorrência, atomicidade e repetição.

Sem contrato writer: append standalone não é atômico com transação de outra aplicação. Sem tipos terminais ou erros de duplicata/conflito sem uso. Preservar fontes em três erros classificados com Display/Debug sanitizados. Usar impl Future + Send nativo e dispatch genérico/estático, evitando async-trait, thiserror, driver e futures boxed. Adapters PostgreSQL futuros dependem para dentro do contrato. Sem migração nova ou conexão de runtime.

## Alternativas consideradas

- SQLx direto na aplicação: menos tipos, mas mistura driver, ciclo e responsabilidades da aplicação.
- Repositório CRUD genérico: reutilização superficial que oculta elegibilidade, claims e limites transacionais do produtor.
- API PostgreSQL específica: expõe locks/transações, acopla chamador ao driver e não resolve propriedade sozinha.
- Port completo writer/claim/ciclo agora: congela decisões de transação/recuperação abertas, especialmente confirmação insegura só por ID.
- Contrato focado de aplicação: observação útil/precisa agora, extensão deliberada após evidência do adapter.

## Consequências

Testes usam fake de uma resposta em código async genérico sem banco. Demonstra substituição e Send, não durabilidade/locking. Runtime não usa reader. Duplicatas do produtor, recuperação de claims e idempotência de mutação ficam abertas; fontes exigem remoção de segredos ao inspecionar. Chamadores configurarão limites; adapter fará conversão verificada/restauração fiel.

## Trade-offs

Abstração adiciona tipos/interface; Send/dispatch estático públicos afetam compatibilidade e excluem dyn sem redesenho. Esconder driver pode ocultar semântica, tratado por garantias/exclusões explícitas. Modelo é visão pending, não todas colunas SQL; visões/tokens futuros evoluem conforme necessidade. Outros adapters são possíveis, mas portabilidade/desempenho não são objetivos ou alegações validadas. API standalone não produz atomicidade do negócio do produtor.

Veja [semântica, modelos, questões abertas e diagramas](../persistence-abstraction.md).
