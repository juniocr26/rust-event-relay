[English](../../en/adr/003-use-versioned-sql-migrations.md) | [README](../../../README.pt-BR.md)

# ADR 003 — Usar Migrações SQL Versionadas para Evolução do Schema PostgreSQL

## Status

Aceito — 2026-10-07, Marco 1.3.

## Contexto

PostgreSQL local persiste o cluster após recriar containers. Reconstruir o schema pretendido exige histórico revisado em código-fonte, não mudanças manuais na interface gráfica. Persistência e outbox pertencem a marcos posteriores. Usuários persistidos também sobrevivem a mudanças das variáveis de inicialização; rollback de schema não corrige essa divergência.

## Decisão

Usar SQLx CLI 0.8.6 como ferramenta Docker de desenvolvimento, fixado com seu lockfile distribuído e apenas features rustls/PostgreSQL. Manter SQL reversível com timestamp em `migrations/`. Sem dependência Rust de runtime ou migração automática na partida. Compose condiciona startup do app ao health PostgreSQL; comandos de migração são explícitos.

Criar namespace vazio `relay` na primeira migração de infraestrutura (Opção B), reservando uma fronteira explícita para futuros objetos qualificados. Down usa RESTRICT. A tabela de metadados SQLx é controle de infraestrutura. Nenhuma tabela da aplicação é criada. Um wrapper Python com biblioteca padrão aplica percent-encoding seguro à URL interna em cada chamada.

## Alternativas consideradas

- SQL manual/mudanças DBeaver: experimentação fácil, mas pouca reprodutibilidade e nenhum histórico canônico revisável.
- Sincronização de schema gerada por ORM: acopla modelos e ferramenta, pode ocultar SQL exato; desnecessária sem modelos de persistência.
- Criação pela aplicação: acopla ciclo de vida e privilégios de schema, complica falhas de startup e concorrência; prematura aqui.
- Migrações SQL versionadas: semântica PostgreSQL explícita, SQL inspecionável, checksums e fluxo reversível local. SQLx CLI combina com Rust/Docker sem vincular SQLx à aplicação.

## Consequências

Migrações controlam evolução do schema e são commitadas com código. Histórico compartilhado aplicado é imutável por padrão. DBeaver serve para inspeção, consultas e depuração. Dados físicos, cache Cargo e artefatos continuam ignorados. Clusters novos exigem aplicação explícita das migrações. Credenciais de inicialização são estado separado; reset exclui todo estado local apenas por ação deliberada do desenvolvedor.

## Trade-offs

Builds Docker ficam mais longos e incluem Python para encoding confiável da URL. SQL precisa ser escrito e revisado, inclusive rollback. Cada migração PostgreSQL é transacional por padrão, com exceções exigindo tratamento explícito sem transação. Down não restaura todos os dados perdidos nem garante recuperação em produção. Namespace vazio é fronteira arquitetural, não persistência outbox. Fixar versão/lockfile não torna reconstruções de imagens e pacotes upstream imutáveis.

Veja [fluxo e limitações](../database/migrations.md).
