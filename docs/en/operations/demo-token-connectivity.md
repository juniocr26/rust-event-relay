# Startup and token verification


The supported host startup is `python3 scripts/start-local.py`. It builds the runtime artifacts with `--locked` in `target/runtime`, verifies/backs up existing storage before any pending forward migrations, and starts Supervisor only after prerequisites. On manager startup, the entrypoint prepares runtime binaries automatically; direct `docker compose up` does not migrate retained storage. Use the supported startup for fresh installations. Missing schemas are an application startup failure, not necessarily a launcher execution failure. Check `docker compose logs --tail 60 app` and `docker compose exec app supervisorctl status`; do not repeatedly restart or reset volumes. Named operator STOPPED states remain respected under the same manager. Tests build in `target/tests` and never replace service binaries.

Inside app: `python3 /app/scripts/demo-token.py rotate`. Rotation saves the authoritative token first, restarts only running HTTP to reload it, starts FATAL/EXITED programs once, and preserves intentional STOPPED programs. It verifies new-token access and rejection of the previous token, named process stability, readiness of running processes and durable RabbitMQ topology through read-only management queries. These checks do not publish business events or purge queues. HTTP intentionally STOPPED leaves activation pending and returns nonzero. A failure after activation explicitly reports that rotation was already applied; retrieve the token privately from `.env` and fix the stated service fault before retrying. Supervisor process status and queue topology are separate checks.

---

[README](../../../README.md) | [Catalog](../index.md) | [Português](../../pt-BR/operations/demo-token-connectivity.md)

# Local demo token and connectivity

Normal startup from the checkout root: `python3 scripts/start-local.py`. Reuse an existing private `.env`; never overwrite it on a fresh clone. Startup builds all Rust binaries before migrations and HTTP/relay/consumer startup. It resumes processes left FATAL/BACKOFF/EXITED by an earlier missing-binary startup; intentional STOPPED processes remain stopped. Use named Supervisor controls for intentional stops.

The single authoritative demo secret is `DEMO_TOKEN` in the checkout's ignored `.env`, bind-mounted at `/app/.env`. The HTTP launcher reads this file on every launch, overriding any stale inherited token. Main Compose does not inject another copy. This is an opaque 64-character hexadecimal secret (256 secure random bits), not a JWT; it has no automatic expiry. A clone alone does not require rotation if the private configuration remains valid. Never put it in VITE_* variables, images, URLs or persistent browser storage.

Inside the running app container, run exactly:

```sh
python3 /app/scripts/demo-token.py rotate
```

From the host, the equivalent is `docker compose exec app python3 /app/scripts/demo-token.py rotate`. This explicitly invoked local command saves a cryptographically secure replacement in the same host `.env`, preserving other settings and file permissions, and restarts only Supervisor's named `http` process. It checks new-token HTTP 200 and previous-token HTTP 401 before displaying the secret with a privacy warning. It does not rotate PostgreSQL, RabbitMQ or replay credentials. Keep the displayed value private. Enter this same value in **Local demo token**, choose **Verify access**, then run the selected scenario.

The replacement survives stop/start and container recreation because the host file is authoritative. Open tabs retain their previous token only in memory and must enter the replacement; an in-flight HTTP connection may close during restart. Keep existing request/action identities and inspect them before submitting work again. Relay/consumer keep running; no volumes, queues, receipts or application records are reset. If restart/verification fails, the new value is already saved: inspect `supervisorctl status`, correct the startup failure and run `supervisorctl start http`; retrieve the saved value privately from `.env`. Do not blindly rerun mutations.

Supported browser addresses: `http://localhost:5173` and `http://127.0.0.1:5173` (or the configured HOST_FRONTEND_PORT). Both exact origins are accepted; arbitrary origins are forbidden. Browser → Vite `/demo` proxy → `http://app:8080` → authenticated route; HTTP first requires PostgreSQL connectivity and the demo migration. Real execution additionally needs relay, consumer and RabbitMQ.

A 401 means missing/invalid token; 403 means forbidden Origin/client header. A 502/proxy ECONNREFUSED means the HTTP listener is unreachable, not that a token expired. Inspect `docker compose exec -T app supervisorctl status`. The 2026-10-17 failure had all three children FATAL with missing binaries and proxy ECONNREFUSED. Use normal startup to build and resume them. A failed configuration GET submitted no business work; a failed POST can have an unknown outcome, requiring inspection of its retained identity.

The final audit PDF was a temporary communication artifact, not an installation prerequisite. Maintained acceptance context lives in [final audit](final-audit.md), [acceptance matrix](milestone-6-3.md) and [application guide](application-scenarios.md). Dated acceptance evidence does not supersede this subsequently reproduced startup defect.

## Verification on 2026-10-17

The previously unavailable main frontend proxy accepted both documented Origins after binary build, migration verification and named process recovery. A local Chromium test container forwarding to the main Vite service passed 11 focused browser checks: all four real scenarios, five viewport widths, both loopback addresses, authentication/Origin rejection and a simulated configuration 502 with no POST or new request identity. Direct PostgreSQL checks independently confirmed stock 10 → 15, one receipt, correlated Duplicate observations and bounded durable recovery. The old token returned 401 and the replacement 200; the replacement still authorized after targeted app recreation, with unchanged run/action/receipt/stock snapshot. Python command regressions passed 11 cases, Rust library tests passed 25 (2 unrelated ignored), and all 12 isolated demo integration cases passed sequentially. Clippy and frontend production build passed. No configured secret was found in production assets.

Initial broad validation encountered linker signal 9 during parallel builds and database-creation contention in parallel isolated tests; bounded/sequential reruns avoid those resource conflicts. This is local evidence, not remote CI or production acceptance. No audit PDF is required.

The disposable main-Compose campaign passed 29 browser cases with independent SQL, historical duplicate inspection, stale/unknown evidence, paused-consumer recovery and backend-unavailable states. The clean candidate campaign passed six focused browser cases through the documented startup, with no inherited build artifacts: nine migrations, four runs, two duplicate actions and four receipts; stop/start and app recreation retained the same snapshot and active token. Exact test-owned services/volumes were removed. Markdown validation checked 129 files with zero missing local links; English/Portuguese Markdown counterpart sets match. Heading anchors and remote links were not checked.
