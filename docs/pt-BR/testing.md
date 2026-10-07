[English](../en/testing.md) | [README](../../README.pt-BR.md)

# Testes

```bash
cargo test --locked
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
```

```bash
docker compose exec app cargo test --locked
docker compose exec app cargo fmt --check
docker compose exec app cargo clippy --locked --all-targets --all-features -- -D warnings
```

`cargo test` também funciona sem `--locked`; use a opção para validação reproduzível. Execute `cargo fmt` para aplicar formatação. CI usa as mesmas verificações em Rust 1.95.0. Nenhum teste exige banco ou broker.

Os testes unitários atuais verificam padrões e rejeição de endereços inválidos/ambiente vazio sem mudar o ambiente do processo. Testes de integração abrem um listener real em porta efêmera, verificam HTTP 200 e corpo, solicitam encerramento gracioso e confirmam o fechamento do listener. Um teste de subprocesso Unix carrega `.env` isolado, confere o ambiente no log de partida e envia separadamente SIGTERM/SIGINT, esperando saída bem-sucedida e log final. Ele exige o comando de sistema `kill` (incluído na imagem Docker). Arquivos temporários ficam fora das fontes. Windows ignora apenas esse teste Unix. Esperas por servidor/processo têm prazos; não há serviços externos nem portas fixas nos testes.

Esses testes comprovam bootstrap, não confiabilidade de eventos. Os próximos marcos adicionarão integração de persistência/broker, janelas de crash entre publicação/confirmação, tentativas, idempotência, mensagens problemáticas, limites de concorrência e contrapressão. Testes de infraestrutura devem isolar estado e injetar falhas; benchmarks precisam publicar carga, hardware, metodologia e limitações medidas. Consulte [resultados de validação](validation-results.md) para comandos executados; configurar CI não comprova uma execução concluída no GitHub.

## Testes do envelope canônico

`tests/event_envelope.rs` verifica geração de identidade UUID v7 e instante UTC, validação na construção/restauração, formato JSON exato, round-trips de metadados e normalização de offsets. Esses testes independem do PostgreSQL.

## Validação de infraestrutura PostgreSQL (Marco 1.2)

São checks locais explícitos de infraestrutura, não testes de integração da persistência da aplicação. Comece com os padrões locais documentados; use seu usuário/banco configurado se diferente:

```bash
docker compose up -d --build --wait --wait-timeout 120
docker compose ps
docker compose exec -T postgres pg_isready -h 127.0.0.1 -p 5432 -U relay -d reliable_event_relay
docker compose exec -T postgres sh -c 'PGPASSWORD="$POSTGRES_PASSWORD" psql -h postgres -p 5432 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT 1;"'
docker compose port postgres 5432
# psql no host, se instalado; informe a senha local quando solicitado:
psql -h localhost -p 5433 -U relay -d reliable_event_relay -W -c "SELECT 1;"
docker compose exec -T app cargo fmt --check
docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings
docker compose exec -T app cargo test --locked
docker compose exec -T app cargo build --locked
```

Confirme PostgreSQL healthy, publicação `127.0.0.1:5433`, autenticação e arquivos em `.dockerized-postgres/18/docker/`. Uma sondagem TCP no host comprova apenas alcance, não autenticação SQL. `/health` e os testes de ciclo de vida continuam verificando a aplicação sem conexão ao banco.

### Sobrevivência à recriação do container

Apenas no seu banco local de desenvolvimento: use uma tabela comum descartável com nome único (uma tabela SQL TEMP não sobrevive à sessão), insira um marcador, recrie o container PostgreSQL sem apagar dados do host, verifique o marcador e remova a tabela. Se o nome escolhido já existir, pare e escolha outro; nunca remova um objeto não relacionado. O objeto é apenas de validação de infraestrutura e não deve permanecer:

```bash
docker compose exec -T postgres psql -U relay -d reliable_event_relay -v ON_ERROR_STOP=1 -c "CREATE TABLE milestone12_validation_20261007 (marker text); INSERT INTO milestone12_validation_20261007 VALUES ('survives-recreation');"
docker compose up -d --force-recreate --no-deps --wait --wait-timeout 120 postgres
docker compose exec -T postgres psql -U relay -d reliable_event_relay -v ON_ERROR_STOP=1 -c "SELECT marker FROM milestone12_validation_20261007;"
docker compose exec -T postgres psql -U relay -d reliable_event_relay -v ON_ERROR_STOP=1 -c "DROP TABLE milestone12_validation_20261007;"
```

Confirme mudança do ID do container e montagem no mesmo diretório físico. Não faça reset/apague dados durante a validação de sobrevivência. Este teste não comprova transações da aplicação, recuperação de crashes ou garantias de entrega. Consulte [resultados reais](validation-results.md) e [instruções de reset destrutivo](docker-and-configuration.md).
