[English](README.md) | [Português brasileiro](README.pt-BR.md)

# Reliable Event Relay

Projeto de portfólio e estudo em Rust com outbox transacional PostgreSQL, publicação confirmada no RabbitMQ e deduplicação atômica no consumidor. A interface Vue executa entrega, duplicata, duplicatas concorrentes e rejeição limitada antes do envio com retry real. Animações de crash e perda de ACK são explicitamente ilustrativas.

Milestone 6.3 aceito localmente em 17/10/2026; Milestone 6 e o escopo de portfólio acordado concluídos. Instalação limpa, cenários reais, inspeção, recuperação e verificações locais aprovados. CI remoto não verificado; prontidão de produção não estabelecida. Registros históricos preservam seu escopo datado; esta página define o fluxo suportado.

Auditoria final de implementação (17/10/2026): [encerramento e evidências executadas](docs/pt-BR/operations/final-audit.md), com [PDF na raiz](PROJECT_FINAL_AUDIT.pdf).

## Instalação limpa

Instale Docker Engine/Desktop com Compose v2, Python 3 e Git. O Docker precisa estar ativo, com acesso à internet para imagens/pacotes e espaço para compilar Rust. Rust/Node nativos são opcionais; os comandos abaixo usam containers. Copie o exemplo, substitua os quatro placeholders de credenciais e inicie:

```sh
git clone https://github.com/juniocr26/rust-event-relay.git
cd rust-event-relay
cp .env.example .env
# POSTGRES_USER, POSTGRES_PASSWORD, RABBITMQ_DEFAULT_USER, RABBITMQ_DEFAULT_PASS
# Linux: LOCAL_UID=$(id -u), LOCAL_GID=$(id -g) — edit .env accordingly.
python3 scripts/start-local.py
```

Use credenciais locais exclusivas no `.env` ignorado; não as copie para logs, commits ou variáveis VITE_*. O script gera DEMO_TOKEN uma vez, valida configuração, inicia infraestrutura, compila binários, verifica migrações, cria backup com restauração de prova antes de migração necessária e inicia HTTP/relay/consumer pelo Supervisor e o frontend. Paradas intencionais sob o mesmo Supervisor são preservadas; use controles por nome para retomá-las. Não use comandos históricos de rollout como instalação.

| Service | Default address |
| --- | --- |
| Frontend | http://localhost:5173 |
| HTTP liveness | http://localhost:8080/health |
| PostgreSQL | localhost:5433 |
| RabbitMQ management | http://localhost:15672 |

Abra o frontend, escolha um cenário real, abra o modal e informe DEMO_TOKEN do `.env` privado. Execute a entrega: estoque inicial 10 e resultado 15 após commit do consumidor. Duplicatas preservam 15 e um recibo de efeito. “Inspect retained runs and actions” consulta evidências preservadas; reload exige autenticação novamente. Fechar o modal encerra polling, nunca trabalho já confirmado no backend.

## Documentação e evidências

- [Guia atual: ciclo de vida, cenários, SQL, topologia, recuperação e estudo](docs/pt-BR/operations/application-scenarios.md)
- [Matriz de aceitação 6.3 e resultados atuais](docs/pt-BR/operations/milestone-6-3.md)
- [Catálogo completo](docs/pt-BR/index.md)
- [ADR 012](docs/pt-BR/adr/012-one-application-real-scenarios.md)

## Testes

```sh
docker compose exec -T app cargo fmt --check
docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings
docker compose exec -T app cargo test --locked
docker compose exec -T app cargo build --locked --bins
docker compose exec -T app env RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps
docker compose exec -T frontend npm ci
docker compose exec -T frontend npm run type-check
docker compose exec -T frontend npm run build
python3 -B scripts/measure-isolated.py verification
python3 -B scripts/test-application.py
python3 -B scripts/test-clean-install.py
python3 -B scripts/check-migrations.py
python3 -B scripts/check-docs.py
python3 -B -m unittest discover -s scripts -p 'test_*.py'
git diff --check
```

O harness opt-in cria PostgreSQL/RabbitMQ descartáveis; testes de navegador usam cópia temporária da composição principal. O harness de instalação copia apenas arquivos candidatos visíveis ao Git, cria credenciais privadas, portas isoladas e armazenamento novo; compila sem artefatos Cargo/node do host. São instalações temporárias de teste. Nunca execute falhas destrutivas nos serviços com evidências preservadas. CI está em `.github/workflows/ci.yaml`; sucesso local não comprova CI remoto.

## Operação e limites

```sh
docker compose exec -T app supervisorctl status
docker compose exec -T app supervisorctl stop http
docker compose exec -T app supervisorctl start http
docker compose stop
# Later / depois:
python3 scripts/start-local.py
```

Alterações de código exigem recompilar e reiniciar deliberadamente o processo relevante; um binário no disco não comprova que o processo o carregou. PostgreSQL persiste em `.dockerized-postgres/` e RabbitMQ em volume nomeado. Não resete volumes, purgue filas ou exclua recibos para reparar.

Esta é uma demonstração local limitada, sem aceite de produção ou transporte exatamente uma vez. SQL e AMQP não confirmam atomicamente. Resultados desconhecidos exigem inspeção; confirms, conclusão do consumidor e recebimento de ACK pelo broker são fatos distintos. TLS/HA, caos irrestrito, retenção automática, reparo genérico e pools maiores estão fora do escopo local acordado.

MIT — [LICENSE](LICENSE).
