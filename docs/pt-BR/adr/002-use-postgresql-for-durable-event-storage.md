[English](../../en/adr/002-use-postgresql-for-durable-event-storage.md) | [README](../../../README.pt-BR.md)

# ADR 002 — Usar PostgreSQL para armazenamento durável de eventos

Status: aceita. Data: 2026-10-07.

## Contexto

O relay pretendido precisa de registros duráveis de eventos e de uma fronteira transacional no produtor que evite escrita dupla insegura entre banco e mensagens. Hoje existem apenas o envelope canônico e a base da aplicação; este marco estabelece infraestrutura local.

## Decisão

Selecionar PostgreSQL para armazenamento durável futuro e outbox transacional. Iniciar o desenvolvimento local com `postgres:18.6-bookworm`, acesso pelo loopback do host em 5433, porta interna 5432 e bind mount físico `.dockerized-postgres/`. Usar healthcheck do servidor sem adicionar persistência à aplicação. Consulte a [decisão detalhada](../database/postgresql.md).

## Alternativas consideradas

MySQL/MariaDB também oferecem transações relacionais; concorrência, JSONB e ferramentas do PostgreSQL se adequam à exploração pretendida. SQLite simplifica implantação, mas seu modelo de escritor único se alinha menos aos workers concorrentes em rede pretendidos. Bancos de documentos têm outras fronteiras transacionais e de modelagem. Durabilidade apenas no broker não torna, por si só, uma alteração de banco atômica com a publicação. Armazenamento em memória não sobrevive à perda do processo. Não houve benchmarks comparativos e PostgreSQL não é universalmente superior.

## Consequências

O produtor futuro precisa gravar estado de negócio e outbox na mesma transação local para obter atomicidade. A entrega pelo relay continua assíncrona, com risco de duplicatas. Arquivos locais sobrevivem à recriação de containers e exigem reset deliberado; produção exige backups, monitoramento e desenho de implantação próprio. Migrações, tabelas, repositórios e reivindicações por workers ficam para depois.

## Trade-offs

Operação com estado, capacidade vertical limitada, contenção de escritas/workers, retenção de outbox crescente e custos de vacuum precisam ser gerenciados. Bloqueios específicos do PostgreSQL podem reduzir portabilidade; portabilidade não é um objetivo atual. SKIP LOCKED pode ajudar nas reivindicações concorrentes, mas não promete justiça ou ordenação global. São considerações futuras, não comportamento implementado.
