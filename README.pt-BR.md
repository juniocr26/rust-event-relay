[English](README.md) | [Português brasileiro](README.pt-BR.md)

# Reliable Event Relay

Projeto de portfólio e estudo em Rust com outbox transacional PostgreSQL, publicação confirmada no RabbitMQ e deduplicação atômica no consumidor. A interface Vue executa entrega, duplicata, duplicatas concorrentes e rejeição limitada antes do envio com retry real. Animações de crash e perda de ACK são explicitamente ilustrativas.

Milestone 6.3 aceito localmente em 17/10/2026; Milestone 6 e o escopo de portfólio acordado concluídos. Instalação limpa, cenários reais, inspeção, recuperação e verificações locais aprovados. CI remoto não verificado; prontidão de produção não estabelecida. Registros históricos preservam seu escopo datado; esta página define o fluxo suportado.

Auditoria final de implementação (17/10/2026): [encerramento e evidências executadas](docs/pt-BR/operations/final-audit.md).

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

No host, a partir da raiz do repositório:

```sh
docker compose exec app tests
```

Dentro de app, em qualquer diretório: `tests`. Na automação: `docker compose exec -T app tests`.

O comando prepara automaticamente dependências npm travadas e Chromium. Executa formatação/lint/testes/binários/documentação Rust, links da documentação e testes das ferramentas Python, tipos/build/browser do frontend, integrações reais PostgreSQL/RabbitMQ/processos, upgrades/rollbacks SQLx em banco descartável e os cenários reais normal/duplicado/concorrente/recuperação no navegador com processos próprios, bancos descartáveis e namespaces de broker na infraestrutura existente. O browser também verifica retomada do consumer pausado, HTTP indisponível e inspeção de ação desconhecida sob Supervisor isolado. Não migra o banco principal nem para os processos gerenciados da aplicação.

A primeira execução baixa pacotes/browser e compila Rust. Os artefatos de teste ficam em `target/tests`; os serviços usam `target/runtime`. Caches npm/browser e a cópia isolada do frontend ficam em `/tmp/relay-tests`, reutilizados até recriar o container. O node_modules do serviço frontend não é modificado. Somente uma execução simultânea é permitida. Uma falha obrigatória interrompe a sequência; o resumo lista apenas fases concluídas. Códigos: 0 sucesso, 1 falha, 130 interrupção. Os diagnósticos mostram a parte final sanitizada e salvam a última fase com falha em `/tmp/relay-tests/last-failure.log` dentro de app; Ctrl+C sinaliza o grupo de processos próprio e permite limpeza antes de encerramento forçado. Falha de limpeza é reportada como falha.

Aceitação separada no host (opcional e potencialmente cara):

```sh
python3 -B scripts/test-application.py
python3 -B scripts/test-clean-install.py
python3 -B scripts/measure-isolated.py verification
```

Essas verificações orquestram infraestrutura Docker nova, controles de indisponibilidade na composição principal e instalações limpas. Exigem Docker no host; não pertencem ao app. Campanhas de capacidade continuam opcionais; consulte os modos documentados no script de medição. `tests` não afirma executar essas aceitações. Não é necessário socket Docker nem modo privilegiado.

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

[Token, ativação persistente e diagnóstico](docs/pt-BR/operations/demo-token-connectivity.md).

Dentro do contêiner app: `python3 /app/scripts/demo-token.py rotate`.
