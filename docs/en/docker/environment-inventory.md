**Current local state — Milestone 4.5 (2026-10-16):** [Verified rollout and recovery](../operations/milestone-4-5-rollout.md). Six migrations applied; current HTTP/relay/consumer artifacts installed and running; consumer autostart=true. Protected backup/isolated restore and real 10 → 15 → 15 demonstration verified. Replay API remains disabled; frontend remains simulation. Earlier observations below retain their dates.

[Other language](../../pt-BR/docker/environment-inventory.md) | [Catalog](../index.md)

# Environment inventory — Milestone 4.4

Audited 2026-10-16 on `cabfbcaa3f31f2be08657c400ef98275acc78df4`. Root ignored `.env` was inspected without printing values. No backend/frontend local `.env` exists. Root entries match the example inventory; no stale extra keys, missing required credentials, empty required values or `change_me` placeholders were observed. Empty `REPLAY_OPERATORS` and `RELAY_MAINTENANCE_CANDIDATES` intentionally disable optional functionality. No secret was created or changed.

Every table entry is optional with its source default unless marked required. PostgreSQL identity/password/database are required by relay, consumer and enabled replay, not by liveness-only HTTP. Broker credentials are required by relay/consumer and Compose broker startup, not by the presentation. Consumer configuration is conditional on explicitly starting that program. All values are server-side/private deployment configuration; **none is a browser API setting**. `VITE_*` is public, but no VITE variable is consumed, no proxy/API base URL is implemented and no operator token belongs there.

## Precedence and observed injection

Compose first resolves shell overrides, then `.env` interpolation and `${VAR:-default}` / `${VAR:?error}`. A root `.env` is not automatically container injection. The service's explicit `environment` mapping selects injected keys. Shell overrides were absent for the audited keys. All configured app/PostgreSQL/broker injected values match the currently resolved Compose settings, **except consumer keys and REPLAY_OPERATORS are absent from the older app container**. Existing app relay settings and credentials match. Frontend receives only `npm_config_cache`; it uses Node's environment for tooling, not backend secrets.

`APP_ENV`, `HTTP_ADDR`, `RUST_LOG` are currently not in Compose's app environment mapping. Each Rust binary calls `Config::load`: dotenvy searches its working directory/parents (`/app/backend` → `/app/.env`) and fills only unset process variables, then defaults apply. This is a real mounted-file fallback, not Compose injection. The same fallback can fill missing consumer/replay keys in a newly launched binary inside the old container; it cannot update an already running process or install its missing Supervisor program. A host shell export of these three keys alone is not forwarded to Compose app. Native process variables override dotenv. Malformed dotenv parsing fails startup with redacted errors.

`RABBITMQ_HOST/PORT` in root `.env` are native/test defaults; Compose deliberately pins app traffic to `rabbitmq:5672`, regardless of their root values. Container clients use `postgres:5432`, host tools `127.0.0.1:5433`. POSTGRES_HOST/PORT supplied to the database container affect its diagnostic client, not the server listener. Image initialization variables do not update persisted roles/passwords/vhosts. Recreate an app container to install changed injected environment; restarting a process alone does not do that.

Initial `/proc/11/environ` and `/proc/12/environ` inspection confirmed all 26 database/broker/relay container keys reached both current child processes, with zero differences. `/proc` shows the initial exec environment, not later dotenv setenv changes.

## Names, types, defaults and consumers

