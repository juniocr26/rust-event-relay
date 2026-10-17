[English](../../en/operations/milestone-6-3.md) | [Guia da aplicação](application-scenarios.md)

# Milestone 6.3 — aceitação final

Estado: aceito localmente em 17/10/2026. Milestone 6 e escopo local de portfólio acordado encerrados após aprovação dos critérios obrigatórios. CI remoto não verificado; prontidão de produção não estabelecida. Registros 6.1/6.2 preservam sua evidência datada.

Implementado neste candidato: paginação autenticada e limitada de execuções retidas, inclusive pendentes; recuperação somente por leitura separada de submissão explícita; seleção e persistência de identidade de ações históricas; token apenas em memória; freshness; SQL copiável/topologia; documentação de instalação e estudo reconciliada em inglês/português. Nenhuma migração alterada; componentes azuis e modal preservados.

A [matriz detalhada](../../en/operations/milestone-6-3.md) relaciona cada capacidade ao código/teste: envelope, atomicidade/idempotência do produtor, ownership/leases/revisões, retry e terminal, descoberta limitada, replay autorizado/auditoria, deduplicação e conflitos, quarentena antes de ACK, limites de recursos, métricas/readiness/freshness, pipeline real, inspeção/reload, autenticação/Origin, Docker/Supervisor/persistência, acessibilidade, migrações e CI/documentação. As capacidades do escopo local foram implementadas e verificadas agora; execução de CI remoto permanece não verificada. Replay é opt-in e permanece desativado no ambiente normal. Playback de crash/race/perda de ACK é ilustrativo; TLS/HA, caos/reparo genérico, retenção automática e prontidão de produção estão fora do escopo local.

## Falhas controladas

`python3 -B scripts/measure-isolated.py verification` cria PostgreSQL/RabbitMQ e rede com identificação exclusiva, limites finitos e limpeza exata. Testes existentes injetam perda de confirmação de transação, crash/cancelamento, incerteza de confirm, rejeição, quarentena/ACK e corridas de ownership. Cada teste define ponto de injeção e asserts independentes; nomes não comprovam execução. `scripts/test-application.py` interrompe um serviço por vez na composição descartável. Recuperação interativa rejeita apenas seu evento antes do envio. Não use controles destrutivos de `check-supervisor.py` no ambiente principal para este aceite.

A correção de sessões concorrentes fecha a sessão após seu ACK atribuído, devolvendo mensagens prefetched pendentes à fila. Ações concorrentes repetidas precisam confirmar com um recibo e estoque 15; limpeza não pode deixar consumidores inesperados.

## Registro de verificação

Comandos/resultados finais estão registrados abaixo e no [registro estruturado](../../en/testing/milestone-6-3-results.json). Falhas obrigatórias impedem encerramento; funcionalidades futuras opcionais não substituem lacunas obrigatórias. O teste de instalação copia arquivos candidatos visíveis ao Git, sem caches/segredos/binários ignorados. Nenhum stage, commit ou push é necessário.

### Verificação local em 17/10/2026

- `python3 -B scripts/measure-isolated.py verification`: fmt/Clippy estrito aprovados; suíte padrão com 102 testes aprovados e 141 ignorados nessa invocação; 138 entradas de integração aprovadas (136 substantivas e dois dispatchers de subprocessos), mais dois testes substantivos de broker da biblioteca. Build final e rustdoc com warnings negados também aprovados.
- `python3 -B scripts/test-application.py`: 27 testes de navegador aprovados: consumidor pausado (1), apresentação/pipeline/histórico (24), fixture unknown (1), API indisponível (1). SQL independente confirmou resultados; sete filas de ações ficaram vazias e sem consumidores temporários.
- `python3 -B scripts/check-migrations.py`: nove upgrades/checksums ordenados, no-op e rollback/reapply suportados aprovados; banco descartável removido.
- `docker compose exec -T frontend npm run build`: type-check Vue e build aprovados. Instalação locked também é exercitada nos harnesses descartáveis.
- `python3 -B scripts/check-docs.py`: 125 arquivos Markdown, zero links locais não resolvidos; não verifica âncoras ou links remotos. Quatro testes do checker e `git diff --check` aprovados.
- Campanha de capacidade aprovada: um teste com 22 execuções, zero resultados inexplicados. Instalação limpa final aprovada, conforme registro abaixo.

Inspecionadas capturas reais de celular/tablet/desktop: larguras 320/375/768/1024/1920, identificadores longos do histórico em 320/768/1920, unknown e stale. Teclado, restauração/contenção de foco, reduced motion e ausência de overflow passaram. Pipeline não foi simulado por mocks.

