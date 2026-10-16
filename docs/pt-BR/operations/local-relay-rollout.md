[Outro idioma](../../en/operations/local-relay-rollout.md)

# Rollout local e inicialização automática do relay

Executado em 2026-10-13 após 3.3, com Rust limpo `cae62cc654efc0b411408dd84a0b92a57738eb38` e Library `c272476429fe4c9223f274d74293edeabb8c79b5`. Usuário autorizou explicitamente aplicar migrations existentes de ownership/retry e ativar worker de desenvolvimento local. Registros anteriores de migrations pendentes/relay parado descrevem estado anterior. Política 3.3 preservada; não representa prontidão produtiva ou fechamento do Marco 3.

## Política instalada

[docker/supervisor/normal.conf](../../../docker/supervisor/normal.conf) inicia `http` e `relay` com autostart=true. RabbitMQ/PostgreSQL continuam serviços Compose separados; aplicação aguarda service_healthy dos dois. PostgreSQL usa SELECT 1 autenticado; RabbitMQ verifica operação do nó. Worker executa preflight autenticado de banco/schema e setup do broker pelo adaptador existente. Saúde pode mudar após admissão; recuperação explícita do worker permanece. Nenhuma migration roda no entrypoint ou nos binários.

Supervisor mantém autorestart=unexpected, exitcodes=0, startsecs=1 e startretries=3. Saídas não zero/sinais inesperados após RUNNING reiniciam; saída normal zero não. Falhas de startup têm tentativas limitadas e podem chegar a FATAL: corrigir configuração/schema/artefatos e iniciar manualmente. supervisorctl stop intencional mantém STOPPED até start explícito, mesmo com autostart=true. Autostart se aplica a novo início do supervisord; restart/recriação do contêiner inicia ambos novamente. [Contrato Supervisor](https://supervisord.org/configuration.html#program-x-section-settings) e [readiness de dependências Compose](https://docs.docker.com/compose/how-tos/startup-order/).

SIGTERM preserva drain/shutdown existentes do worker (padrões 15 s / 25 s, limites validados). Supervisor permite 30 s por filho antes de kill; stop/kill de grupo mantidos. Compose permite 65 s para encerramento sequencial e margem. Cancelamento pode esconder efeitos duráveis; nenhuma compensação adicionada.

## Instalação explícita e controles

Em ambiente local novo, configurar .env ignorado com credenciais atuais, revisar SQL/dados e preparar artefatos/schema explicitamente antes da aplicação:

```bash
docker compose build app
docker compose run --rm --no-deps app cargo build --locked --bins
docker compose up -d --wait postgres rabbitmq
docker compose run --rm --no-deps app sqlx migrate info
# Ação explícita do operador somente após revisar SQL e dados atuais:
docker compose run --rm --no-deps app sqlx migrate run
python3 scripts/start-local.py
docker compose exec -T app supervisorctl status
```

Para atualização do Supervisor com dependências/schema prontos, reconstruir/recriar somente aplicação afetada:

```bash
docker compose up -d --build --no-deps app
```

Storage bind/nomeado de banco/broker permanece. Não resetar banco, excluir volumes, purgar filas ou mudar credenciais para instalar autostart. Sincronização de credenciais existentes exige ação explícita; variáveis de inicialização não alteram credenciais persistidas.

Controles diretos:

```bash
docker compose exec app supervisorctl status
docker compose exec app supervisorctl stop relay
docker compose exec app supervisorctl start relay
docker compose exec app supervisorctl restart relay
docker compose exec app supervisorctl stop http
docker compose exec app supervisorctl start http
# Alias encaminha os mesmos argumentos:
docker compose exec app supervisor status
docker compose exec app supervisor stop relay
docker compose exec app supervisor start relay
# Console: status, stop relay, start relay, restart http, quit
docker compose exec app supervisorctl
docker compose exec app supervisor
```

`all` controla ambos os filhos da aplicação, nunca PostgreSQL/RabbitMQ ou frontend. Parar relay mantém HTTP, management e storage independente do broker. Para reconstruir binários montados, parar filhos correspondentes, executar cargo build --locked --bins e iniciar novamente. Sem reload automático.

## Topologia e evidência

Management permanece [http://localhost:15672/](http://localhost:15672/); usar credenciais atuais do .env ignorado. AMQP interno usa rabbitmq:5672. RabbitMqPublisher existente declara exchange direct durável, fila durável e binding/routing key configurados após conexão autenticada. Padrões: vhost relay, exchange relay.events, fila relay.local, routing key event; conferir overrides antes de assumir padrões. Setup não publica mensagens. Painel vazio é válido sem trabalho de produtor. Nenhuma mensagem de demonstração foi enviada para preencher o painel.

[check-rabbitmq.py --require-topology](../../../scripts/check-rabbitmq.py) verifica página host, identidade/permissões autenticadas, vhost e topologia durável exata, sem imprimir credenciais, corpos API ou conteúdo de mensagens. Sem flag continua útil antes da declaração. [check-supervisor.py](../../../scripts/check-supervisor.py) é teste operacional explícito: para/inicia/reinicia ambos por comandos diretos/interativos, espera após stop intencional, verifica HTTP/metadados/contagens da fila e injeta SIGKILL para verificar recuperação. Não publica/consome mensagens, purga ou migra. Executar somente quando esses controles locais forem pretendidos.

Rollout inspecionou zero linhas outbox, inclusive zero esgotados, e duas migrations iniciais instaladas. SQL aditivo existente de ownership 20261009000000 e retry 20261012000000 revisado; sqlx migrate run aplicou ambos explicitamente. As quatro migrations reportam instaladas, outbox original permanece vazio e constraints/preflight passam. Baseline zero não renova orçamento histórico; metadados de terminais legados mantêm contrato existente.

Somente app reconstruído/recriado. Banco/broker/frontend permaneceram ativos, com dados/credenciais preservados. Ambos os filhos iniciaram automaticamente. Management autenticado confirmou topologia; controles diretos/interativos e stop intencional passaram; HTTP saudável durante relay parado; falhas inesperadas reiniciaram ambos. Contagens compartilhadas permaneceram zero. Isso comprova disponibilidade de fila/topologia/controle, não persistência de payload não vazio: nenhuma mensagem compartilhada estava disponível ou foi inventada. Formulário de login no navegador não foi operado; página host/autenticação API verificadas. [Validação executada](../testing/validation-results.md#rollout-local-autostart--2026-10-13).

Descoberta geral de poison/esgotados permanece 3.4, replay autenticado/auditado 3.5 e aceitação/fechamento 3.6. Simulações frontend inalteradas, sem integração de dashboard vivo.

## Consumidor opt-in — 16/10/2026

Configuração fonte agora contém três programas: HTTP/relay mantêm autostart=true; consumer usa autostart=false. start all/restart all inicia consumer também. Use comandos nomeados; o programa novo não está instalado no manager vivo. Grace Compose é 95s para três filhos limitados. [Configuração, topologia, pré-requisitos de schema e procedimento de rollout autorizado](../architecture/milestone-4-3.md#supervisor-e-rollout-compartilhado-pendente). Resultados históricos acima mantêm suas datas.
