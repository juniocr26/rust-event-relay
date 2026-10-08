[English](../en/development-dependencies.md) | [README](../../README.pt-BR.md)

# Dependências de desenvolvimento e recuperação

`Cargo.toml` declara metadados da aplicação, intervalos de versões e features das dependências. `Cargo.lock` registra versões exatas resolvidas e checksums, incluindo dependências transitivas. Versione o lockfile nesta aplicação; `--locked` impede mudanças silenciosas de resolução. Atualizações intencionais usam `cargo update`, seguido das verificações e revisão do lockfile.

`cargo fetch --locked` baixa dependências sem compilar. `cargo build --locked` compila aplicação e dependências, baixando fontes ausentes quando necessário. O primeiro fetch precisa de rede; um cache preenchido permite comandos offline com `--offline`.

`CARGO_HOME` contém índices de registry, arquivos de crates baixados, fontes extraídas e checkouts git quando usados. Também pode conter configuração ou credenciais Cargo; não o versione nem guarde segredos em arquivos rastreados. Artefatos compilados não pertencem ali. `target/` contém dependências compiladas, binários e estado incremental; não substitui o cache de fontes.

Docker usa `CARGO_HOME=/app/.cargo-cache` e `CARGO_TARGET_DIR=/app/target`. Todo o repositório é montado em `/app`, tornando ambos os diretórios fisicamente visíveis no host. Não há volumes nomeados. O entrypoint cria diretórios ausentes como usuário developer sem privilégios de root. No Linux, alinhe primeiro UID/GID; consulte [Docker](docker-and-configuration.md).

Se apenas `.cargo-cache/` for removido, execute `docker compose exec app cargo fetch --locked`; se apenas `target/` for removido, execute `docker compose exec app cargo build --locked`. Pare processos Cargo/aplicação antes de apagar seus diretórios. Reconstrução completa a partir da raiz do repositório:

```bash
docker compose down
rm -rf .cargo-cache target
docker compose run --rm --no-deps app cargo build --locked
python3 scripts/start-local.py
docker compose exec app cargo test --locked
```

A remoção descarta apenas downloads/artefatos ignorados, não fontes ou `Cargo.lock`. `up` recria diretórios ausentes; fetch restaura fontes; build recria artefatos. Se a imagem também estiver ausente ou Dockerfile/UID/GID mudarem, use `docker compose up -d --build`. Nem `down` nem reconstruir a imagem removem os diretórios do host.

Cargo nativo normalmente usa `~/.cargo` para dependências e `target/` do repositório para builds. Para usar os mesmos caminhos explícitos nativamente: `CARGO_HOME="$PWD/.cargo-cache" CARGO_TARGET_DIR="$PWD/target" cargo fetch --locked`. Artefatos nativos e do container dependem da plataforma; use `cargo clean` ao alternar targets/toolchains incompatíveis. Um lockfile válido sozinho não permite recuperação offline após apagar todos os caches de fontes.

PostgreSQL segue a mesma filosofia de estado visível com `.dockerized-postgres/`, mas contém dados de banco em vez de artefatos Cargo regeneráveis. A recuperação acima preserva o banco. `docker compose down` preserva os três diretórios. Consulte [armazenamento do banco e reset deliberado](docker-and-configuration.md).

## Repositório PostgreSQL — Marco 1.6 implementado

`src/infrastructure/postgres/` implementa `OutboxReader` com `PgPool` injetado. Infraestrutura depende dos contratos de persistência e domínio; SQL, SQLx, linhas privadas e classificação de erros ficam na infraestrutura. Modelos validam/convertem dados sem consultas. ADR 005 permanece suficiente: sem interface duplicada, camadas vazias, nova migração ou ADR. Futures Send nativas e dispatch estático permanecem. Bootstrap HTTP continua independente do banco.

Veja [consulta, restauração, limites e erros](postgres-repository.md), [testes de integração](testing.md) e [validação executada](validation-results.md). SQLx agora também é dependência da aplicação. Escrita, claims e processamento ficam adiados; Marcos 1.7 e 1.8 concluídos: integração e [semântica de falhas/transações](failure-and-transaction-semantics.md). Marco 1 fechado; entrega futura não iniciada.

[Marco 2.1](milestone-2-1.md) adiciona Lapin 4.12.0 (Tokio, AMQP local plaintext) e encoding de URL. Após reconstruir artefatos, inicie filho `http` com `supervisorctl start http`.