Falhas verificadas incluem crash antes do envio/após confirm; perda de confirmação; rejeição/mandatory return; perda de confirmação de COMMIT; cancelamento/locks/stale owner; quarentena indisponível/cheia/confirm perdido; crash do consumidor antes/depois de commit; ACK perdido e redelivery; prefetched copies nas sessões concorrentes; rejeição interativa antes do envio. Oráculos são SQL independente, estado do broker e limites dos testes. A fixture UI unknown altera apenas estado em banco de teste: não representa outro experimento de crash. Recuperação real de worker interrompido é verificada separadamente em `demo_runs`.

Tentativas anteriores com falha não contam como sucesso: timeout de infraestrutura, permissão do cookie no probe root do broker, corrida STARTING/socket do Supervisor, expectativa antiga de submissão automática e preparação de histórico excedendo cota. Correções limitam schedulers, executam probe como usuário do broker, aguardam readiness, confirmam admissão explícita antes de fechar e respeitam os 10/s existentes. Asserções e deadlines de confiabilidade não foram relaxados. Um snapshot de instalação já substituído foi interrompido e limpo antes de repetir com o candidato concluído.


### Instalação limpa e estado final

`python3 -B scripts/test-clean-install.py` aprovado no candidato final: 289 arquivos visíveis ao Git, digest SHA-256 `06905b42520df88c35b7ee2deb5b3a72a4e4a0301020e4db7e93dccf338f4077`. Não foram copiados `.env`/`.demo.env`, volumes/banco existentes, target/cache Cargo, node_modules ou binários gerados. Credenciais privadas, nomes e portas foram novos. O fluxo literal do README executou nove migrações antes dos workers, criou topologia e aguardou readiness, sem reparos manuais. Suíte Rust padrão: 102 aprovados, 141 opt-ins ignorados nessa invocação; npm ci/type-check/build e 24 testes de navegador aprovados. SQL independente verificou os 13 runs com outbox processado, recibo 10+5=15 e estoque 15; sete ações com Duplicate correlacionado e um efeito/run. Stop/start preservou 13 runs, sete ações, 13 recibos e estoque somado 195. Nenhum segredo gerado apareceu nos assets de produção. Ausência de `.env`, token malformado e placeholders foram rejeitados separadamente antes de iniciar infraestrutura, com mensagens acionáveis sem valores privados.

O proxy TCP temporário do teste preserva Origin/porta localhost isolados; frontend/API/SQL/broker são reais. Não é outro ambiente permanente. O digest descreve arquivos no início do teste; este registro final foi escrito depois. Código de runtime não mudou após esse snapshot.

Ao todo, 243 entradas Rust aprovadas em invocações padrão/opt-in: 241 testes substantivos e dois dispatchers de subprocessos. Não somamos targets vazios ou repetições da instalação a essa contagem única. Capacidade: 22 execuções entre Baseline, Duplicates, Contended, Larger, Paused, Locked, Invalid e Stale, todas com unexplained=0. O observador coletou 200 amostras sem erro, máximos agregados observados de 79 mensagens ready e uma unacknowledged. São observações locais em VM Docker compartilhada, sem garantia de throughput/latência de produção. Limpeza de bancos/filas e remoção exata de containers/redes/volumes de teste comprovadas.

`python3 -B scripts/start-local.py` também aprovado no ambiente principal. Nove migrações/checksums já atuais; nenhuma migração principal, alteração de credencial ou restauração necessária. App/frontend/broker recriados para a configuração candidata; PostgreSQL e armazenamento durável preservados. HTTP/relay/consumer RUNNING, três readiness 200 e SHA-256 dos executáveis carregados iguais aos binários candidatos no disco.

Contagens e fingerprints canônicos das 11 tabelas permaneceram iguais antes/depois: outbox, replay, estoque, recibos, runs, fatos do produtor, ações, publicações, observações, faults e ledger de migrações. Fingerprint combinado `227e045692aae560b23fe417d47106467f350481faa40de33eb53b2e698bdd1a`. Principal: um demo run/ação, dois recibos globais incluindo efeito anterior não-demo, nove migrações; demo com estoque 15 e um recibo. Filas de origem/ação/quarentena sem ready/unacknowledged; consumidor principal ativo, nenhum consumidor temporário na ação. Histórico autenticado consultado sem POST. Arquivos/volumes legados preservados; sem purge, exclusão de recibos, reset de volumes, stage, commit ou push.

Bloqueios restantes: nenhum para o aceite local acordado. CI remoto não executado/verificado; publicação pública, TLS/HA, recuperação/retenção de produção e reparo genérico ficam fora deste aceite.
