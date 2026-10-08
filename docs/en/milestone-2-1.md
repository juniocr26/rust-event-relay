[Português brasileiro](../pt-BR/milestone-2-1.md) | [README](../../README.md)

# Milestone 2.1 — RabbitMQ publisher and process management

Implemented on 2026-10-08. This is the publisher slice of Milestone 2, not a completed at-least-once relay. The Milestone 1 handoff remains a historical snapshot. [ADR 006](adr/006-rabbitmq-publisher-and-supervisor.md) records the decisions.

## Local setup and browser login

Edit the ignored root `.env`: replace `RABBITMQ_DEFAULT_USER` and `RABBITMQ_DEFAULT_PASS` placeholders with your development credentials. Use the **same values in the browser login form at http://localhost:15672/**. This checkout has local credentials configured; they are not documented or committed. `.env.example` contains only placeholders. Existing shell variables override Compose interpolation; avoid unintended overrides. Do not share raw `docker compose config`, environment dumps or source chains containing secrets.

```bash
# New checkout only:
cp .env.example .env
# Edit credentials, and on Linux LOCAL_UID / LOCAL_GID, before initialization.
docker compose build app
# Build the real binary before supervisord starts its HTTP child:
docker compose run --rm --no-deps app cargo build --locked
python3 scripts/start-local.py
python3 scripts/check-rabbitmq.py
curl --fail http://localhost:8080/health
```

`start-local.py` detects a management-port conflict and reports the requested port without changing it. Docker also reports races/conflicts during binding. Default publication is exactly `127.0.0.1:${RABBITMQ_MANAGEMENT_PORT:-15672}:15672`. If you explicitly override that variable, the URL changes. Free a conflicting default port to retain `http://localhost:15672/`.

