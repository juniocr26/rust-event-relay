[English](../../en/architecture/milestone-2-1.md) | [README](../../../README.pt-BR.md)

# Marco 2.1 — Publisher RabbitMQ e gestão de processos

Implementado em 2026-10-08. Este é o recorte de publicação do Marco 2, sem completar entrega pelo menos uma vez. O handoff do Marco 1 permanece como snapshot histórico. A [ADR 006](../adr/006-rabbitmq-publisher-and-supervisor.md) registra as decisões.

## Ambiente local e login no navegador

Edite o `.env` ignorado na raiz: substitua os placeholders `RABBITMQ_DEFAULT_USER` e `RABBITMQ_DEFAULT_PASS` pelas credenciais locais. Digite **esses mesmos valores no formulário do navegador em http://localhost:15672/**. Este checkout já tem credenciais locais configuradas; elas não são documentadas nem commitadas. `.env.example` contém somente placeholders. Variáveis do shell têm precedência na interpolação Compose; evite sobrescritas acidentais. Não compartilhe saída bruta de `docker compose config`, dumps de ambiente ou cadeias de fontes com segredos.

```bash
# Somente em checkout novo:
cp .env.example .env
# Edite credenciais e, no Linux, LOCAL_UID / LOCAL_GID antes da inicialização.
docker compose build app
# Compile o binário real antes de o supervisord iniciar HTTP:
docker compose run --rm --no-deps app cargo build --locked
python3 scripts/start-local.py
python3 scripts/check-rabbitmq.py
curl --fail http://localhost:8080/health
```

`start-local.py` detecta conflito na porta de management e informa a porta solicitada sem alterá-la. Docker também informa conflitos/corridas no bind. Publicação padrão: `127.0.0.1:${RABBITMQ_MANAGEMENT_PORT:-15672}:15672`. Sobrescrever explicitamente a variável muda a URL. Libere um conflito na porta padrão para manter `http://localhost:15672/`.

