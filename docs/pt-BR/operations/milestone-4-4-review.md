**Estado local atual — Marco 4.5 (2026-10-16):** [Rollout verificado e recuperação](milestone-4-5-rollout.md). Seis migrations aplicadas; artefatos atuais HTTP/relay/consumer instalados e ativos; consumer autostart=true. Backup protegido/restore isolado e demonstração real 10 → 15 → 15 verificados. API replay desabilitada; frontend continua simulação. Observações anteriores abaixo preservam suas datas.

[English](../../en/operations/milestone-4-4-review.md) | [Catálogo](../index.md)

# Marco 4.4 — verificação completa e fechamento do Marco 4

Auditoria de 16/10/2026. Base `cabfbcaa3f31f2be08657c400ef98275acc78df4`; checkout inicialmente limpo, sem alterações preexistentes. Nenhum arquivo foi adicionado ao stage, commitado ou enviado. Somente rust-event-relay foi alterado. Não foi encontrado AGENTS.md aplicável no checkout ou ancestrais; instruções internas de dependências não governam a aplicação. Nenhuma skill disponível de artefatos/plugins se aplicava a esta auditoria de código e runtime.

**Decisão: Marco 4 fechado com limitações não bloqueantes explícitas.** Há evidência executável do efeito local de estoque/recibo PostgreSQL, entrega/quarentena RabbitMQ real, recuperação, ciclo de vida limitado e apresentação ilustrativa. Isso não significa projeto completo, prontidão de produção, entrega exatamente uma vez ou rollout compartilhado do consumer. Os resultados históricos de [4.1](../architecture/milestone-4-1.md), [4.2](../architecture/milestone-4-2.md), [4.3](../architecture/milestone-4-3.md), [ADR 010](../adr/010-atomic-consumer-and-quarantine.md) e [revisão do Marco 3](milestone-3-review.md) foram preservados.

## Matriz de aceitação

“Verificado” exige inspeção e observações específicas, não apenas build/health. Fontes de evidência: [consumer_runtime.rs](../../../backend/tests/consumer_runtime.rs), [postgres_consumer.rs](../../../backend/tests/postgres_consumer.rs), [integrated_recovery.rs](../../../backend/tests/integrated_recovery.rs) e seus harnesses reais. Fakes validam política, não protocolo PostgreSQL/RabbitMQ.

