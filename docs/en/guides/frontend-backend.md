**Current workflow:** run `python3 scripts/start-local.py` from the root. Rollout commands, dedicated-stack instructions and older observations below are historical; they do not replace current installation. See [application](../operations/application-scenarios.md) and [token/connectivity](../operations/demo-token-connectivity.md).

[Historical record: current installation and addresses](../operations/application-scenarios.md)

# Frontend/backend preparation — 2026-10-12

## Milestone 6.1 — current integration — 2026-10-17

Successful delivery now uses the dedicated real demo producer/API, relay and consumer. The other four scenarios remain illustrative. Earlier dated statements below are historical. Use the [6.1 runbook](../operations/milestone-6-1.md), not shared-stack migrations, for the live experience.


## Milestone 4.4 — current verification and closure — 2026-10-16

Milestone 4 is closed with explicit nonblocking limitations. Real inventory/deduplication, ACK/quarantine and recovery evidence is separate from pending shared rollout. Presentation frontend verified; live backend integration is not implemented. Earlier dated sections retain their historical scope. [Verification matrix](../operations/milestone-4-4-review.md) | [Environment inventory](../docker/environment-inventory.md).


The repository now separates `frontend/` (Vue presentation) and `backend/` (Rust ownership, delivery and persistence). The presentation prototype is now implemented; Milestone 3 remains separate work.

## Organization and responsibilities

```
frontend/                 Vue shell, npm manifest/lockfile, strict TypeScript, Vite
backend/                  Cargo.toml, Cargo.lock, src/, tests/, migrations/
docker/Dockerfile         Rust development container
docker/supervisor/normal.conf  physical http/relay configuration and launchers
docker/rabbitmq.conf     broker configuration
docs/                     bilingual shared documentation
scripts/                  shared environment diagnostics and documentation generation
compose.yaml              root orchestration
```

No root Cargo workspace is needed. Rust public modules and APIs retain their names. `domain` owns models/invariants; `persistence/model` owns persistence-facing data and meaningful conversions; focused `persistence` and `application/publisher` contracts are interfaces. `infrastructure/postgres` contains repositories; `infrastructure/rabbitmq` is the existing adapter. `application/delivery` and `application/worker` contain use cases and scheduling. HTTP handling in `application.rs` is the controller boundary; fixed health needs no artificial use case. Composition roots construct infrastructure outside application orchestration. Models/contracts remain independent of SQLx, Lapin and Vue. No empty services/helpers or generic CRUD layers were added.

Vue SFCs are views, using Composition API and script setup. Future composables may coordinate presentation; API adapters implement focused application contracts when needed. Delivery rules, database repositories and ownership authority stay in Rust. The current frontend is a full-viewport blue landing page with a compact scenario launcher and a native accessible dialog. Reusable controls, modal lifecycle, progress, message flow, illustrative activity, input/output envelopes and explanatory trace are composed from typed Vue components. Scenario fixtures are separate from rendering; `usePlayback` owns a single cancellable presentation timer and cleans up on replay and unmount. Escape/Close restore focus, the native modal makes the background inert, and body scrolling is blocked while open. Dialog content scrolls internally and reflows at mobile sizes; reduced motion disables animation without accelerating the explanation.

Five local scenarios illustrate confirmed delivery, post-confirmation crash/recovery, competing workers, lost completion acknowledgement and future consumer deduplication. Interrupted/skipped/uncertain/future stages keep their own labels. Broker acceptance is not consumer processing. Recovery can duplicate stable event IDs; database tokens do not fence RabbitMQ publication. CommitUncertain proves neither commit nor rollback; independent committed observation is this scenario's chosen result. The future consumer models deduplication plus a local database effect in one transaction, not an implemented consumer or atomic external effect. No network calls or infrastructure mutations occur. Shared CSS tokens govern the presentation. See [frontend component and test guide](../../../frontend/README.md).

## Development and environment