Compose pins `rabbitmq:4.3.6-management`, which includes the enabled management plugin, based on [official installation/release information](https://www.rabbitmq.com/docs/download) and [official image tags](https://hub.docker.com/_/rabbitmq/). The app uses **rabbitmq:5672** through Docker DNS. AMQP is not published to the host: the real integration workflow runs in app; only the management HTTP port needs host access. Broker data persists in the `rabbitmq-data` named volume, with stable hostname `rabbitmq`. PostgreSQL configuration and `.dockerized-postgres/` are preserved. Normal down/up and rebuild keep data; do not use `down -v` to change credentials.

The broker readiness check is `rabbitmq-diagnostics -q check_running`; Compose gates app startup on both broker and PostgreSQL health, without fixed sleeps. This does not prove authenticated readiness or delivery. The dedicated diagnostics and integration tests verify authentication separately; HTTP `/health` remains liveness only.

`docker/rabbitmq/rabbitmq.conf` initializes the user with the **management** tag and configure/write/read regex `^relay[.].*` in `RABBITMQ_DEFAULT_VHOST` (default `relay`). It grants project topology operations, including unique `relay.it.*` integration resources, without access to other vhosts. Management is sufficient for browser access and `/api/whoami`/accessible vhosts; listing permission records requires an administrator. The diagnostic verifies authenticated identity and visible vhost through HTTP, then verifies scoped permissions with the local node CLI. It never promotes the user to administrator.

**Initialization variables do not update an existing broker volume.** Changing `.env` or rebuilding/recreating the container does not change stored users/passwords/tags/permissions/vhosts. If authentication fails, inspect the persisted user and vhost first. Use an authorized existing administrator and RabbitMQ user/password/tag/permission administration (or `rabbitmqctl help change_password`) to change the existing account, then synchronize `.env` and recreate app environment. Use interactive password entry or a protected administrative workflow; avoid passwords in shell history/process arguments. Never reset/delete broker data to apply credentials. Existing volumes are not automatically reconciled by this configuration.

RabbitMQ stores password hashes; this does **not** encrypt network traffic. Local HTTP and AMQP are plaintext. TLS, external exposure, production access control, backups and high availability require later operational design. A single durable local broker is not an HA guarantee. See [RabbitMQ access control](https://www.rabbitmq.com/docs/access-control).

## Architecture and publication semantics

`application::publisher::EventPublisher` accepts a borrowed validated `EventEnvelope` and returns a native Send future with static dispatch. `infrastructure::rabbitmq::RabbitMqPublisher` implements it with Lapin 4.12.0 using its Tokio feature, no TLS backend for this explicit plaintext local scope, and no automatic recovery/retry. See [official Lapin documentation](https://docs.rs/lapin/4.12.0/lapin/). Protocol/client types stay in infrastructure. `main` still starts HTTP only; there is no worker or publication endpoint. The read-only `OutboxReader` remains unchanged and does not publish or mutate state. A future use case must own orchestration and ownership/transition decisions.

The adapter owns one connection and one confirm-enabled channel. `connect` declares a durable **direct** exchange `relay.events`, durable queue `relay.local`, and binding key `event`; topology is created when the adapter connects, not by the HTTP bootstrap. Environment overrides `RABBITMQ_EXCHANGE`, `RABBITMQ_QUEUE`, `RABBITMQ_ROUTING_KEY` stay outside the domain. Exchange and queue must start with `relay.`; AMQP names are bounded to 255 bytes. The local queue retains messages until explicitly consumed; no production consumer exists.

Publishing serializes the unchanged envelope with serde_json arbitrary precision, preserving original ID, occurrence time, payload and optional metadata. AMQP properties are `content_type=application/json`, `delivery_mode=2` (persistent) and stable `message_id=event.id`. Mandatory publishing waits for the broker confirmation, not only socket writing.

| Result | Meaning |
| --- | --- |
| `Ok(())` | Confirmed ack with no return under configured topology |
| `Unroutable` | Ack accompanied by mandatory return: no successful routed publication |
| `Rejected` | Broker nack |
| `Unavailable` | Invalid configuration, connection/setup/admission failure, or closed/retired transport before sending |
| `Serialization` | JSON serialization failed before sending |
| `Uncertain` | Send/confirmation operation failed or timed out after entering the sending boundary, or confirms unexpectedly absent |

Public Display/Debug report only classification. Typed URL/parse, serde, Lapin and Tokio deadline sources remain available through Error::source; source chains may contain secrets and require redaction. A validated current envelope has no deliberately failing serializer; serialization classification is defensive, not an injected real-broker failure.

`RABBITMQ_CONNECT_TIMEOUT_MS` and `RABBITMQ_PUBLISH_TIMEOUT_MS` default to 10000; valid durations are 1–300000 ms. The connection and topology setup each have a connection deadline; cleanup on setup failure has another bounded deadline. Publication admission and send-plus-confirm each have a publication deadline. Serialization runs synchronously before those network waits and is not a byte/memory bound. Large payload limits remain future work.

The adapter serializes concurrent publications with a mutex, limiting in-flight sends to one without spawning a task per message. Caller task counts/admission still need future application bounds. The channel is marked retired before entering sending, and restored only after a definitive ack/return/nack. Timeout, transport error or cancellation after that boundary leave it retired: subsequent publication fails before sending, and the caller must explicitly close/drop and reconnect. Cancellation returns no error to the caller and does not prove absence; reconnecting does not reconcile the prior event. There is **no automatic retry**, especially for uncertain acceptance. Call `close(self)` after awaiting in-flight calls for a bounded graceful connection close; dropping is resource cleanup, not acknowledgement or guaranteed graceful draining.

Publisher confirmation proves broker acceptance under this topology, not consumer processing or exactly-once delivery. This follows [RabbitMQ publisher-confirm semantics](https://www.rabbitmq.com/docs/confirms). Retry after an unknown result can duplicate; persist delivery state and consumer idempotency must be designed separately.

## Supervisor and build workflow

The physical versioned config is `docker/supervisor/supervisord.conf`, installed as `/etc/supervisor/supervisord.conf`. Rebuild/recreate app after changing it or the executable aliases. `init: true` remains; the non-root developer entrypoint execs foreground supervisord. Cache/source mounts and Cargo tooling remain writable as before. The socket directory is developer-owned mode 0700; its Unix control socket is mode 0600. `supervisorctl` discovers the installed standard config with no manual `-c`.

The actual and only program name is **http**. RabbitMQ and PostgreSQL are separate Compose services. The publisher is a callable adapter, not an idle daemon. `all` means all programs in this app's Supervisor (currently `http`).

```bash
docker compose exec app bash
# Direct, inside app:
supervisorctl status
supervisorctl stop all
supervisorctl start all
supervisorctl stop http
supervisorctl start http
supervisorctl restart http
# Alias forwards every argument:
supervisor status
supervisor stop all
supervisor start all
supervisor stop http
supervisor start http
supervisor restart http
# Interactive: launch either command with no arguments:
supervisorctl
# Or: supervisor
# At the supervisor> prompt:
status
stop all
start all
stop http
start http
restart http
quit
```

`supervisor` is a small executable forwarding to supervisorctl. Intentional `stop` leaves supervisord available for `start`. The child executes `/app/target/debug/reliable-event-relay`, through an `exec` launcher, without `cargo run`. Missing binary produces an explicit build instruction and bounded startup retries; app remains usable to build and then `start http`. A successful container start is not proof that the child started: inspect `supervisorctl status` and health.

For source changes, stop before rebuilding the executable, then start:

```bash
docker compose exec app supervisorctl stop http
docker compose exec app cargo build --locked
docker compose exec app supervisorctl start http
# A restart is also available after an already completed build:
docker compose exec app supervisor restart http
```

Unexpected nonzero exit/signal restarts the child; normal exit code 0 is expected. Explicit stops never autorestart. SIGTERM asks HTTP to drain; group stop/kill prevents orphan descendants. Supervisor allows 30 seconds before SIGKILL, a finite local development budget for a currently request-light health server; Compose allows 35 seconds for the manager to finish. Long-lived requests may exceed that budget and be interrupted. Child stdout/stderr go to container streams with rotation disabled. Use `docker compose logs app` to inspect shutdown/restart. Direct binary lifecycle tests remain intact and independent of Supervisor.

## Test commands and evidence

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

Default tests need neither PostgreSQL nor RabbitMQ. Broker integration tests use explicit credentials and unique UUID topology in the project vhost; cleanup deletes only the exact owned queues/exchanges after success/panic/deadline. A test-owned TCP proxy withholds confirm responses to exercise timeout and post-send cancellation without restarting the broker or disturbing other clients. The received message proves that those uncertain outcomes may already have been accepted; no second message/retry is produced. Abrupt test process death or failed cleanup can leave scoped test resources; inspect exact names before cleanup. CI adds a dedicated RabbitMQ integration job with disposable credentials, following the PostgreSQL job pattern.

Actual execution and remaining validation limits are recorded in [validation results](validation-results.md). The management page and authenticated HTTP API were tested on the user's host localhost. **Browser form login was not directly tested: no browser-control tool is available.** Do not equate API checks with an interactive browser login. Remote GitHub CI and production behavior were not executed.

## Deferred Milestone 2 work

Delivery-state transitions and authority, claims/leases/ownership, reconciliation of uncertain outcomes, retries/backoff, dead-letter/quarantine/repair, producer writes, consumer idempotency, polling workers, byte/concurrency bounds and operational readiness remain deferred. A pending read snapshot does not authorize exclusive delivery. Broker acceptance followed by a failed database transition creates a duplicate window; database completion before publication creates a loss window. These designs are prerequisites for an at-least-once delivery foundation beyond this adapter.