| Área / contrato | Classificação | Evidência e limite |
| --- | --- | --- |
| Ambiente, parsers, requisitos condicionais e injeção Compose | Implementado e verificado | [Inventário completo](../docker/environment-inventory.md), comparação sanitizada de configuração/contêiner, autenticação TCP/API e testes de protocolo. Nenhuma credencial exibida ou alterada. |
| Estoque e recibo histórico atômicos | Implementado e verificado | 12 casos PostgreSQL: 10+5=15, duplicata fica em 15, outro ID incrementa, conteúdo conflitante não altera estoque, concorrência, overflow, rejeição de comando/COMMIT, corrupção armazenada, trigger diferida e escrita com role restrita. |
| Correspondência efeito/recibo | Implementado e verificado | Ambos são escritos na mesma transação; rollback/rejeição/crash observam estoque e recibos independentemente. Trigger impede placeholder incompleto commitado. Escrita SQL privilegiada arbitrária fica fora da garantia. |
| Representação v1 e identidade no replay | Implementado e verificado | BYTEA e comparação do modelo validado, sem depender de igualdade JSONB/hash. Novo teste `milestone44_sql_replay_preserves_identity_and_consumer_effect`: ledger real → publicação pelo relay → Duplicate do consumer, um único efeito. Fixture nasce com precisão temporal PostgreSQL. |
| Publicação persistente/mandatory e ACK individual no canal original | Implementado e verificado | Confirms/returns reais; `milestone44_lost_source_ack_redelivers_committed_duplicate` interpreta e retém o frame basic.ack real, observa commit e provoca redelivery pelo fechamento do proxy próprio. Sucesso de envio do ACK no cliente não é confirmação do servidor. |
| Identidades concorrentes iguais/distintas | Implementado e verificado | Duas assinaturas reais e handlers concorrentes: uma identidade aplica uma vez; dois IDs novos incrementam a mesma linha sem perda. Testes PostgreSQL coordenam locks e conteúdo conflitante. Sem promessa de justiça/throughput. |
| Crash antes do commit | Implementado e verificado | Binário consumer bloqueado em lock de estoque observado pelo banco; SIGKILL/reap apenas do Child criado no teste; estoque 10/sem recibo; redelivery aplica 15/um recibo. |
| Crash depois do commit e antes do ACK | Implementado e verificado | Barreira de ACK somente no harness delega ao handler/adapters reais; mata o filho após fase durável; observação independente vê 15/um recibo; redelivery retorna Duplicate, estoque segue 15. Nenhum flag de falha em produção. |
| Resposta de COMMIT perdida / cancelamento após efeito durável | Implementado e verificado | Proxy PostgreSQL descarta/retém resposta de COMMIT observada no servidor. Testes integrados observam estoque/recibo por outra conexão, cancelam sem ACK e recuperam com Duplicate. Timeout não prova rollback. |
| Preservação de inválidos/não suportados/conflitos | Implementado e verificado | Bytes, message_id e propriedades originais com sidecar tipado/limitado; metadado forjado fica aninhado. Diagnósticos enumerados. ACK somente após confirm roteado. |
| Quarentena indisponível/sem rota/incerta/cheia | Implementado e verificado | Exchange ausente, binding removido/mandatory return, confirmação retida e fila exata de teste com x-max-length=1/reject-publish gera NACK; origem permanece recuperável sem ACK. Limite de capacidade não instalado no compartilhado. |
| Crash após confirm da quarentena e antes do ACK | Implementado e verificado | Barreira no filho real, kill/reap e redelivery geram duas cópias preservadas com bytes originais e nenhum efeito de estoque. Teste de confirm perdido também permite cópias. Publicação e ACK não são atômicos. |
| Mensagem acima do limite / recuperação pelo operador | Implementado e verificado | Publicação real >1 MiB: binário encerra normalmente, bytes continuam na origem, nenhum recibo/truncamento. Recuperação explícita, sem descarte automático. |
| Prefetch, perda de conexão, restart e shutdown limitado | Implementado e verificado | Casos existentes de crédito/drain/restart do binário, cancelamento no servidor/reassinatura, TERM ocioso, banco indisponível e recuperação com pausa; uma operação ativa, prefetch 1..32. Escalonamentos finitos, não todos os interleavings. |
| Migrations fresh/upgrade/down/up | Implementado e verificado | Seis upgrades SQLx ordenados/checksums SHA-384, no-op, down/re-up de consumer/replay em banco descartável exato; ausência confirmada ao final. |
| Schema/runtime compartilhado de replay/consumer | Bloqueado por configuração ou rollout | Apenas quatro migrations; schemas replay/consumer ausentes, replay desabilitado e Supervisor instalado só conhece HTTP/relay. Auditoria não aplicou migrations nem ativou consumo compartilhado. |
| Supervisor direto/interativo e programas nomeados | Implementado e verificado | Cinco casos worker_smoke mais opt-in do consumer com manager/socket/filhos próprios; stop intencional e restart após saída inesperada. Comparação instalada/source identifica drift separadamente. |
| Apresentação e navegador | Implementado e verificado | 13 testes Chromium, larguras 320/375/768/1024/1920: teclado/foco/modal, cleanup/replay, movimento reduzido, wrapping, inglês, favicon e ausência de erros/chamadas API. Renderizações mobile/desktop inspecionadas. |
| Integração frontend/backend ao vivo | Planejado e fora do escopo atual | **Presentation frontend verified; live backend integration is not implemented.** Sem fetch/XHR de API, proxy Vite, CORS ou autenticação no navegador. |
| CI | Implementado, mas não verificado remotamente | Workflow revisado; equivalentes locais passaram. Job de docs adicionado; integrado instala Supervisor e executa suite consumer completa. Nenhuma execução remota alegada. |
| HA/TLS, backup/restore, benchmark, certificação completa de acessibilidade | Planejado e fora do escopo atual | Nenhuma evidência alegada; volume persistente não comprova backup ou recuperação após perda de energia. |
| DLQ RabbitMQ para SQL dead_letter / efeitos externos | Não aplicável | Estado terminal SQL não exige fila; transação cobre somente estoque/recibo locais. HTTP/email e outros efeitos estão ausentes. |
| Defeitos confirmados pendentes | Defeituoso: nenhum restante no escopo auditado | Correções abaixo revalidadas. Drift instalado é pendência de rollout, não evidência de atualização dos processos. |