Use Node 22.12+ and npm, or the Node 22 Compose service. From `frontend/`: `npm ci`, `npm run dev`, `npm run type-check`, `npm run build`. Vite and Vue installation follow [Vue guidance](https://vuejs.org/guide/quick-start.html), [Vite prerequisites](https://vite.dev/guide/) and the [Tailwind Vite plugin](https://tailwindcss.com/docs/installation/using-vite). Tailwind uses `@tailwindcss/vite` and a CSS import. Versions are pinned in package.json and package-lock.json. TypeScript 7 was incompatible with vue-tsc; 5.9.3 was installed and verified. Current versions: Vue 3.5.43, Vite 8.3.4, plugin-vue 6.0.10, Tailwind/plugin 4.3.3, vue-tsc 3.3.13.

Root ignored `.env` stays in place for Compose; dotenvy searches ancestor directories during native backend startup. Existing environment variables retain precedence. Frontend `.env.example` contains safe conventions only; `VITE_*` values are public browser data and must never contain credentials. No frontend API variable is consumed yet. Local frontend env files and build/dependency directories are ignored. Frontend native dev binds localhost; Compose binds container 0.0.0.0 and publishes `127.0.0.1:5173` (HOST_FRONTEND_PORT override). Source bind mount provides Vite HMR; Rust still requires explicit compilation and process restart.

From root: `docker compose build app`; `docker compose exec app cargo build --locked --bins`; `docker compose up -d --no-deps frontend`. Backend working directory is `/app/backend`; native commands run after `cd backend`. All migrations now live in `backend/migrations`; SQLx commands in app inherit this directory. Root diagnostics stay under scripts because they inspect the whole deployment. Docker build context stays root, with explicit `docker/Dockerfile`. The frontend uses the upstream Node image directly; no extra Dockerfile is needed.

`CARGO_HOME=/app/.cargo-cache` and `CARGO_TARGET_DIR=/app/target` preserve existing host caches without moving/copying them. Native Cargo defaults to `backend/target`; optionally set `CARGO_TARGET_DIR` to the root target, but avoid mixing host and Linux artifacts. PostgreSQL `.dockerized-postgres` and RabbitMQ named volume stay untouched. No shared migrations are applied automatically.

## Processes and broker management

Versioned `docker/supervisor/normal.conf` is installed as `/etc/supervisor/supervisord.conf`, retaining default supervisorctl discovery. Executable launchers run `/app/target/debug/reliable-event-relay` and `/app/target/debug/relay` directly. `supervisor` forwards arguments to supervisorctl. Both open an interactive console without arguments and support status/start/stop/restart for http, relay and all. Example: `docker compose exec app supervisorctl status`; interactive: `docker compose exec app supervisor`. Supervisor manages application processes, never queues or rabbitmq-server. RabbitMQ stays a separate service.

`relay` stays STOPPED with autostart=false while shared ownership migration 20261009000000 remains pending. **`start all` explicitly starts relay despite autostart=false; do not use it on shared state before authorized rollout.** Isolated worker_smoke tests verify relay and collective controls. Existing private Unix socket 0600, developer user, process groups, unexpected-exit restart policy, logs, 30s child shutdown and 65s Compose grace remain intact.

Management stays at [http://localhost:15672/](http://localhost:15672/) on loopback, using existing local credentials. `python3 scripts/check-rabbitmq.py` verifies HTTP, authenticated vhost/topology listings and scoped permissions without printing secrets. Existing matching publication demonstrates port ownership; no substitute port is selected. Exchanges/queues/bindings may appear only after publisher connection; no shared relay startup or fabricated messages are needed. Initialization variables do not update existing-volume credentials. Browser login was not tested because no authorized browser capability is available.

## Validation and continuation

CI retains all isolated PostgreSQL, RabbitMQ and integrated-recovery jobs, with Cargo working directory backend. Added binary build, strict rustdoc and a frontend npm-ci/type-check/build job. Remote CI is configured, not executed by this local preparation. Run backend fmt, strict all-target/all-feature Clippy, standard tests, both binaries and `RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps`; opt-in suites retain isolated databases and UUID broker resources. Documentation validation remains in Engineering Library tools.

Delivery semantics remain the accepted at-least-once foundation under explicit assumptions; broker acceptance is not consumer effect, ownership does not fence external publication, and duplicates still require consumer idempotency. Shared ownership rollout remains pending. Read the [Milestone 2 review](../operations/milestone-2-review.md), then the [frontend component guide](../../../frontend/README.md). Future API integration requires separately agreed contracts; retry/backoff, quarantine and replay belong to separately scoped Milestone 3.

## Actual local outcome and operational limitation

All backend baseline checks, isolated PostgreSQL suites, publisher/connection tests, all three worker_smoke tests (including isolated collective Supervisor controls) and 11 integrated-recovery tests passed. npm ci/type-check/build and development HTTP smoke passed. PostgreSQL and RabbitMQ container IDs remained unchanged. Shared ownership migration remains absent and relay remains stopped. Documentation validation passed with zero errors; 18 warnings concern restricted image fixtures in another project.

Current root .env credentials differ from existing running RabbitMQ initialization credentials. Default management validation returned 401 and initial publisher tests failed authentication. Existing broker credentials authenticated successfully; `python3 scripts/check-rabbitmq.py --running-credentials` verifies them privately without modifying .env or broker users. Broker suites passed with those existing credentials injected only in memory. The recreated app still has root .env values: align configuration through a separate authorized operation before shared rollout. Browser login was not tested.


## Milestone 3.1 policy

Milestone 3.1 defines the failure matrix, durable retry/budget/isolation/replay contracts and remaining roadmap. Only pure policy calculations are implemented; future persistence and frontend scenarios remain planned. [Policy](../architecture/milestone-3-1.md) | [ADR 008](../adr/008-durable-failure-policy.md).