The defaults below are versioned source/example defaults, never observed credential values. Local nonempty entries are present and syntactically compatible with the audited setup; successful authenticated protocol tests establish current credentials work. All consumer entries are present in local `.env` but absent in the old container's injected environment. All other explicitly injected keys match; the three application keys use dotenv fallback. OS/host-port/build keys are Compose inputs, not application process variables.
| Name | Process / purpose | Requirement / type / range | Source default |
| --- | --- | --- | --- |
| `APP_ENV` | HTTP, relay, consumer | nonempty string | `development` |
| `RUST_LOG` | HTTP, relay, consumer | tracing EnvFilter | `info` |
| `HTTP_ADDR` | HTTP; parsed by all binaries | IP:port socket | `0.0.0.0:8080` |
| `HOST_HTTP_PORT` | Compose port publication | TCP port 1..65535 | `8080` |
| `LOCAL_UID` | Docker build / frontend user | positive OS UID | `1000` |
| `LOCAL_GID` | Docker build / frontend user | positive OS GID | `1000` |
| `POSTGRES_DB` | same database clients / PostgreSQL init | nonempty existing database; required | `required; Compose reliable_event_relay` |
| `POSTGRES_USER` | relay, consumer, enabled replay HTTP, SQLx, PostgreSQL init/tests | nonempty role; required; private | `required; Compose example placeholder` |
| `POSTGRES_PASSWORD` | same database clients / PostgreSQL init | nonempty password; required; SECRET | `required; Compose example placeholder` |
| `POSTGRES_HOST` | database clients / health probe | hostname; SQLx wrapper requires postgres | `postgres` |
| `POSTGRES_PORT` | database clients / health probe | u16 > 0 (relay/consumer); SQLx requires 5432 | `5432` |
| `POSTGRES_HOST_PORT` | Compose port publication | TCP port 1..65535 | `5433` |
| `RABBITMQ_DEFAULT_USER` | relay, consumer, broker init, management/tests | nonempty user; required; private | `required` |
| `RABBITMQ_DEFAULT_PASS` | same broker clients / broker init | nonempty password; required; SECRET | `required` |
| `RABBITMQ_DEFAULT_VHOST` | broker clients / broker init | vhost encoded as one URI path segment | `relay` |
| `RABBITMQ_MANAGEMENT_PORT` | Compose port publication | TCP port 1..65535; checker expects 15672 | `15672` |
| `RABBITMQ_HOST` | relay / consumer / tests | valid URI hostname; Compose pins rabbitmq | `rabbitmq` |
| `RABBITMQ_PORT` | relay / consumer / tests | u16 1..65535; Compose pins 5672 | `5672` |
| `RABBITMQ_CONNECT_TIMEOUT_MS` | relay / consumer | integer milliseconds 1..300000 | `10000` |
| `RABBITMQ_PUBLISH_TIMEOUT_MS` | relay / consumer | integer milliseconds 1..300000 | `10000` |
| `RABBITMQ_EXCHANGE` | relay publisher / tests | relay.* short string <=255 bytes | `relay.events` |
| `RABBITMQ_QUEUE` | relay publisher / tests | relay.* short string <=255 bytes | `relay.local` |
| `RABBITMQ_ROUTING_KEY` | relay publisher / tests | nonblank short string <=255 bytes | `event` |
| `RELAY_LEASE_MS` | relay process | integer milliseconds 1..3600000 | `60000` |
| `RELAY_IDLE_MS` | relay process | integer milliseconds 1..60000 | `1000` |
| `RELAY_FAILURE_MS` | relay process | integer milliseconds 1..60000 | `1000` |
| `RELAY_RECONNECT_MS` | relay process | integer milliseconds 1..60000 | `5000` |
| `RELAY_POOL_MAX` | relay process | integer count 1..16 | `2` |
| `RELAY_POOL_ACQUIRE_MS` | relay process | integer milliseconds 1..60000 | `5000` |
| `RELAY_STATEMENT_MS` | relay process | integer milliseconds 1..60000 | `5000` |
| `RELAY_LOCK_MS` | relay process | integer milliseconds 1..60000 | `3000` |
| `RELAY_DRAIN_MS` | relay process | integer milliseconds 1..20000 | `15000` |
| `RELAY_SHUTDOWN_MS` | relay process | integer milliseconds 1..25000 | `25000` |
| `HOST_FRONTEND_PORT` | Compose port publication | TCP port 1..65535 | `5173` |
| `RELAY_MAINTENANCE_CANDIDATES` | relay worker | optional; <=4096 bytes; <=32 unique UUID:cycle:revision entries | `empty; disabled` |
| `REPLAY_OPERATORS` | HTTP only | optional SECRET JSON array, see below | `absent/empty disables routes` |
| `CONSUMER_NAME` | consumer | 1..128 ASCII letters/digits/._-; stable across replicas | `inventory.stock.v1` |
| `CONSUMER_PREFETCH` | consumer | integer count 1..32; one operation active | `1` |
| `CONSUMER_SOURCE_EXCHANGE` | consumer | <=255 bytes; exchange/queue relay.*; routing key nonblank | `relay.events` |
| `CONSUMER_SOURCE_QUEUE` | consumer | <=255 bytes; exchange/queue relay.*; routing key nonblank | `relay.local` |
| `CONSUMER_SOURCE_ROUTING_KEY` | consumer | <=255 bytes; exchange/queue relay.*; routing key nonblank | `event` |
| `CONSUMER_QUARANTINE_EXCHANGE` | consumer | <=255 bytes; exchange/queue relay.*; routing key nonblank | `relay.quarantine` |
| `CONSUMER_QUARANTINE_QUEUE` | consumer | <=255 bytes; exchange/queue relay.*; routing key nonblank | `relay.quarantine.inventory` |
| `CONSUMER_QUARANTINE_ROUTING_KEY` | consumer | <=255 bytes; exchange/queue relay.*; routing key nonblank | `inventory` |
| `CONSUMER_OPERATION_MS` | consumer process | integer milliseconds 1..60000 | `10000` |
| `CONSUMER_RECONNECT_MS` | consumer process | integer milliseconds 1..60000 | `5000` |
| `CONSUMER_DRAIN_MS` | consumer process | integer milliseconds 1..20000 | `15000` |
| `CONSUMER_SHUTDOWN_MS` | consumer process | integer milliseconds 1..25000 | `25000` |
| `CONSUMER_POOL_MAX` | consumer process | integer count 1..16 | `2` |
| `CONSUMER_POOL_ACQUIRE_MS` | consumer process | integer milliseconds 1..60000 | `5000` |
| `CONSUMER_STATEMENT_MS` | consumer process | integer milliseconds 1..60000 | `5000` |
| `CONSUMER_LOCK_MS` | consumer process | integer milliseconds 1..60000 | `3000` |

