[English](../en/database-migrations.md) | [README](../../README.pt-BR.md)

# Migrações de banco — Marco 1.3

## Objetivo e ferramenta

Migrações SQL versionadas são responsáveis pela evolução do schema PostgreSQL. SQLx CLI **0.8.6** é instalado na imagem Docker de desenvolvimento com `cargo install sqlx-cli --version 0.8.6 --locked --no-default-features --features rustls,postgres --root /opt/sqlx`. Não exige Cargo no host nem nova dependência Rust da aplicação. A versão do CLI e seu lockfile de dependências distribuído são fixados; tags das imagens e revisões de pacotes Debian não são digests imutáveis. Reconstrua com `docker compose up -d --build --wait --wait-timeout 120`.

`docker/sqlx.py`, instalado como `sqlx`, constrói DATABASE_URL a cada chamada com POSTGRES_USER/PASSWORD/DB/HOST/PORT injetados pelo Compose. Aplica percent-encoding a usuário, senha e banco e passa a URL apenas no ambiente do processo filho. Exige a topologia interna `postgres:5432`. Um DATABASE_URL no `.env` do host não é usado pelo wrapper. A aplicação Rust continua sem conexão PostgreSQL. Não imprima strings de conexão nem use credenciais reais nos exemplos. A ajuda de subcomandos SQLx pode mostrar DATABASE_URL como padrão do ambiente; remova credenciais antes de compartilhar essa saída.

O healthcheck existente condiciona a partida do app; não há sleeps fixos. Health indica aceitação de conexões, não que os usuários ou senhas configurados existem. Erros de conexão SQLx encerram o comando; examine logs e valide autenticação TCP se o banco estiver indisponível.

## Diretório e estratégia inicial

```text
migrations/
  20261007000000_create_relay_schema.up.sql
  20261007000000_create_relay_schema.down.sql
  20261007175358_create_outbox_events.up.sql
  20261007175358_create_outbox_events.down.sql
```

Opção B: criar o namespace vazio `relay`, reservado para futuros objetos do relay. Define uma fronteira arquitetural e demonstra migração reversível sem tabelas de negócio. Migrações futuras devem qualificar objetos com `relay.`; o search_path padrão permanece inalterado. A migração up falha se já existir namespace sem gerenciamento. A down usa RESTRICT, recusando destruir objetos dependentes. SQLx mantém `_sqlx_migrations` no schema public padrão para controle interno; não é armazenamento da aplicação. Marco 1.4 adiciona relay.outbox_events por migração separada; sem repositórios Rust, inserts ou workers.

Migrações são código-fonte e devem ser commitadas com o código relacionado. `.dockerized-postgres/` é estado local ignorado, nunca histórico de migrações. Reconstruir containers preserva dados; um cluster novo exige `sqlx migrate run` explicitamente. Nem entrypoint nem partida da aplicação executam migrações ou reset automático.

## Fluxo de desenvolvimento

Crie uma migração, revise SQL up/down, aplique, inspecione o schema, execute testes, teste rollback, reaplique e faça commit junto com o código. SQLx prefixa arquivos com timestamp; use nomes claros como `add_relay_namespace_comment`. `create_outbox_events` é a migração do Marco 1.4; siga nomes igualmente descritivos. Evite `migration1`, `update_db` e `changes`. `-r` pede explicitamente arquivos up/down. Não é necessário Makefile.

```bash
docker compose up -d --build --wait --wait-timeout 120
docker compose exec app sqlx --version
docker compose exec app sqlx migrate add -r describe_schema_change
# Revise os dois arquivos SQL gerados antes de executá-los.
docker compose exec app sqlx migrate info
docker compose exec app sqlx migrate run
docker compose exec postgres sh -c 'psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -c "\dn"'
docker compose exec app cargo test --locked
docker compose exec app sqlx migrate revert
docker compose exec app sqlx migrate info
docker compose exec app sqlx migrate run
```

Para a migração inicial commitada, pule `migrate add`. `migrate revert` reverte a última migração reversível aplicada; não apaga todo o histórico. Confirme pending em `info` após rollback e installed após reaplicar. Inspecione pelo DBeaver ou psql no container. Não execute rollback às cegas em banco compartilhado.

## Responsabilidade e histórico imutável

Os arquivos de migração são o histórico canônico. DBeaver serve para inspeção, consultas e depuração; objetos criados manualmente nele não são mudanças oficiais do projeto. Registre mudanças pretendidas em migrações e revise o SQL.

Após commit e aplicação em histórico compartilhado, prefira nova migração a reescrever uma antiga. SQLx valida checksums; alterar SQL aplicado causa divergência. Histórico imutável favorece reprodutibilidade, consistência da equipe, sincronização dos ambientes e auditabilidade. Experimentos locais iniciais não compartilhados podem ser recriados deliberadamente; não normalize reescrita de histórico compartilhado ou exclusão de checksums.

