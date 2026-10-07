[English](../en/postgresql.md) | [README](../../README.pt-BR.md)

# Decisão PostgreSQL e infraestrutura local

## Implementado hoje

O Marco 1.2 adiciona apenas um serviço PostgreSQL local, configuração, healthcheck, acesso de rede e armazenamento visível no host. A imagem oficial selecionada é `postgres:18.6-bookworm`, versão estável com patch explícito sobre Debian Bookworm, em vez de `latest`. A seleção segue o [anúncio da versão 18.6](https://www.postgresql.org/about/news/postgresql-186-1711-1615-1519-1424-and-19-beta-3-released-3365/). A tag está fixada, mas o digest não; reconstruções upstream ainda podem alterar pacotes do sistema. Atualizações devem ser deliberadas, com backup e verificação de compatibilidade; trocar a versão principal não atualiza dados existentes automaticamente.

A [imagem oficial](https://hub.docker.com/_/postgres) usa `/var/lib/postgresql/18/docker` no PostgreSQL 18. Compose monta `.dockerized-postgres/` em `/var/lib/postgresql`, deixando os arquivos em `.dockerized-postgres/18/docker/`, sem volumes de dados nomeados ou anônimos. Recriar containers e executar `docker compose down` preserva esses arquivos. Esse diretório local não é uma estratégia de backup de produção.

O binário Rust ainda não lê configurações de banco nem estabelece conexões. Compose fornece `DATABASE_URL` como configuração para uma futura conexão; nenhuma dependência Rust de banco ou tipo de configuração sem uso foi adicionado. `/health` continua indicando liveness HTTP. `depends_on: service_healthy` condiciona a partida do container de desenvolvimento, enquanto `pg_isready` verifica aceitação pelo servidor, não autenticação da aplicação, disponibilidade do schema ou readiness contínua após a partida.

## Útil para a arquitetura pretendida

PostgreSQL se adequa à outbox transacional pretendida por suas transações ACID e controle maduro de concorrência. Uma mudança de negócio e um registro outbox podem confirmar atomicamente quando o produtor grava ambos na **mesma transação local do banco PostgreSQL**. Isso não abrange bancos distintos ou um broker remoto. PostgreSQL oferece SQL robusto, índices, bloqueio por linha e comportamento transacional confiável; nenhuma dessas capacidades foi integrada à persistência da aplicação. Consulte [transações](https://www.postgresql.org/docs/18/tutorial-transactions.html) e [bloqueios](https://www.postgresql.org/docs/18/explicit-locking.html).

Fronteira futura apenas conceitual (não existem tabelas ou migrações):

```text
BEGIN
update business_state ...
insert into outbox_events ...
COMMIT
```

Isso aborda a escrita dupla insegura: confirmar a mudança de negócio, publicar uma mensagem e perder a publicação se o produtor falhar entre essas ações. O relay futuro processará registros outbox duráveis de forma assíncrona. Publicação e confirmação ainda podem falhar independentemente; consumidores precisam de idempotência e não há promessa de entrega exatamente uma vez.

JSON/JSONB podem acomodar o payload do envelope; JSONB oferece índices, mas altera algumas propriedades da representação textual. O schema e a estratégia de índices continuam indefinidos. Consulte [tipos JSON](https://www.postgresql.org/docs/18/datatype-json.html). `FOR UPDATE SKIP LOCKED` é um mecanismo possível para reivindicação futura por workers, permitindo pular linhas ocupadas; não está implementado e não oferece ordenação estrita nem justiça. Ferramentas maduras, clientes SQL, logs e views do sistema tornam inspeção e monitoramento operacional familiares; o monitoramento de produção não foi configurado.

## Trade-offs

- Operação com estado exige gerenciamento de armazenamento, backups em implantações reais, gerenciamento de conexões, evolução de schema e monitoramento.
- Um único banco tem capacidade finita. Alto volume de escrita, índices ruins ou padrões de reivindicação podem gerar contenção; escala vertical não é ilimitada.
- A futura outbox durável exige limpeza de eventos processados, retenção/arquivamento, desenho de índices, monitoramento do crescimento das tabelas, comportamento de vacuum e planejamento de capacidade.
- Workers concorrentes podem disputar linhas. Bloqueios e SKIP LOCKED ajudam na coordenação, mas exigem transações curtas, regras de recuperação e decisões explícitas sobre justiça/ordenação.
- Semântica específica do PostgreSQL reduz portabilidade. Portabilidade de banco não é um objetivo atual; é um trade-off deliberado, não uma alegação de superioridade universal.

Consulte [ADR 002](adr/002-use-postgresql-for-durable-event-storage.md), [Docker e DBeaver](docker-and-configuration.md), [checks de infraestrutura](testing.md) e [validação executada](validation-results.md).
