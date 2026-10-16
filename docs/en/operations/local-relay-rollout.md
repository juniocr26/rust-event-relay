**Current local state — Milestone 4.5 (2026-10-16):** [Verified rollout and recovery](milestone-4-5-rollout.md). Six migrations applied; current HTTP/relay/consumer artifacts installed and running; consumer autostart=true. Protected backup/isolated restore and real 10 → 15 → 15 demonstration verified. Replay API remains disabled; frontend remains simulation. Earlier observations below retain their dates.

[Other language](../../pt-BR/operations/local-relay-rollout.md)

# Local relay rollout and automatic startup

Executed 2026-10-13 after Milestone 3.3, from clean Rust `cae62cc654efc0b411408dd84a0b92a57738eb38` and Library `c272476429fe4c9223f274d74293edeabb8c79b5`. The user explicitly authorized applying the existing ownership/retry migrations and activating the local development worker. Earlier records of pending migrations/stopped relay describe the pre-rollout state, not current local operation. Milestone 3.3 policy remains unchanged. This is not production readiness or Milestone 3 closure.

## Installed startup policy

[docker/supervisor/normal.conf](../../../docker/supervisor/normal.conf) starts both `http` and `relay` with autostart=true. Compose still runs RabbitMQ and PostgreSQL as separate services and gates app startup on both service_healthy dependencies. PostgreSQL health uses authenticated SELECT 1; RabbitMQ health checks node operation. The worker then performs authenticated PostgreSQL/schema preflight and broker setup using its existing adapter. Health can change after admission; the worker retains its explicit reconnection policy. No SQL migration runs in the entrypoint or either binary.

Supervisor uses autorestart=unexpected, exitcodes=0, startsecs=1 and startretries=3. Unexpected nonzero exits/signals after RUNNING restart; normal zero exits do not. Startup failures have bounded retries and can reach FATAL: repair configuration/schema/artifacts, then start manually. An intentional supervisorctl stop remains STOPPED until explicitly started, even with autostart=true. Autostart applies when supervisord starts again, so container recreation/restart starts both children anew. [Supervisor contract](https://supervisord.org/configuration.html#program-x-section-settings) and [Compose dependency readiness](https://docs.docker.com/compose/how-tos/startup-order/).

SIGTERM preserves existing worker drain and overall shutdown limits (defaults 15 s / 25 s, validated bounds). Each Supervisor child has 30 s before forced termination; group stop/kill remains enabled. Compose grants 65 s for sequential managed-child shutdown plus margin. A cancelled operation may have durable effects; no compensating cleanup was added.

## Explicit installation and controls

For a fresh local environment, configure ignored .env with current credentials, inspect migration SQL/data, and explicitly prepare artifacts/schema before starting the application:

```bash
docker compose build app
docker compose run --rm --no-deps app cargo build --locked --bins
docker compose up -d --wait postgres rabbitmq
docker compose run --rm --no-deps app sqlx migrate info
# Explicit operator action only, after inspecting SQL and current data:
docker compose run --rm --no-deps app sqlx migrate run
python3 scripts/start-local.py
docker compose exec -T app supervisorctl status
```

For a Supervisor-config update with already ready dependencies/schema, rebuild/recreate only the affected application container:

```bash
docker compose up -d --build --no-deps app
```

Existing bind/named database and broker storage survives this operation. Never reset the database, delete volumes, purge queues or change credentials to install autostart. Existing-volume credentials must remain synchronized explicitly; initialization variables do not change persisted credentials.

Direct controls:

```bash
docker compose exec app supervisorctl status
docker compose exec app supervisorctl stop relay
docker compose exec app supervisorctl start relay
docker compose exec app supervisorctl restart relay
docker compose exec app supervisorctl stop http
docker compose exec app supervisorctl start http
# Identical forwarding alias:
docker compose exec app supervisor status
docker compose exec app supervisor stop relay
docker compose exec app supervisor start relay
# Interactive console: status, stop relay, start relay, restart http, quit
docker compose exec app supervisorctl
docker compose exec app supervisor
```

`all` controls both app children, never PostgreSQL/RabbitMQ or frontend. Stopping relay leaves HTTP running, broker management reachable and broker-owned storage independent. Rebuilding mounted Rust executables requires stopping their corresponding children first, then cargo build --locked --bins and restarting them. There is no source reload.

## Broker topology and evidence

Management remains [http://localhost:15672/](http://localhost:15672/); log in with current ignored .env credentials. Container AMQP uses rabbitmq:5672. The existing RabbitMqPublisher declares the configured durable direct exchange, durable queue and binding with configured routing key after authenticated connection. Defaults are vhost relay, exchange relay.events, queue relay.local and routing key event; do not assume defaults when environment overrides exist. Topology setup does not publish messages. An empty management panel is valid when no producer work exists. No demonstration messages were sent to populate it.

[check-rabbitmq.py --require-topology](../../../scripts/check-rabbitmq.py) verifies the host login page, authenticated identity/scoped permissions, correct configured vhost and exact durable topology without printing credentials, API bodies or message contents. Without the flag it remains useful before topology exists. [check-supervisor.py](../../../scripts/check-supervisor.py) is an explicit operational test: it stops/starts/restarts both children through direct/interactive commands, waits after intentional stops, checks HTTP/broker queue metadata and counts, and injects SIGKILL to prove unexpected-exit recovery. It performs no message publication/consumption, purge or migration. Run it only when these local process controls are intended.

Actual rollout inspected zero outbox rows (including zero initial-budget exhausted rows) and two installed initial migrations. Existing additive 20261009000000 ownership and 20261012000000 durable retry SQL was reviewed, then sqlx migrate run explicitly applied both. All four migrations now report installed, original outbox remains empty, and constraints/schema preflight succeed. Baseline-zero backfill grants no renewed historical allowance; terminal legacy metadata handling remains the existing migration contract.

Only app was rebuilt/recreated. Broker/database/frontend remained running with their storage and credentials unchanged. Both children started automatically. Authenticated host management verified configured topology; direct/interactive controls and intentional-stop checks passed; HTTP remained healthy when relay stopped; unexpected exits restarted both children. Observed shared queue counts remained zero. This proves queue/topology/control availability, not preservation of a nonempty message payload: no shared message was available, and none was fabricated. Browser form login was not performed; host page/API authentication was verified. [Executed validation](../testing/validation-results.md#local-relay-autostart-rollout--2026-10-13).

General poison/exhausted discovery remains 3.4, authenticated audited replay 3.5 and full acceptance/closure 3.6. Frontend simulations remain unchanged and do not represent live backend/dashboard integration.

## Consumer opt-in — 2026-10-16

The source configuration now has three programs: HTTP/relay keep autostart=true; consumer has autostart=false. start all/restart all explicitly starts consumer too. Use named commands; the new program is not installed in the current running manager. Compose grace is 95s for three bounded children. [Consumer configuration, topology, schema prerequisites and authorized rollout procedure](../architecture/milestone-4-3.md#supervisor-and-pending-shared-rollout). Historical rollout results above retain their dates.
