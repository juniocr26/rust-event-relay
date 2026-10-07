[English](../en/postgresql.md) | [README](../../README.pt-BR.md)

# Decisão PostgreSQL e infraestrutura local

## Implementado hoje

O Marco 1.2 adiciona apenas um serviço PostgreSQL local, configuração, healthcheck, acesso de rede e armazenamento visível no host. A imagem oficial selecionada é `postgres:18.6-bookworm`, versão estável com patch explícito sobre Debian Bookworm, em vez de `latest`. A seleção segue o [anúncio da versão 18.6](https://www.postgresql.org/about/news/postgresql-186-1711-1615-1519-1424-and-19-beta-3-released-3365/). A tag está fixada, mas o digest não; reconstruções upstream ainda podem alterar pacotes do sistema. Atualizações devem ser deliberadas, com backup e verificação de compatibilidade; trocar a versão principal não atualiza dados existentes automaticamente.

A [imagem oficial](https://hub.docker.com/_/postgres) usa `/var/lib/postgresql/18/docker` no PostgreSQL 18. Compose monta `.dockerized-postgres/` em `/var/lib/postgresql`, deixando os arquivos em `.dockerized-postgres/18/docker/`, sem volumes de dados nomeados ou anônimos. Recriar containers e executar `docker compose down` preserva esses arquivos. Esse diretório local não é uma estratégia de backup de produção.

O binário Rust ainda não lê configurações de banco nem estabelece conexões. O wrapper SQLx constrói `DATABASE_URL` com configurações Compose para comandos de migração; nenhuma dependência Rust de banco ou tipo de configuração sem uso foi adicionado. `/health` continua indicando liveness HTTP. `depends_on: service_healthy` condiciona a partida do container de desenvolvimento, enquanto `pg_isready` verifica aceitação pelo servidor, não autenticação da aplicação, disponibilidade do schema ou readiness contínua após a partida.

## Útil para a arquitetura pretendida

PostgreSQL se adequa à outbox transacional pretendida por suas transações ACID e controle maduro de concorrência. Uma mudança de negócio e um registro outbox podem confirmar atomicamente quando o produtor grava ambos na **mesma transação local do banco PostgreSQL**. Isso não abrange bancos distintos ou um broker remoto. PostgreSQL oferece SQL robusto, índices, bloqueio por linha e comportamento transacional confiável; nenhuma dessas capacidades foi integrada à persistência da aplicação. Consulte [transações](https://www.postgresql.org/docs/18/tutorial-transactions.html) e [bloqueios](https://www.postgresql.org/docs/18/explicit-locking.html).

Fronteira futura apenas conceitual (não existem tabelas da aplicação):

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

## Inicialização e reset deliberado

POSTGRES_DB, POSTGRES_USER e POSTGRES_PASSWORD são principalmente **variáveis de inicialização** da imagem oficial. Inicializam usuários/banco/senha apenas quando o diretório de dados do cluster está vazio. Mudar `.env`, reconstruir ou recriar containers não altera usuários, bancos ou senhas PostgreSQL existentes.

```text
Ambiente Docker != usuários/bancos já criados no PostgreSQL
```

Exemplo: inicialize com `first_user`, depois mude `.env` para `second_user`. O ambiente do container mostra second_user, mas o cluster persistido mantém first_user, causando `FATAL: role "second_user" does not exist`. Mudar só a senha também pode causar falha de autenticação. `pg_isready` pode continuar saudável; valide SQL autenticado por TCP, não apenas variáveis. Não imprima senhas para depurar.

Mantenha credenciais originais ou administre usuários/bancos deliberadamente via SQL com acesso autorizado existente. No desenvolvimento inicial, recriar o cluster intencionalmente é outra opção, após backup do necessário. Nenhuma operação Compose ou entrypoint exclui ou reinicializa `.dockerized-postgres/` automaticamente.

**AVISO: isto exclui permanentemente o estado do banco PostgreSQL local.** Os comandos manuais exigem decisão explícita de descartar o cluster; nunca executam automaticamente. Execute na raiz do repositório e faça backup do necessário primeiro.

```bash
# Somente após decidir explicitamente descartar todo estado local do banco:
docker compose down
rm -rf .dockerized-postgres/
docker compose up -d --build --wait --wait-timeout 120
docker compose exec app sqlx migrate run
```

Reset exclui todos os bancos, usuários e dados do cluster; rollback executa SQL down controlado em um banco. **Reset do banco != rollback de migração.** Veja [migrações](database-migrations.md).

## Expansão do shell e clientes externos

Aspas simples impedem expansão no shell do host; `sh -c` expande variáveis dentro do container onde o Compose as forneceu:

```bash
docker compose exec postgres sh -c 'psql -U "$POSTGRES_USER" -d "$POSTGRES_DB"'
docker compose exec -T postgres sh -c 'PGPASSWORD="$POSTGRES_PASSWORD" psql -h postgres -p 5432 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT 1;"'
```

O primeiro usa socket Unix local e pode não validar senha; o segundo autentica via TCP. Escrever `-U "$POSTGRES_USER"` diretamente no comando do host expande variáveis do host, que podem estar ausentes ou diferentes. Valores do ambiente ainda precisam coincidir com o cluster inicializado.

Conexão externa DBeaver:

```text
Host: 127.0.0.1
Porta: 5433 (ou POSTGRES_HOST_PORT)
Banco: valor de POSTGRES_DB
Usuário: valor de POSTGRES_USER
Senha: valor de POSTGRES_PASSWORD
```

Use 127.0.0.1 para corresponder à publicação IPv4 deliberada e evitar ambiguidade IPv4/IPv6 de localhost. Containers usam postgres:5432; clientes do host usam 127.0.0.1:5433. Valores do `.env` precisam corresponder às credenciais persistidas, não apenas ao ambiente atual.

## Responsabilidade pelo schema

[migrations/](../../migrations/) contém o histórico SQL canônico versionado, gerenciado pelo SQLx CLI 0.8.6 no Docker. DBeaver serve para inspeção, consultas e depuração; mudanças pretendidas pertencem a migrações. Dados físicos `.dockerized-postgres/18/docker/` são estado local ignorado e sobrevivem a up, down, build e recriação. Rollback não exclui esse diretório. Marco 1.3 cria apenas namespace relay vazio; sem tabelas da aplicação ou persistência.
