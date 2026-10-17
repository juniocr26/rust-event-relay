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

```sh
docker compose exec -T app cargo fmt --check
docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings
docker compose exec -T app cargo test --locked
docker compose exec -T app cargo build --locked --bins
docker compose exec -T app env RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps
docker compose exec -T frontend npm ci
docker compose exec -T frontend npm run type-check
docker compose exec -T frontend npm run build
python3 -B scripts/measure-isolated.py verification
python3 -B scripts/test-application.py
python3 -B scripts/test-clean-install.py
python3 -B scripts/check-migrations.py
python3 -B scripts/check-docs.py
python3 -B -m unittest discover -s scripts -p 'test_*.py'
git diff --check
```

The opt-in verification harness creates disposable PostgreSQL/RabbitMQ services; browser tests use a temporary copy of the main Compose composition. The clean-install harness copies only Git-visible candidate files and creates private credentials, isolated ports and fresh storage; it builds without host Cargo/node artifacts. These are temporary test installations, not another supported stack. Never run destructive fault tests against retained application services. CI configuration is in `.github/workflows/ci.yaml`; local success does not establish remote CI success.

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
