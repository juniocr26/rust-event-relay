[English](../../en/operations/milestone-4-5-rollout.md) | [Catálogo](../index.md)

# Marco 4.5 — rollout local controlado e verificação operacional

Executado em 2026-10-16 mediante autorização explícita para desenvolvimento local. **Rollout local concluído.** Base `a1b0b389fe2216618298614c144f4187c3257666`; árvore inicialmente limpa, inclusive arquivos não rastreados. Nenhum AGENTS.md de projeto/ancestral aplicável. A metodologia do Dev Career Copilot invocado e os contratos/fontes atuais orientaram o trabalho. Somente rust-event-relay foi alterado, sem stage, commit ou push. Este registro substitui afirmações anteriores de rollout compartilhado pendente; a [aceitação 4.4](milestone-4-4-review.md) preserva sua evidência histórica isolada.

## Baseline e configuração

A inspeção confirmou o relato anterior: HTTP/relay executavam inodes removidos de binários antigos; Supervisor instalado sem consumer/launcher; quatro migrations; outbox vazio; replay desabilitado; relay.local durável classic vazia, sem entregas não confirmadas/consumidores; quarentena ausente. Nenhuma mensagem inesperada ou writer concorrente identificado. Frontend ilustrativo.

SHA-256 inicialmente carregado: HTTP `0896f95c8b9c901ca01b2f83f52cf9fad2bf6d62c34ba9e01c59bdf360f4a502`; relay `b3f0490f339a90f06910673decf553c9cd00a426632c8a24d1b0a74861014f92`. Não é possível atribuir revisão exata de fonte a esses artefatos antigos.

Revisados parsers/código, exemplo .env e configurações locais privadas, Compose, injeção real, ambientes dos processos, launchers, SQL, contratos consumer/replay e diagnósticos existentes. Credenciais obrigatórias presentes e autenticadas; nenhuma foi impressa, gerada, rotacionada ou alterada. Variáveis consumer e REPLAY_OPERATORS faltavam na injeção do container antigo. A recriação instalou o mapeamento atual; todos os valores mapeados coincidem com Compose e ambiente inicial de cada filho. Nenhum valor específico do usuário bloqueia este rollout. REPLAY_OPERATORS e candidatos de manutenção continuam vazios; ativação privilegiada não foi necessária.

Variáveis do processo prevalecem sobre dotenv, que completa ausências procurando de `/app/backend` até `/app/.env`; interpolação do shell Compose prevalece sobre .env raiz. APP_ENV/HTTP_ADDR/RUST_LOG mantêm o fallback dotenv montado documentado. Endpoints internos continuam postgres:5432 e rabbitmq:5672. Escolhas locais preservadas. O [inventário](../docker/environment-inventory.md) define limites e requisitos condicionais; startup/preflight atual comprova admissão, não capacidade produtiva. Identidade `inventory.stock.v1`, prefetch 1, uma operação ativa; operação/drain/shutdown padrão 10/15/25 s; limite Supervisor 30 s por filho e grace Compose 95 s.

O papel PostgreSQL de desenvolvimento continua superuser/CREATEDB. Privilégios necessários funcionam, mas **least privilege não foi instalado**. Permissões configure/write/read RabbitMQ continuam limitadas a `^relay[.].*`. Implantação ampliada exige revisão específica de papéis/grants restritos e autoridade sobre topologia. Nenhuma credencial/configuração de API foi adicionada ao frontend.

## Backup com writers parados e restauração

HTTP e relay foram parados graciosamente por nome antes do backup final; consumer ainda não instalado, replay desabilitado. Checkpoint: outbox=0, leases ativos=0, outras sessões no banco=0, source ready/unacked/consumers=0/0/0. PostgreSQL/RabbitMQ permaneceram ativos. Isso delimita atividade observada da aplicação, sem impedir futuras conexões administrativas.

