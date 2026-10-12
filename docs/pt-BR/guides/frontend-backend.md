# Preparação frontend/backend — 2026-10-12

O projeto separa `frontend/` (apresentação Vue) e `backend/` (Rust, entrega, ownership e persistência). A preparação não implementa dashboard nem Marco 3.

## Estrutura e responsabilidades

`backend/` contém Cargo.toml, Cargo.lock preservado, src, tests e migrations; `frontend/` contém a base Vue/TypeScript/Vite/Tailwind e lockfile npm. Documentação, scripts de operação e compose.yaml ficam na raiz. Dockerfile fica em `docker/Dockerfile`, configuração física em `docker/supervisor/normal.conf` e broker em `docker/rabbitmq.conf`. Não há workspace Cargo na raiz nem camadas vazias.

Models: domain e persistence/model. Interfaces: contratos focados em persistence e application/publisher. Repositories: infrastructure/postgres. Adapters: infrastructure/rabbitmq, mantendo sua interface. Use Cases: application/delivery e coordenação application/worker. Controllers: fronteira HTTP em application.rs; health fixo não precisa de caso de uso artificial. Composition roots montam infraestrutura. Dependências apontam para modelos e contratos independentes de SQLx, Lapin e Vue. Serviços e helpers só devem existir com responsabilidade concreta; módulos públicos Rust não foram renomeados.

Componentes Vue são Views com Composition API e script setup; composables coordenam apresentação e adapters de API implementar contratos quando necessários. Componentes reutilizáveis usam props tipadas e eventos explícitos. Regras de entrega, repositories e autoridade permanecem no backend. A aplicação atual apresenta cinco cenários ilustrativos em modal acessível, sem chamadas API ou operações de infraestrutura.

## Ambiente e comandos