Compose fixa `rabbitmq:4.3.6-management`, incluindo plugin management habilitado, conforme [instalação/releases oficiais](https://www.rabbitmq.com/docs/download) e [tags da imagem oficial](https://hub.docker.com/_/rabbitmq/). App usa **rabbitmq:5672** pelo DNS Docker. AMQP não é publicado no host: a integração real executa no app; somente HTTP management precisa de acesso pelo host. Dados do broker persistem no volume nomeado `rabbitmq-data`, com hostname estável `rabbitmq`. Configuração PostgreSQL e `.dockerized-postgres/` foram preservados. Down/up e rebuild normais preservam dados; não use `down -v` para mudar credenciais.

Readiness do broker usa `rabbitmq-diagnostics -q check_running`; app aguarda health de broker e PostgreSQL sem sleeps fixos. Isso não comprova readiness autenticada nem entrega. Diagnóstico e integração verificam autenticação separadamente; `/health` continua apenas liveness.

`docker/rabbitmq/rabbitmq.conf` inicializa o usuário com tag **management** e regex configure/write/read `^relay[.].*` no `RABBITMQ_DEFAULT_VHOST` (padrão `relay`). Permite operações na topologia do projeto e recursos únicos `relay.it.*`, sem conceder acesso a outros vhosts. Management basta para navegador, `/api/whoami` e vhosts acessíveis; listar registros de permissões exige administrador. O diagnóstico verifica identidade autenticada e vhost visível via HTTP, e permissões pelo CLI local do nó. Não promove o usuário a administrador.

**Variáveis de inicialização não atualizam volume existente.** Editar `.env`, reconstruir ou recriar container não modifica usuários/senhas/tags/permissões/vhosts persistidos. Se autenticação falhar, inspecione primeiro usuário e vhost existentes. Use administrador autorizado e administração RabbitMQ de usuário/senha/tag/permissão (ou `rabbitmqctl help change_password`) para alterar a conta existente; sincronize `.env` e recrie o ambiente do app. Prefira entrada interativa de senha ou fluxo administrativo protegido, evitando histórico do shell/argumentos de processos. Nunca resete/apague dados do broker para aplicar credenciais. Volumes existentes não são reconciliados automaticamente.

RabbitMQ armazena hashes de senha; isso **não** cifra o tráfego. HTTP e AMQP locais são plaintext. TLS, exposição externa, controle de acesso produtivo, backups e alta disponibilidade exigem desenho operacional posterior. Broker local único e durável não garante HA. Veja [controle de acesso RabbitMQ](https://www.rabbitmq.com/docs/access-control).

## Arquitetura e semântica de publicação

`application::publisher::EventPublisher` recebe `EventEnvelope` validado emprestado e retorna future Send nativa com despacho estático. `infrastructure::rabbitmq::RabbitMqPublisher` implementa com Lapin 4.12.0, feature Tokio, sem backend TLS neste escopo local explicitamente plaintext e sem recovery/retry automático. Veja [documentação oficial Lapin](https://docs.rs/lapin/4.12.0/lapin/). Tipos de protocolo ficam na infraestrutura. `main` continua iniciando só HTTP: não há worker nem endpoint de publicação. `OutboxReader` segue somente leitura, sem publicar/mutar estado. Futuro use case deve controlar orquestração, propriedade e transições.

Adapter possui uma conexão e um canal com confirms. `connect` declara exchange **direct** durável `relay.events`, fila durável `relay.local` e binding `event`; topologia surge quando o adapter conecta, não no bootstrap HTTP. Overrides `RABBITMQ_EXCHANGE`, `RABBITMQ_QUEUE`, `RABBITMQ_ROUTING_KEY` ficam fora do domínio. Exchange/fila começam com `relay.`; nomes AMQP têm limite de 255 bytes. Fila local retém mensagens até consumo explícito; não há consumidor produtivo.

Publicação serializa o envelope intacto com serde_json arbitrary precision, preservando ID, instante, payload e metadados opcionais. Propriedades AMQP: `content_type=application/json`, `delivery_mode=2` (persistente) e `message_id=event.id` estável. Publish mandatory aguarda confirmação do broker, além da escrita no socket.

| Resultado | Significado |
| --- | --- |
| `Ok(())` | Ack confirmado sem retorno na topologia configurada |
| `Unroutable` | Ack acompanhado de mandatory return: não houve publicação roteada bem-sucedida |
| `Rejected` | Nack do broker |
| `Unavailable` | Configuração inválida, falha de conexão/setup/admissão ou transporte fechado/retirado antes de enviar |
| `Serialization` | Serialização JSON falhou antes do envio |
| `Uncertain` | Operação de envio/confirm falhou ou expirou após entrar na fronteira de envio, ou confirms ausentes inesperadamente |

Display/Debug públicos mostram somente classificação. Fontes tipadas URL/parse, serde, Lapin e Tokio ficam em Error::source; cadeias podem conter segredos e exigem redação. Envelope validado atual não possui serializer deliberadamente falho: categoria Serialization é defensiva, sem falha real injetada.

`RABBITMQ_CONNECT_TIMEOUT_MS` e `RABBITMQ_PUBLISH_TIMEOUT_MS` têm padrão 10000; duração válida: 1–300000 ms. Conexão e setup de topologia têm cada um prazo de conexão; limpeza após falha de setup tem outro prazo limitado. Admissão e envio+confirm têm cada um prazo de publicação. Serialização é síncrona antes das esperas de rede, sem limite de bytes/memória. Limites de payload ficam futuros.

Mutex serializa publicações concorrentes, com um envio em voo e sem criar task por mensagem. Quantidade de tasks/admissão do chamador ainda exige limites na aplicação futura. O canal é marcado como retirado antes de entrar no envio, restaurado só após ack/return/nack definitivo. Timeout, erro de transporte ou cancelamento após essa fronteira o mantêm retirado: nova publicação falha antes de enviar; chamador deve fechar/descartar e reconectar explicitamente. Cancelamento não retorna erro ao chamador nem prova ausência; reconexão não reconcilia evento anterior. **Não há retry automático**, especialmente com aceitação incerta. `close(self)` após aguardar chamadas em voo fecha graciosamente com prazo; drop limpa recursos, sem confirmar nem garantir drenagem graciosa.

Confirm prova aceitação do broker na topologia, sem comprovar processamento do consumidor ou exatamente uma vez, conforme [semântica RabbitMQ](https://www.rabbitmq.com/docs/confirms). Repetir resultado desconhecido pode duplicar; estado persistido de entrega e idempotência exigem desenho separado.

## Supervisor e fluxo de build

Config física versionada: `docker/supervisor/supervisord.conf`, instalada em `/etc/supervisor/supervisord.conf`. Recompile/recrie app após modificar config ou aliases executáveis. `init: true` permanece; entrypoint developer sem root executa supervisord em primeiro plano. Montagens de fonte/cache e Cargo continuam graváveis. Diretório do socket pertence a developer com modo 0700; socket Unix tem modo 0600. `supervisorctl` descobre config padrão sem `-c` manual.

Nome real e único programa: **http**. RabbitMQ e PostgreSQL são serviços Compose separados. Publisher é adapter chamável, sem daemon ocioso. `all` significa todos os programas desse Supervisor do app (atualmente `http`).

```bash
docker compose exec app bash
# Diretos dentro do app:
supervisorctl status
supervisorctl stop all
supervisorctl start all
supervisorctl stop http
supervisorctl start http
supervisorctl restart http
# Alias encaminha todos os argumentos:
supervisor status
supervisor stop all
supervisor start all
supervisor stop http
supervisor start http
supervisor restart http
# Interativo: execute sem argumentos:
supervisorctl
# Ou: supervisor
# No prompt supervisor>:
status
stop all
start all
stop http
start http
restart http
quit
```

`supervisor` é executável pequeno encaminhando para supervisorctl. Stop intencional mantém supervisord disponível. Filho executa `/app/target/debug/reliable-event-relay` por launcher com `exec`, sem `cargo run`. Binário ausente imprime instrução de build e tem tentativas de partida limitadas; app continua disponível para compilar e `start http`. Container iniciado não prova filho iniciado: confira status e health.

Para mudar fonte, pare antes de recompilar e depois inicie:

```bash
docker compose exec app supervisorctl stop http
docker compose exec app cargo build --locked
docker compose exec app supervisorctl start http
# Restart também disponível após build já concluído:
docker compose exec app supervisor restart http
```

Saída inesperada com código não zero/sinal reinicia filho; saída 0 é esperada. Stop explícito nunca autoreinicia. SIGTERM solicita drenagem HTTP; parada/kill em grupo evitam descendentes órfãos. Supervisor permite 30 segundos antes de SIGKILL, orçamento finito local para servidor hoje limitado a health; Compose permite 35 segundos ao manager. Requisições longas podem ultrapassar prazo e ser interrompidas. Stdout/stderr do filho vão aos streams do container sem rotação. Inspecione `docker compose logs app`. Testes diretos do binário continuam independentes de Supervisor.

## Comandos de teste e evidências

```bash
docker compose exec -T app cargo fmt --check
docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings
docker compose exec -T app cargo test --locked
docker compose exec -T app cargo test --locked --test postgres_repository -- --ignored --test-threads=4
docker compose exec -T app cargo test --locked --test rabbitmq_publisher -- --ignored
docker compose exec -T app cargo test --locked --lib infrastructure::rabbitmq::tests::closed_owned_connection_is_unavailable -- --ignored
docker compose exec -T app cargo build --locked
docker compose exec -T -e RUSTDOCFLAGS='-D warnings' app cargo doc --locked --no-deps
python3 scripts/check-rabbitmq.py
docker compose exec -T app python3 scripts/check-supervisor.py
git diff --check
```

Suíte padrão não exige banco/broker. Integrações usam credenciais explícitas e topologia UUID única no vhost do projeto; limpeza remove só filas/exchanges exatos após sucesso/panic/prazo. Proxy TCP próprio retém confirmações para testar timeout/cancelamento pós-envio, sem reiniciar broker ou afetar clientes alheios. Mensagem recebida comprova que resultado incerto pode já estar aceito; não produz segunda mensagem/retry. Morte abrupta do teste ou falha de limpeza pode deixar recursos isolados; inspecione nomes exatos antes de limpar. CI inclui job RabbitMQ com credenciais descartáveis no padrão do job PostgreSQL.

Execução real e limitações estão em [validação](../testing/validation-results.md). Página management e API HTTP autenticada foram verificadas no localhost do host do usuário. **Login pelo formulário do navegador não foi testado diretamente: não há ferramenta de controle de navegador disponível.** API não equivale a login interativo. CI remoto GitHub e comportamento produtivo não foram executados.

## Marco 2 adiado

Transições/autoridade de estado, claims/leases/propriedade, reconciliação de resultados incertos, retry/backoff, dead-letter/quarentena/reparo, escrita do produtor, idempotência do consumidor, worker de polling, limites de bytes/concorrência e readiness operacional ficam adiados. Snapshot pending não autoriza entrega exclusiva. Broker aceita e transição no banco falha: janela de duplicação. Banco marca conclusão antes de publicar: janela de perda. Esses desenhos são pré-requisitos para a fundação de entrega pelo menos uma vez além deste adapter.
