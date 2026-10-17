[English](../../en/operations/milestone-6-1.md) | [Catálogo](../index.md)

# Marco 6.1 — entrega real com evidência persistente

Escopo: somente entrega bem-sucedida. [ADR 011](../adr/011-persistent-demo-runs.md). Crash, disputa, perda de confirmação e duplicação permanecem ilustrativos. Sem CDC, controle de caos, credenciais administrativas no bundle ou promessa de transporte exatamente uma vez.

## Operação isolada

Na raiz do repositório, com Docker Desktop:

```sh
python3 scripts/demo-stack.py init
python3 scripts/demo-stack.py infra
python3 scripts/demo-stack.py build
python3 scripts/demo-stack.py migrate
python3 scripts/demo-stack.py start
python3 scripts/demo-stack.py status
python3 scripts/demo-stack.py stop
```

Parar preserva volumes; para retomar, executar `infra` e `start`. `init` cria `.demo.env` privado/ignorado e preserva credenciais existentes. Abrir esse arquivo localmente e informar DEMO_TOKEN no modal. O token fica apenas em memória; o UUID da requisição fica no sessionStorage da aba para recuperação segura. Após sucesso confirmado, **Prepare a new run** permite outra execução intencional. Uma submissão incerta conserva a identidade.

| Serviço | Endereço | Identificação |
| --- | --- | --- |
| Interface | http://localhost:5174 | Origin exata autorizada |
| API | http://localhost:8081 | Proxy `/demo`, bearer próprio |
| PostgreSQL | localhost:5441 | Banco `relay_demo61`, usuário `demo61`, senha local DEMO_DB_PASSWORD |
| RabbitMQ management | http://localhost:15681 | Usuário `demo61`, senha local DEMO_BROKER_PASSWORD, vhost `demo61` |

AMQP interno `rabbitmq:5672`, sem porta AMQP publicada. Exchange durável `relay.events` → fila `relay.local`, chave `event`; consumidor `inventory.stock.v1`. Quarentena `relay.quarantine` → `relay.quarantine.inventory`, chave `inventory`. Estatísticas de filas são agregadas, não medições por evento. Não há curva decorativa de atividade.

O projeto Compose fixo `relay-demo61` tem broker, banco, armazenamento e rede próprios. O script verifica label do container e fingerprint `relay_demo61/demo61/5432` antes das migrations. As aplicações não executam migrations. Não há socket Docker; montagens da aplicação são somente leitura. A stack compartilhada e o evento original ficam fora das mutações.

## Transação, API e evidência

`POST /demo/runs` recebe JSON estrito com request_id UUID, scenario normal, initial_stock 10 e quantity 5. A mesma transação grava estoque inicial 10, execução em `demo.runs`, fato de negócio em `demo.goods_receipts` e evento canônico em `relay.outbox_events`. O produtor não aplica o incremento. Lock transacional e restrições duráveis impedem duplicação concorrente. Repetir a mesma intenção retorna Run ID/Event ID originais; divergência gera 409, sem resetar estoque.

`GET /demo/runs/{request_id}` consulta evidência autorizada. Ambos exigem bearer exclusivo e `X-Demo-Client: demo61`; Origin de navegador, quando presente, deve ser `http://localhost:5174`. Sem CORS/cookies. Há um único principal local com acesso às execuções desta stack; não é autenticação pública multiusuário.

202 confirma commit do produtor ou reconciliação do pedido já commitado; não confirma entrega. 422 indica rejeição conhecida da transação, 429 limite de capacidade/taxa e 503 indisponibilidade ou commit desconhecido. Após timeout, consultar/repetir a mesma identidade. Um 404 representa ausência naquele snapshot, sem autorizar automaticamente nova identidade.

Evidências independentes: fato do produtor, status/timestamps/tentativas do outbox, receipt do consumidor, efeito antes/depois, estoque atual e horário da observação. accepted_at é horário de inserção dentro da transação, não instante exato do COMMIT. Receipt pode preceder conclusão do relay. A consulta usa snapshot SQL de uma instrução, sem snapshot distribuído com o broker. Erros/deadline tornam a evidência em cache explicitamente desatualizada. Fechar o modal para apenas o polling; a execução durável continua.

Schemas: `relay` armazena publicação/retry/replay; `consumer`, receipts e efeito de estoque; `public`, metadados SQLx; `demo`, execução e fato do produtor. **Where to verify** fornece SQL somente leitura filtrado pelos identificadores reais. Para conexão: `psql -h localhost -p 5441 -U demo61 -d relay_demo61 -W`; informar senha privada no prompt. [Consultas completas em inglês](../../en/operations/milestone-6-1.md).