Em `frontend/`, use Node 22.12+ e npm: `npm ci`, `npm run dev`, `npm run type-check`, `npm run build`. Versões verificadas: Vue 3.5.43, Vite 8.3.4, plugin-vue 6.0.10, Tailwind e plugin 4.3.3, vue-tsc 3.3.13 e TypeScript 5.9.3. TypeScript 7 falhou com vue-tsc; o pin 5.9.3 passou após nova instalação pelo lockfile. Seguidas orientações oficiais de [Vue](https://vuejs.org/guide/quick-start.html), [Vite](https://vite.dev/guide/) e [Tailwind](https://tailwindcss.com/docs/installation/using-vite).

Na raiz: `docker compose build app`, `docker compose exec app cargo build --locked --bins`, `docker compose up -d --no-deps frontend`. Cargo nativo roda em `backend/`. O app usa `/app/backend`; SQLx encontra migrations nesse diretório. Contexto de build continua raiz, Dockerfile explícito. Frontend usa imagem Node diretamente, sem Dockerfile adicional, instala com npm ci e publica `127.0.0.1:5173` (HOST_FRONTEND_PORT), com bind mount e HMR. Rust continua exigindo build e reinício explícitos.

`.env` ignorado da raiz foi preservado; dotenvy busca diretórios ancestrais no backend nativo, mantendo precedência das variáveis do processo. `frontend/.env.example` só documenta convenções: `VITE_*` é público, nunca credencial; nenhuma variável de API é consumida pelo protótipo. Caches `.cargo-cache` e `target` continuam na raiz, com CARGO_HOME=/app/.cargo-cache e CARGO_TARGET_DIR=/app/target. Cargo nativo usa backend/target por padrão; evite compartilhar artefatos de plataformas distintas. Não foram copiados caches nem movidos dados PostgreSQL/RabbitMQ.

## Supervisor e RabbitMQ

`normal.conf` é instalado em `/etc/supervisor/supervisord.conf`, preservando descoberta padrão. Launchers executáveis usam binários compilados em /app/target/debug. `supervisorctl` e alias `supervisor` aceitam status/start/stop/restart para http, relay e all, além de console interativo sem argumentos. Supervisor gerencia processos, não filas; broker permanece serviço separado. Socket Unix 0600, usuário developer, grupos de processo, autorestart inesperado, logs e limites de 30s/65s foram preservados.

**`start all` inicia relay mesmo com autostart=false.** Não usar no ambiente compartilhado antes do rollout autorizado. Relay permanece STOPPED e migration 20261009000000 pendente. Controles coletivos e de relay são validados somente em banco e recursos de broker isolados.

Painel em [http://localhost:15672/](http://localhost:15672/), publicado em loopback com credenciais locais existentes e permissões limitadas ao projeto. `python3 scripts/check-rabbitmq.py` verifica HTTP, autenticação, vhost, listas de exchanges/queues/bindings e permissões sem mostrar segredos. Porta existente é preservada. Topologia pode surgir somente quando publisher conecta; não criar mensagens nem iniciar relay compartilhado para popular o painel. Variáveis de inicialização não alteram credenciais de volume existente. Login pelo formulário do navegador não foi testado: não há capacidade autorizada de browser disponível.

## Validação e continuidade

CI mantém integrações isoladas PostgreSQL/RabbitMQ/recuperação e passa Cargo para backend. Adicionados build dos binários, rustdoc estrito e job npm ci/type-check/build. CI remoto configurado, não executado localmente. Validar fmt, Clippy estrito, testes, binários, rustdoc e suites opt-in com bancos isolados e nomes UUID no broker; comandos e testes do frontend ficam no [guia](../../../frontend/README.md).

Semântica de entrega permanece fundação at-least-once sob hipóteses explícitas: confirmação não prova efeito no consumidor, ownership não cerca publicação externa e duplicatas exigem idempotência. Leia a [revisão do Marco 2](../operations/milestone-2-review.md). Protótipo UI implementado; integração real, Marco 3 e rollout compartilhado exigem escopos separados.

## Resultado local e limitação operacional

Checks básicos Rust, suites PostgreSQL isoladas, publisher/conexão, três worker_smoke (incluindo controles coletivos Supervisor isolados) e 11 integrated_recovery passaram. npm ci/type-check/build e smoke HTTP Vite passaram. IDs PostgreSQL/RabbitMQ preservados. Migration compartilhada permanece ausente e relay parado. Validador documental: zero erros, 18 avisos de fixtures de imagem restritas em outro projeto.

Credenciais atuais do root .env divergem das existentes no contêiner RabbitMQ. Diagnóstico padrão retornou 401 e publisher inicialmente falhou autenticação. Credenciais existentes autenticaram: `python3 scripts/check-rabbitmq.py --running-credentials` as verifica sem alterar .env ou usuários. Suites broker passaram com injeção dessas credenciais apenas em memória. App recriado conserva valores do .env: alinhar configuração em operação autorizada separada antes do rollout. Login browser não testado.


## Protótipo de apresentação — 2026-10-12

A landing page azul ocupa toda a viewport; o card compacto abre um dialog nativo sobre o fundo. Componentes Vue tipados reutilizam controles, lifecycle do modal, progresso, fluxo de mensagens, pulso ilustrativo, envelopes e trace explicativo. Dados dos cinco cenários ficam separados da renderização; `usePlayback` mantém um timer cancelável com etapas de 3,6 segundos, independentemente dos timestamps simulados. Fechar/desmontar cancela o timer; replay reinicia o estado. Escape/Close restauram foco; o modal contém foco, bloqueia interação de fundo e scroll do body. Conteúdo interno reflow/scroll, com animações desativadas sob reduced motion.

Cenários locais: entrega confirmada, crash após confirmação e recuperação, dois workers, acknowledgement de conclusão perdido e deduplicação futura. Etapas interrompidas, incertas, puladas e futuras mantêm rótulos explícitos. Aceitação do broker não prova processamento do consumer. Recovery pode duplicar o event ID; tokens protegem transições no banco, não publicação externa. CommitUncertain não prova commit/rollback; inspeção independente observa commit apenas neste cenário. Deduplicação e efeito local atômico são preview, não implementação; efeito externo não compartilha atomicidade automaticamente. Sem chamadas de rede ou mutações de infraestrutura. Tokens CSS centralizam o design. Veja [componentes e testes](../../../frontend/README.md). Marco 3 e integração real permanecem escopos separados.