Backup no host: `.local-backups/20261016T153255Z/pre-milestone-4-5.dump`, dentro do checkout, fora de arquivos rastreados e de armazenamento exclusivo do container. Criado em **2026-10-16 15:32:55 UTC / 12:32:55 America/Sao_Paulo**, 8.943 bytes, SHA-256 `6a82bb0801cb33e65d8dfcbdee317c9c27b390f86960de8a8b3bb7c1dd4a03d2`. Diretórios 0700; dump/registros privados 0600. Caminho absoluto exato, checksum e identidade do banco de restauração estão no `verification.json` protegido ao lado. Git e contexto Docker excluem `.local-backups/`; nunca force-add/publicar. Esta cópia local não cobre perda do host.

Ferramentas do container PostgreSQL 18.6: `pg_dump -Fc` completo transmitido ao host com status verificado. Restauração em banco novo `relay_restore45_…` com UUID exclusivo, usando `pg_restore --exit-on-error --single-transaction`, preservando ownership/ACL no mesmo cluster. Comparados independentemente os quatro registros/checksums de migration, contagem outbox e dump completo de schema (excluindo somente marcadores restrict aleatórios). Tudo coincidiu. Removido apenas aquele banco de teste, com ausência confirmada. Nenhuma migration compartilhada foi aplicada antes da restauração bem-sucedida.