## Correções e investigação

1. **Texto obsoleto:** cenário de duplicata afirmava que o consumer não existia, contrariando 4.2/4.3. Corrigidos cenário, fluxo, estado de espera e input para ilustrar a deduplicação de estoque atual, com 10+5=15 e indicação explícita de simulação. Expectativa de navegador atualizada; nenhuma integração API criada. [Cenários](../../../frontend/src/scenarios/index.ts), [regressão de navegador](../../../frontend/tests/experience.spec.ts).
2. **Cache Vite compartilhado:** depois de npm ci, probe Chromium somente de leitura em 5173 encontrou landing ausente e um recurso com falha; servidor limpo em 5174 passou. Reiniciado **somente frontend**. Repetição do probe: uma landing, zero erros de console/runtime e zero recursos falhos. Backend/banco/broker intactos; testes mantêm cache/servidor próprios.
3. **Lacunas de evidência:** nove casos substantivos novos e uma entrada auxiliar, incluindo perda real do frame ACK. Fixture nova de replay inicialmente usou scope incorreto e falhou na constraint; corrigida para `outbox:replay` e precisão do banco, mantendo a asserção Duplicate. Erro de compilação do guard de proxy foi corrigido com JoinHandle genérico. Suites finais passaram; nenhuma asserção enfraquecida.
4. **CI/portabilidade/docs:** teste Supervisor consumer era omitido remotamente e assumia `/app/backend`. Configuração isolada agora usa CARGO_MANIFEST_DIR; CI instala Supervisor e roda todos os casos. Adicionados validador de links locais Markdown, quatro testes de fronteira e job de docs. Seis links históricos para repositório irmão viraram referências explícitas ao caminho externo, permitindo clone independente; resultados históricos preservados. Nenhum arquivo de engineering-library foi alterado. Validador não cobre links remotos, âncoras ou qualidade de tradução.

Os testes finitos não encontraram defeito confirmado no código de produção de estoque/ACK/quarentena. A auditoria ampliou evidência sem mudar contratos/dependências.

## Banco, broker e processos compartilhados

Metadados somente de leitura: PostgreSQL saudável/autenticado em postgres:5432 e host 127.0.0.1:5433; outbox compartilhado com zero linhas. Versões aplicadas com sucesso: 20261007000000, 20261007175358, 20261009000000, 20261012000000; quatro checksums SHA-384 iguais ao código. Pendentes: 20261013000000 (replay) e 20261014000000 (consumer). Nenhuma migration compartilhada aplicada. Preflight do consumer sem schema falha antes da assinatura.

A role de desenvolvimento configurada é superuser/CREATEDB: permite bancos descartáveis, mas não comprova least privilege implantado. Uma regressão nova com role descartável NOLOGIN verificou os grants documentados: Applied/Duplicate permitidos, remoção/alteração de identidade/DDL negados; ausência da role verificada após cleanup. [4.2](../architecture/milestone-4-2.md) explica grants restritos, retenção e bypass por owner/DDL. Revisar permissões antes de ampliar implantação, sem mudar senhas persistidas para satisfazer exemplos.

| Recurso no vhost `relay` | Esperado pelo código atual | Observado no compartilhado |
| --- | --- | --- |
| Exchange/fila de origem relay | relay.events direct durável → relay.local classic durável, chave event | Presentes; x-queue-type=classic, sem auto-delete; ready=0, unacked=0, consumers=0 |
| Origem consumer | mesmos defaults explícitos relay.events/relay.local/event | Recursos compatíveis; consumer desabilitado. Outra responsabilidade precisaria de fila própria. |
| Quarentena | relay.quarantine direct durável → relay.quarantine.inventory durável, chave inventory | Ambos ausentes, esperado até rollout explícito. Propostas antigas do 4.1 não são a topologia atual. |
| DLX/TTL/expiração/capacidade | Sem descarte/DLX/TTL automático; capacidade futura deve rejeitar publicação | Sem policies/operator policies; policy efetiva da origem vazia. Nenhuma policy de teste compartilhada. |
| Management/permissões/conexões | API autenticada loopback 15672; configure/write/read ^relay[.].* | API/CLI verificados; uma conexão existente visível, zero consumidores na inspeção final. Login pelo formulário não certificado no 4.4. |