Execução retida: Run ID `01a149f0-bb7e-74db-96f4-750800816a7c`; request `19c660e8-3f75-4a92-a961-3ad60dd95529`; Event ID `01a149f0-bb7e-74db-96f4-750996a3bcc8`. SQL independente confirmou processed, receipt 10+5=15 e estoque 15.

## Limites, retenção e testes

Limites: 100 execuções retidas, 4 pendentes sem receipt, 2 HTTP simultâneos, 10/s, body 1024 bytes, deadline HTTP 8s. Pool API 2, acquire 2s, statement 3s, lock 1s. Polling sequencial de 1s por até 60s, cancelado ao fechar. Relay e consumidor conservam budgets finitos, pool 2 e sampler adicional por processo, prefetch 1; exporters internos com 4 conexões e labels fixas. Banco max_connections 30. Rust/banco 256 MiB e 0,5 CPU; broker/frontend 512 MiB e 1 CPU; PID 128 (broker 256), logs rotacionados 2×5 MiB. Build 3 GiB/2 CPU; Chromium de teste 2 GiB/2 CPU/512 PID. Não é prova de capacidade máxima ou limite total de disco do host.

Sem TTL, purge ou descarte automático. Atingir capacidade interrompe admissão, preservando evidências. Limpeza intencional da stack inteira exige exportar evidências desejadas, parar aplicações e remover primeiro entregas do broker dedicado, depois dados SQL:

```sh
python3 scripts/demo-stack.py stop
docker compose --env-file .demo.env -p relay-demo61 -f compose.demo.yaml down
docker volume rm relay-demo61_demo-rabbitmq
docker volume rm relay-demo61_demo-postgres
```

Isso perde todas as evidências da demo. Não executar para parada normal. Nunca excluir receipts isoladamente, usar prune global ou remover recursos compartilhados. Preservar `.demo.env` junto dos volumes; alterar o arquivo não rotaciona senhas já armazenadas.

Testes SQL criam/removem bancos únicos. `python3 -B scripts/check-migrations.py --demo` valida sete upgrades/checksums e reversões demo/consumer/replay em banco descartável. `python3 scripts/test-demo-browser.py` usa outro projeto temporário, credenciais/banco/broker novos, sem portas publicadas, API e pipeline reais. Remove somente seus recursos ao terminar; não executar duas cópias simultâneas do harness fixo. Primeira execução baixa dependências do navegador de teste.

6.2: ação finita, durável e idempotente para publicar novamente o envelope/message_id idênticos; observar Applied e **Duplicate** correlacionados, mantendo um receipt e estoque 15. Duas confirmações de publicação ou estoque estável não provam Duplicate. 6.3 amplia inspeção e recuperação de sessão. Produção, hospedagem pública, caos e coleta de atividade do broker continuam fora do escopo.

### Aceitação local — 2026-10-17

Base `f6b0ec11e8035dedbfb7ed3002157302aa827bbb`. Índices e documentos de preparação EN/PT-BR preexistentes preservados. Passaram 7 testes SQL focados (atomicidade, concorrência/idempotência/conflito, rejeição imediata e no COMMIT, confirmação perdida, cancelamento após commit, autorização/indisponibilidade e limites), 58 regressões selecionadas e 25 testes de biblioteca; dois opt-ins de broker da invocação de biblioteca não foram executados. Passaram fmt, Clippy estrito all-targets, build de binários, rustdoc e type-check/build frontend. Campanha final: 21 testes Chromium com API/pipeline reais, clipboard real, SQL independente, consumidor de teste parado, API de teste indisponível, cinco larguras, foco/reduced motion e cenários ilustrativos. Capturas 320/1920 inspecionadas; sem mocks de rotas.

Sete upgrades/checksums, no-op e reversões demo/consumer/replay passaram em banco descartável removido/verificado. Configuração Compose, parsing Python, quatro testes do validador documental, links de 117 Markdown e whitespace passaram. Tentativas iniciais do harness exigiram corrigir profile tools ausente, saída 137 no orçamento pequeno do frontend de teste e duas expectativas antigas de texto; a campanha final passou. O script de controle Supervisor foi tentado no host e parou na ausência do socket; não foi executado contra serviços compartilhados, pois interromperia seus processos.

O run retido sobreviveu a stop/start completo, seguido de SQL direto: sete migrations bem-sucedidas e uma única execução operacional retida. Evento compartilhado `481fb751-81d5-46b7-aea2-eed344084725` preservado em processed/tentativa 1, receipt 10+5→15 no mesmo timestamp original e estoque 15; `demo.runs` ausente no banco compartilhado. Sem migrations, falhas ou resets compartilhados; sem stage/commit/push. Recursos temporários do navegador foram removidos e a demo retida continua ligada. CI remoto e prontidão de produção não foram afirmados.