Escopo: schema/dados/ACL de um banco, incluindo outbox/histórico naquele checkpoint. Não inclui backup de mensagens/topologia RabbitMQ, papéis globais/senhas/tablespaces, outros bancos, segredos de configuração ou efeitos externos. As linhas da demonstração foram criadas depois e não estão nesse backup. Referências oficiais: [escopo pg_dump](https://www.postgresql.org/docs/18/app-pgdump.html) e [opções pg_restore](https://www.postgresql.org/docs/18/app-pgrestore.html).

## Migrations e instalação

Os quatro SHA-384 aplicados foram conferidos com a fonte antes da mudança. Revisados exatamente os upgrades aditivos pendentes:

- `20261013000000`: ledger replay/FK/checks, trigger de auditoria imutável e revogação PUBLIC da tabela.
- `20261014000000`: schema consumer, estoque/recibos, PK/FK diferida, checks de conclusão coerente, trigger de completude diferido, proteção de identidade/remoção/truncate e revogações PUBLIC.

Sem operação destrutiva nos ups ou conflito com dados existentes identificado. `sqlx migrate run` explícito aplicou somente essas duas versões, em ordem. As seis entradas finais de sucesso/checksum coincidem com fonte. Inspecionadas tabelas, constraints, três funções e quatro triggers de usuário, incluindo completude diferida e bloqueio de truncate. Sem reescrever SQL aplicado, migration alheia, down ou migração no startup.

Executado `cargo build --locked --bins` com writers parados; Cargo validou entradas atuais e reutilizou artefatos atualizados. Imagem app construída; recriado **somente app** com `docker compose up -d --no-deps app`, inicialmente mantendo consumer autostart=false. HTTP/relay retomados deliberadamente, com schema/health válidos. Conferidos bytes Supervisor fonte/instalado, launchers executáveis, diretórios `/app/backend` e caminhos `/app/target/debug`.

Depois da ativação segura e dos controles, consumer passou a autostart=true. Imagem reconstruída, três programas **nomeados** parados graciosamente e somente app recriado novamente. Os três iniciaram automaticamente. Identidades/armazenamento dos containers banco, RabbitMQ e frontend preservados. Sem `start all`, purge, reset, recriação de fila, mudança de credencial ou crash/outage compartilhado.

SHA-256 final carregado (cada um coincide com o executável em disco; nenhum removido):

| Programa | Caminho | SHA-256 |
| --- | --- | --- |
| HTTP | `/app/target/debug/reliable-event-relay` | `10ab2225cf166f20a0c862b244db3a404fd3ed86a5419466bcb440422e1f97d6` |
| Relay | `/app/target/debug/relay` | `5b3c3b3c593b65718e235ccf213b1e834224095a45abd48d2fd84016eb2a80c8` |
| Consumer | `/app/target/debug/consumer` | `dcb6e9957017246d109b704137cbed26578cba686419638e53b6823ebaabf80a` |

Fontes rastreadas da aplicação/arquivos Cargo coincidem byte a byte com a base. Configuração/docs do rollout compõem a árvore suja. Build locked e igualdade carregado/disco fornecem evidência de fonte atual; binários de desenvolvimento não incorporam revisão Git nem atestação de build reproduzível. Não se afirma revisão exata embutida. Verificação final privada retém hashes e horário.

## Admissão, topologia e demonstração preservada

Imediatamente antes de assinar, metadados sem buscar mensagens: relay.local ready=0/unacked=0/consumers=0, declaração classic durável compatível, binding esperado e nenhuma policy/operator policy. Confirmados source alinhado ao relay e privilégios necessários. Consumer iniciado explicitamente com `supervisorctl start consumer`. Topologia durável direct atual no vhost `relay`:

| Exchange | Fila | Routing key | Final ready / unacked / consumers |
| --- | --- | --- | --- |
| relay.events | relay.local (classic) | event | 0 / 0 / 1 |
| relay.quarantine | relay.quarantine.inventory (classic) | inventory | 0 / 0 / 0 |

Filas sem auto-delete; assinatura manual ACK/prefetch 1. Nenhum TTL, expiry, DLX, capacidade, purge ou replay introduzido. Quarentena é armazenamento preservado, sem fluxo de replay/purge. Topologia durável não equivale a backup do broker.

Identificadores preservados para estudo, com colisões verificadas:

- Evento: `481fb751-81d5-46b7-aea2-eed344084725` (UUID novo não nulo).
- Warehouse/aggregate ID: `m45-demo-6aef7312e7014b969e0b5552d872abf1`.
- SKU: `M45-DEMO-PLUS5-6aef7312e701`.

Fixture SQL conforme convenções existentes: uma transação insere estoque novo=10 e outbox pending `inventory.units_received` / aggregate `stock` / schema 1 / ocorrência pelo relógio do banco, quantidade 5. Consultas de colisão precederam escritas; constraints únicas oferecem proteção final. Demonstra o pipeline, **não integração atômica produtor/business-outbox**.

Relay ativo publicou, consumer ativo confirmou estoque=15 e um recibo (antes=10, quantidade=5, depois=15, completed_at presente). Outbox permanece processed, attempt_count=1. Helper Rust temporário usou `RabbitMqPublisher` produtivo (persistência/mandatory/confirm), restaurando o envelope imutável completo do recibo e comparando independentemente **todos os campos** com o outbox, incluindo instante temporal. Republicou mesmo ID/conteúdo; estoque permaneceu 15, conclusão/conteúdo originais do recibo inalterados. Nenhum observador basic.get competiu com consumer. Helper/envelope permanecem como evidência local privada fora do Git; fonte temporária removida de backend/examples.

Consumer também foi parado intencionalmente (parada graciosa medida ~0,246 s), permaneceu STOPPED e recebeu publicação de outra cópia idêntica na fila. Observação independente: ready=1/unacked=0/consumers=0 enquanto parado; start retomou 0/0/1 sem novo efeito. Depois da recriação final, outra publicação idêntica produziu resultado sanitizado `Duplicate(stock_after=15)` e ACK bem-sucedido. Contadores finais broker: publish=4, deliver_get=4, ACK=4, redeliver=0. Duplicação deliberada de publicação, não experimento de crash/redelivery. Permanece exatamente um estoque/outbox/recibo de demonstração; sem exclusão automática.

Estatísticas de management atrasaram uma observação inicial; inspeção posterior confirmou a mensagem retida. Nenhum evento adicional foi publicado para compensar. Status Supervisor retorna exit 3 para STOPPED/STARTING; observação de estado, não falha de controle.

## Verificação, controles e recuperação

Executados: build locked; build/config Compose; igualdade Supervisor/launchers; seis checksums/schema; autenticação PostgreSQL/management; demonstração real de retenção/parada/retomada/duplicação; HTTP health 200 e replay 404; controles nomeados supervisorctl/supervisor; PIDs estáveis sem loop de restart no intervalo observado. Regressão isolada Supervisor consumer passou (1 teste), mantendo override opt-in no teste mesmo com startup local normal habilitado. Uma compilação inicial do teste foi interrompida pela recriação app (exit 137); caso focado completo reexecutado com sucesso e limpeza conferida independentemente. Sem injeção de falha compartilhada.

Probe Chromium abriu o frontend **compartilhado** na porta 5173: título/botão esperados, zero erros runtime/console e zero recursos falhos/HTTP de erro. Fonte UI e integração API não mudaram. Apresentação continua simulação local. Logs atuais app sem registros JSON WARN/ERROR; inspeção em memória sem senhas configuradas/campos payload. Somente resultados sanitizados reportados. Nenhum dump bruto de ambiente/log rastreado. Validador documental e `git diff --check` passaram. Sem bancos relay_it_/relay_migration_/relay_restore45_, recursos broker relay.it. ou diretórios Supervisor isolados ao final.

Contenção se houver comportamento incorreto:

```sh
docker compose exec -T app supervisorctl stop consumer
docker compose exec -T app supervisorctl status consumer
# Pare novas publicações outbox ao investigar o pipeline inteiro:
docker compose exec -T app supervisorctl stop relay
# Retome somente após revisar estado durável/topologia:
docker compose exec -T app supervisorctl start relay
docker compose exec -T app supervisorctl start consumer
```

Parada intencional persiste no manager atual. **Consumer autostart=true retoma em restart do manager/container.** Para contenção persistente, mudar consumer para autostart=false em normal.conf, reconstruir/instalar configuração app com parada controlada e conferir acordo antes de reiniciar manager. Não reiniciar/recriar manager casualmente durante contenção. Parar assinatura preserva mensagens ready; entregas unacked interrompidas podem voltar. Preservar recibos/identidade imutável; incerteza é resolvida por deduplicação do mesmo evento, sem excluir recibos nem incrementar manualmente. [Semântica Supervisor](https://supervisord.org/configuration.html#program-x-section-settings).

Recuperação preparada: parar writers relevantes, preservar estado atual de banco/broker e fazer novo backup protegido antes de decidir. Conferir dump/checksum retidos e restaurar primeiro em **novo banco isolado**, com ferramenta compatível e ownership inspecionado. Comparar schema/histórico/contagens e trabalho posterior ao checkpoint, mantendo investigação isolada. Restore compartilhado exige decisão explícita de reconciliação: pode apagar dados posteriores e deduplicação sem reverter publicações, ACKs ou efeitos externos. Restore/down automático não é estratégia de recuperação. Preferir correção forward revisada quando possível. Não restaurar esse dump anterior ao rollout sobre o compartilhado apenas para produzir resultado verde.

## Entrada no Marco 5

Pré-requisitos do rollout local concluídos; nenhum valor específico do usuário bloqueia continuidade. Antes do Marco 5: escolher/registrar escopo real e critérios de aceitação (não existe contrato detalhado do Marco 5 neste repositório); reler este estado/contratos 4.4, confirmar saúde dos processos/filas e seis checksums, preservar fixture/recibos e usar recursos isolados para experimentos de falha/capacidade. Nova migration compartilhada exige novo backup com writers parados e restauração verificada. Definir carga/admissão de mensagens e semântica de capacidade antes de concorrência/backpressure. Transações do produtor e integração frontend ao vivo continuam trabalhos planejados separados.

Antes de implantação ampliada: instalar/revisar least privilege, TLS/HA, retenção independente de backups incluindo dependências broker/globais e capacidade/observabilidade. Limitações explícitas, não entregas deste rollout local. Replay desabilitado; REPLAY_OPERATORS só seria necessário para ativação privilegiada escolhida separadamente, nunca no chat/frontend. Viva Bem e Engineering Library não foram modificados.