Nenhuma fila compartilhada foi consumida, recebeu mensagens de teste, foi purgada, removida ou redeclarada. SQL dead_letter é independente de quarentena RabbitMQ; somente topologia UUID própria de teste foi criada/removida.

Docker versionado usa caminhos minúsculos, COPY com chmod explícito, usuário developer, `/app/backend`, binds/caches/target, diretório PostgreSQL e volume RabbitMQ persistidos, portas loopback e dependências saudáveis. Entrypoint cria diretório Supervisor 0700, socket 0600. Programas usam TERM/grupos e stopwaitsecs=30; consumer autostart=false; grace do Compose fonte=95s. Controles diretos/interativos supervisorctl/supervisor passaram isoladamente. `start all` / `restart all` podem iniciar consumer mesmo com autostart=false; use nomes.

normal.conf instalado difere do versionado; relay-consumer ausente. Wrappers relay-worker/relay-http/supervisor correspondem. HTTP PID 11 e relay PID 12 permaneceram ativos. Hashes dos executáveis carregados diferem dos artefatos recompilados; `/proc/<pid>/exe` aponta para inodes antigos `(deleted)`. Não há revisão embutida, logo o commit do código carregado é indeterminável. Os três binários atuais compilaram; Cargo atualizou `/app/target`, portanto um próximo restart nomeado carregará artefato novo. Compilação não atualiza processo existente. Nenhum rebuild/recreate/reload/restart do app ocorreu; somente frontend reiniciou para resolver o recurso obsoleto.

## Verificações executadas e contagens substantivas

| Verificação | Resultado real |
| --- | --- |
| cargo fmt --check; Clippy estrito --all-targets --all-features | Passaram no código de testes final |
| cargo test --locked | 87 passaram, 120 ignorados; zero exemplos de doc-test. Ignorados não são evidência da suite padrão. |
| PostgreSQL repository / ownership-schema / ownership / retry / discovery / replay / consumer | 15 / 1 / 12 / 18 / 7 / 10 / 12 substantivos passaram |
| RabbitMQ publisher / conexão própria fechada | 2 / 1 substantivos passaram |
| integrated_recovery | 16 substantivos + 1 entrada de filho no-op |
| consumer_runtime final | 19 substantivos + 1 entrada de filho no-op; nove casos substantivos novos no 4.4 |
| worker_smoke completo / Supervisor isolado | 5 substantivos passaram, incluindo controles direto/interativo |
| Total opt-in único | 118 substantivos + 2 auxiliares no-op. Repetições de desenvolvimento não somadas. |
| cargo build --locked --bins; rustdoc estrito | Três binários e docs no-deps passaram com RUSTDOCFLAGS=-D warnings |
| npm ci; type-check; build | Passaram; browser inicialmente ausente, Chromium/dependências instalados e execução corrigida |
| npm test | 13 casos Chromium passaram; dedupe final repetido com asserções reforçadas de texto/input/10+5; servidor/cache próprios na 5174; renderizações mobile/desktop inspecionadas |
| Migrations, Compose, PostgreSQL/RabbitMQ somente leitura | Passaram; banco exato descartável removido; checksums/configuração/topologia compartilhados inspecionados separadamente |
| Sintaxe Python/shell | Oito arquivos Python parseados; seis scripts/launchers shell passaram sh -n |
| Validador docs / testes / git diff --check | Passaram; quatro testes do validador; links de arquivo locais, sem validação remota/âncoras |

Comandos realmente utilizados a partir da raiz; Cargo no app trabalha em `/app/backend`:

```sh
docker compose exec -T app cargo fmt --check
docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings
docker compose exec -T app cargo test --locked
docker compose exec -T app cargo test --locked --test postgres_repository --test delivery_ownership_schema --test postgres_ownership --test postgres_retry --test postgres_discovery --test postgres_replay --test postgres_consumer --test rabbitmq_publisher --test integrated_recovery --test consumer_runtime -- --ignored --test-threads=1
docker compose exec -T app cargo test --locked --test worker_smoke -- --ignored --test-threads=1
docker compose exec -T app cargo test --locked --lib infrastructure::rabbitmq::tests::closed_owned_connection_is_unavailable -- --ignored
docker compose exec -T app cargo build --locked --bins
docker compose exec -T -e RUSTDOCFLAGS='-D warnings' app cargo doc --locked --no-deps
python3 scripts/check-migrations.py
sh scripts/check-postgres.sh
python3 scripts/check-rabbitmq.py --require-topology
docker compose config --quiet
docker compose exec -T frontend npm ci
docker compose exec -T -u root frontend npx playwright install --with-deps chromium
docker compose exec -T -u root frontend npm run type-check
docker compose exec -T -u root frontend npm run build
docker compose exec -T -u root frontend npm test
python3 -B scripts/check-docs.py
python3 -B -m unittest discover -s scripts -p 'test_*.py'
git diff --check
```