## Additional tool and test variables

- `CARGO_HOME=/app/.cargo-cache`, `CARGO_TARGET_DIR=/app/target`: Compose/Docker/entrypoint cache and binary paths. Optional tooling overrides; launchers still execute the fixed `/app/target/debug/{reliable-event-relay,relay,consumer}` paths. A separate target does not update those launchers.
- `DATABASE_URL`: SQLx wrapper overwrites it from required POSTGRES fields with URI escaping and insists on postgres:5432. Do not print SQLx help or connection URLs. Application binaries do not consume DATABASE_URL.
- `npm_config_cache=/tmp/npm-cache`: frontend container tooling. `PLAYWRIGHT_BROWSERS_PATH`: optional Playwright tooling override, not application configuration; installation and test users must share an accessible browser cache. Here Chromium was installed/run as root in the frontend container; no backend privilege changed.
- `PGPASSWORD`, `PGCONNECT_TIMEOUT`, `PGOPTIONS`: script-local libpq diagnostics, passed by environment; bounded connection/statement/lock checks. Never put passwords on command lines.
- `RECOVERY_CHILD`, `RECOVERY_READY`, `CONSUMER_BOUNDARY_FILE`: test-only child modes/readiness paths. Missing modes make internal child test entries no-ops, excluded from substantive counts. Never configure shared services with these.
- `HANDOFF_FONT_DIR`: optional historical PDF generation font directory, not runtime. No PDF/handoff was generated in 4.4.
- CI supplies disposable POSTGRES/RABBITMQ credentials, network hosts and admin database, independently of ignored local `.env`; docs job uses Python and integration job installs Supervisor. CI configuration was reviewed/updated, not remotely executed.

## Conditional requirements and actions

`REPLAY_OPERATORS`: absent/empty disables replay. If deliberately enabled, provide a trusted JSON array of 1..16 objects with token (64 lowercase hex characters, generated by OS CSPRNG), actor (unique 1..64 ASCII letters/digits/._-), allow_replay (boolean), maximum_budget (1..100). Actor/token must be unique; total <=8192 bytes. Install migration 20261013000000 and privileges first. Put secrets only in protected server configuration; do not paste them in chat. An enabled malformed value fails rather than silently disabling authorization.

All numeric runtime values reject empty/noninteger/zero/out-of-range input, except HTTP's separate PostgreSQL port parser accepts u16 syntax and relies on connection failure for port zero. Consumer/relay explicitly reject port zero. Both drain budgets must be strictly below their shutdown budget. Changing logical consumer name changes deduplication scope. Source and quarantine exchange/queue names must differ. Bounds and exact parsing are linked to [configuration](../../../backend/src/config.rs), [broker config](../../../backend/src/infrastructure/rabbitmq.rs), [consumer config](../../../backend/src/infrastructure/rabbitmq/consumer.rs), [HTTP composition](../../../backend/src/main.rs) and [SQLx wrapper](../../../docker/sqlx.py).

No missing secret blocks isolated verification. Shared consumer rollout is blocked by unapplied migration 20261014000000 and missing installed Supervisor launcher/config; shared replay is deliberately disabled and migration 20261013000000 is pending. Keep both disabled until the [reviewed rollout](../operations/milestone-4-4-review.md) is executed. Never change persisted database/broker passwords merely to match an example.

## Milestone 5.1 additions

Separate process-local exporters; all optional, with validated source defaults. Compose explicitly injects them; no host port is published. [Contract](../operations/milestone-5-1.md).

| Variable | Default | Validation |
| --- | --- | --- |
| HTTP_METRICS_ADDR | 127.0.0.1:9100 | loopback IP, port 1..65535 |
| RELAY_METRICS_ADDR | 127.0.0.1:9101 | loopback IP, port 1..65535 |
| CONSUMER_METRICS_ADDR | 127.0.0.1:9102 | loopback IP, port 1..65535 |

All three ports must be distinct and differ from HTTP_ADDR; defaults apply independently of replay. A stopped process has no listener. No additional limit knobs: publisher rejects waiting calls immediately, replay admission is fixed at 2 / 10 seconds / 4096 body bytes, exporter at 4 clients / 2 seconds / 1024 request bytes. These defaults were not installed by restarting shared processes.
