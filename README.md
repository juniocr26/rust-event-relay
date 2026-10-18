[English](README.md) | [Português brasileiro](README.pt-BR.md)

# Reliable Event Relay

A Rust portfolio and study project demonstrating a transactional PostgreSQL outbox, confirmed RabbitMQ publication and atomic consumer deduplication. The local Vue interface runs real delivery, duplicate, concurrent duplicate and one bounded before-send rejection/retry scenario. Crash and ACK-loss animations are explicitly illustrative.

Milestone 6.3 is locally accepted (2026-10-17); Milestone 6 and the agreed portfolio scope are complete. Clean installation, real scenarios, inspection, recovery and local checks passed. Remote CI is unverified; production readiness is not established. Historical records retain their dated scope; this page defines the supported workflow.

Final implementation audit (2026-10-17): [current closure and executed evidence](docs/en/operations/final-audit.md).

## Clean installation

Install Docker Engine/Desktop with Compose v2, Python 3 and Git. Docker must be running, with internet access for image/package downloads and enough disk space for Rust builds. Native Rust/Node are optional; the supported commands below run them in containers. Copy the example, replace the four credential placeholders, then start:

```sh
git clone https://github.com/juniocr26/rust-event-relay.git
cd rust-event-relay
cp .env.example .env
# POSTGRES_USER, POSTGRES_PASSWORD, RABBITMQ_DEFAULT_USER, RABBITMQ_DEFAULT_PASS
# Linux: LOCAL_UID=$(id -u), LOCAL_GID=$(id -g) — edit .env accordingly.
python3 scripts/start-local.py
```

Use unique local credentials in ignored `.env`; do not paste them into logs, commits or VITE_* variables. The startup script generates DEMO_TOKEN once, validates configuration, starts infrastructure, builds binaries, checks migration history, backs up/restores before a necessary forward migration, and starts HTTP/relay/consumer under Supervisor plus the frontend. Existing intentional stops under an unchanged Supervisor manager remain stopped; use named controls to resume them. Do not run historical rollout commands as installation instructions.

| Service | Default address |
| --- | --- |
| Frontend | http://localhost:5173 |
| HTTP liveness | http://localhost:8080/health |
| PostgreSQL | localhost:5433 |
| RabbitMQ management | http://localhost:15672 |

Open the frontend, choose a real scenario, open the modal and enter DEMO_TOKEN from your private `.env`. Run delivery: stock starts at 10 and the consumer commits 15. Duplicate actions preserve 15 and one effect receipt. “Inspect retained runs and actions” reads retained evidence; reload requires token entry again. Closing a modal stops polling, never committed work.

## Documentation and evidence

- [Current application guide: lifecycle, scenarios, SQL, topology, recovery and study path](docs/en/operations/application-scenarios.md)
- [6.3 acceptance matrix and current verification results](docs/en/operations/milestone-6-3.md)
- [Complete documentation catalog](docs/en/index.md)
- [ADR 012](docs/en/adr/012-one-application-real-scenarios.md)

## Tests

Host, from the repository root:

```sh
docker compose exec app tests
```

Inside app, from any directory: `tests`. Automation may use `docker compose exec -T app tests`.

The command prepares locked npm dependencies and Chromium automatically, then runs Rust formatting/lint/tests/binary compilation/documentation, documentation links and Python tooling tests, frontend type checking/build/browser experience, real PostgreSQL/RabbitMQ/process integration tests, disposable SQLx migration upgrades/rollbacks, and real normal/duplicate/concurrent/recovery browser scenarios with test-owned processes and disposable databases/broker namespaces on the existing infrastructure. Live browser checks also cover paused-consumer recovery, unavailable HTTP and unknown-action inspection under an isolated Supervisor manager. It never migrates the main database or stops the managed application.

First use downloads packages/browser and compiles Rust. Cargo test artifacts live in `target/tests`; managed service artifacts live in `target/runtime`. npm/browser caches and the isolated frontend workspace live under `/tmp/relay-tests` and are reused until container recreation. The frontend service’s node_modules is not modified. One invocation at a time is allowed. Each required phase stops on failure; the summary lists only completed phases. Exit codes: 0 success, 1 failure, 130 interruption. Diagnostics show a sanitized output tail and save the last failing phase to `/tmp/relay-tests/last-failure.log` inside app; Ctrl+C signals the owned process group and permits cleanup before forced termination. Cleanup errors are failures, not successful verification.

Separate host acceptance (optional, potentially expensive):

```sh
python3 -B scripts/test-application.py
python3 -B scripts/test-clean-install.py
python3 -B scripts/measure-isolated.py verification
```

These checks orchestrate fresh Docker infrastructure, main Compose outage controls and clean installations; they cannot run inside app without host Docker access. Capacity campaigns remain optional; see the measurement script’s documented modes. Routine `tests` does not claim these acceptance checks ran. No Docker socket or privileged nesting is required.

## Operation and limits

```sh
docker compose exec -T app supervisorctl status
docker compose exec -T app supervisorctl stop http
docker compose exec -T app supervisorctl start http
docker compose stop
# Later / depois:
python3 scripts/start-local.py
```

Source changes require rebuilding binaries and deliberately restarting the relevant process; a binary on disk is not proof that a running process loaded it. PostgreSQL persists under `.dockerized-postgres/`, RabbitMQ in its named volume. Do not reset volumes, purge queues or delete receipts as repair.

This is a bounded local engineering demonstration, not production readiness or exactly-once transport. SQL and AMQP cannot commit atomically. Unknown outcomes require inspection; confirms, consumer completion and broker receipt of ACK are distinct. TLS/HA, unrestricted chaos, automatic retention, generalized repair and larger worker pools are outside the agreed local scope.

MIT — [LICENSE](LICENSE).

[Token, persistent activation and troubleshooting](docs/en/operations/demo-token-connectivity.md).

Inside the app container: `python3 /app/scripts/demo-token.py rotate`.