O caso novo de role restrita foi executado explicitamente com `cargo test --locked --test postgres_consumer restricted_consumer_grants -- --ignored`. O comando opt-in longo passou inicialmente nos casos existentes; consumer_runtime alterado foi repetido por completo, assim como Clippy/formatação/testes padrão finais. `scripts/check-supervisor.py` compartilhado não foi executado porque interrompe filhos; os casos isolados forneceram essa evidência. CI foi revisado/configurado, não executado remotamente. Sem upgrade de dependências.

## Limpeza, ações do operador e estudo

Inspeção final independente de catálogo/API/filesystem: zero bancos relay_it_/relay_migration_, zero filas/exchanges relay.it., zero roles NOLOGIN de teste e diretórios/arquivos temporários próprios de recovery/Supervisor/barreira; origem compartilhada segue ready=0/unacked=0/consumers=0. Somente Child handles criados pelo harness receberam sinais; filhos reaproveitados nunca foram sinalizados. Proxies foram encerrados e fixtures verificaram ausência exata. Caches ignorados, browsers, screenshots/logs continuam como artefatos de validação. O 4.4 não promete/testa limpeza após encerramento abrupto do harness inteiro; o harness anterior testa separadamente cleanup após asserção falhar.

Nenhum segredo obrigatório ausente/vazio/inválido bloqueia aceitação local. Para rollout compartilhado: revisar backup/recuperação e conteúdo real da fila; aplicar migrations pendentes em ordem explicitamente; verificar permissões restritas do banco/broker; revisar admissão/capacidade/policies; rebuild/recreate somente app para instalar Supervisor/ambiente/launchers conhecidos e escolher restarts nomeados HTTP/relay. Manter consumer parado até autorização explícita para consumir origem real. Se replay for desejado, provisionar `REPLAY_OPERATORS` no formato protegido do [inventário](../docker/environment-inventory.md), sem enviar segredos pelo chat. Replay/purge da quarentena exige outro protocolo.

Para estudar, siga `handle_delivery` → `PostgresInventoryProcessor::process` → migration de completude do recibo → RabbitDelivery::ack/publisher de quarentena, depois os testes das fronteiras. Estoque começa em 10: evento A soma 5 e commita estoque 15 mais recibo A; só depois ACK. Duplicata A precisa ter o mesmo conteúdo imutável e retorna recibo histórico, mantendo 15. B distinto soma 5 e chega a 20. A conflitante não muda estoque e precisa ser preservado antes do ACK. Perder resposta do COMMIT/ACK torna o progresso incerto: consultar/reprocessar a mesma identidade, sem somar/compensar cegamente. Crash após confirm da quarentena pode produzir duas cópias e nenhum efeito no estoque. Replay preserva A: não adiciona outros cinco nem desfaz seu efeito. São garantias da transação local nos cenários testados, não entrega exatamente uma vez nem atomicidade de efeitos externos.

Referências primárias consultadas: [precedência de ambiente Compose](https://docs.docker.com/compose/how-tos/environment-variables/envvars-precedence/), [ACK/confirms RabbitMQ 4.3](https://www.rabbitmq.com/docs/confirms), [READ COMMITTED PostgreSQL 18](https://www.postgresql.org/docs/18/transaction-iso.html). Código instalado de Lapin/SQLx e testes reais governam detalhes do cliente. Limitações não bloqueantes: frontend sem API, rollout de least privilege/HA/TLS, perda de energia no broker, backup/restore, benchmark, Firefox/WebKit e certificação integral de acessibilidade não demonstrados. Próximo passo recomendado: rollout local revisado separadamente, seguido de observação independente de estoque/recibos/topologia; funcionalidades novas precisam de escopo próprio.
