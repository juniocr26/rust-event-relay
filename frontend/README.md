# Frontend

[English application guide](../docs/en/operations/application-scenarios.md) | [Guia em português](../docs/pt-BR/operations/application-scenarios.md)

Normal startup from the repository root: `python3 scripts/start-local.py`. This starts the main Compose application, including Vite on localhost:5173 or 127.0.0.1:5173 and its `/demo` proxy to app:8080. Standalone `npm run dev` does not start the backend or its dependencies.

The Vue interface supports four real scenarios: delivery, duplicate, concurrent duplicate and controlled before-send rejection with durable retry. Crash and ACK-loss playback remain illustrative. Enter the active secret in **Local demo token**, choose **Verify access** (a non-mutating configuration GET), then run or inspect retained evidence. The token stays in page memory; sessionStorage retains only request/action identities. [Token rotation, persistence and connectivity](../docs/en/operations/demo-token-connectivity.md) | [Token em português](../docs/pt-BR/operations/demo-token-connectivity.md).

Shared Vue components preserve the blue design, modal keyboard controls and responsive layouts. Recovery details expand separately; evidence reflects database observations rather than animation timing. Unknown mutation outcomes require inspection using the same identity.

Run `docker compose exec -T frontend npm run type-check` and `docker compose exec -T frontend npm run build`. `python3 scripts/test-application.py` validates real scenarios in disposable main-Compose resources; `python3 scripts/test-clean-install.py` exercises the documented startup without inherited artifacts. Historical presentation-only results are retained in the [Milestone 3 review](../docs/en/operations/milestone-3-review.md).

Never put secrets in VITE_* variables or the frontend bundle. Broker confirms, consumer commit and ACK receipt remain distinct observations.