## Transações e trade-offs de rollback

O backend PostgreSQL do SQLx executa cada migração e seu controle essencial em transação por padrão, incluindo down. DDL transacional evita mudanças parciais quando uma migração falha. A sequência inteira de migrações não é uma única transação. Operações como CREATE INDEX CONCURRENTLY não podem executar em bloco transacional. SQLx aceita a diretiva inicial `-- no-transaction` para esses casos; perder atomicidade exige recuperação cuidadosa. Não adicione BEGIN/COMMIT explícitos em migrações SQLx comuns. Consulte o [backend versionado](https://github.com/launchbadge/sqlx/blob/v0.8.6/sqlx-postgres/src/migrate.rs) e as [restrições CREATE INDEX](https://www.postgresql.org/docs/18/sql-createindex.html).

Down ajuda no desenvolvimento, mas não garante recuperar dados excluídos ou transformados. Rollback destrutivo em produção pode perder informações; backups, restauração testada e às vezes migração corretiva para frente são preferíveis. Este é um fluxo de aprendizado/desenvolvimento, não uma estratégia completa de implantação em produção. Revise down com o mesmo cuidado que up.

## Reset não é rollback

**Reset do banco != rollback de migração.** Reset exclui todo o cluster `.dockerized-postgres/`, todos os bancos, usuários e dados; rollback aplica SQL down controlado em um banco escolhido. Rollback não altera credenciais de inicialização. Reset exige intenção explícita e backup do necessário. Veja o [aviso e comandos exatos](postgresql.md#inicialização-e-reset-deliberado). Após reset, reaplique migrações explicitamente.

## Solução de problemas

- `sqlx: command not found`: reconstrua a imagem app; instalação no host é desnecessária.
- Usuário inexistente/falha de senha: `.env` pode divergir dos usuários persistidos. Mudar ambiente Docker não atualiza cluster inicializado; veja [inicialização PostgreSQL](postgresql.md).
- Conexão recusada/indisponível: examine `docker compose ps`, `docker compose logs postgres` e health; use postgres:5432 internamente, nunca 5433 no app.
- Checksum divergente: restaure SQL commitado e adicione nova migração; não ignore validação de histórico.
- Schema já existe: investigue responsabilidade antes de aplicar; não adote nem exclua objetos desconhecidos silenciosamente.
- Rollback RESTRICT falha: inspecione dependências e reverta primeiro suas migrações. Não substitua por CASCADE para forçar exclusão.
- Permissões: geração escreve na montagem do código como developer; alinhe LOCAL_UID/GID no Linux e permissões da montagem.

Consulte [uso do SQLx CLI 0.8.6](https://github.com/launchbadge/sqlx/blob/v0.8.6/sqlx-cli/README.md), [ADR 003](adr/003-use-versioned-sql-migrations.md), [testes](testing.md) e [validação real](validation-results.md).

## Diagnóstico de autenticação somente leitura

```bash
./scripts/check-postgres.sh
docker compose exec -T app sqlx migrate info
```

[check-postgres.sh](../../scripts/check-postgres.sh) verifica health, publicação IPv4 loopback, Compose/.env atual versus configurações dos containers, identidade autenticada e histórico existente. Não imprime usuário ou senha configurados. Falha com saída não-zero em divergência de configuração/autenticação, nunca altera usuários, cria metadados, aplica migrações ou reinicializa dados. Usa Python no container app; a sondagem TCP do host usa nc ou Python 3 e informa explicitamente quando pula por ausência de ambos. SQLx é validado separadamente.

Compose fornece POSTGRES_HOST/PORT ao postgres para diagnósticos de cliente, sem alterar escuta do servidor. Host/DBeaver usa 127.0.0.1:5433 e POSTGRES_DB/USER/PASSWORD atualmente inicializados; containers usam postgres:5432. A interface DBeaver não foi testada.

Mudar `.env` não atualiza cluster inicializado. Usuário ausente pode gerar erro TCP genérico de senha. Inspecione detalhes do servidor/usuários para distinguir de usuário existente com senha incorreta. A correção real verificou ausência de dados antes do reset destrutivo explicitamente autorizado e executou migrações existentes. Startup/shutdown Compose normal continua não destrutivo. Reset exclui todo estado; rollback de migração não corrige credenciais.

Configuração Compose bruta, dumps de ambiente e ajuda SQLx podem revelar segredos; use parser que informe apenas campos não sensíveis/comparações e remova credenciais dos logs compartilhados. Veja [correção PostgreSQL](postgresql.md#correção-de-autenticação-no-cluster-local-real) e [resultados reais](validation-results.md#correção-de-autenticação-postgresql-local).

A última migração atual é create_outbox_events. Revert exclui só essa tabela e seus índices/constraints, preservando relay; inspecione dados antes. Reaplique com migrate run. [Schema outbox](outbox-schema.md) documenta mapeamento e limites do rollback destrutivo.
