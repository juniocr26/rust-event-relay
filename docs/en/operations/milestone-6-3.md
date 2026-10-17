[Português brasileiro](../../pt-BR/operations/milestone-6-3.md) | [Application guide](application-scenarios.md)

# Milestone 6.3 — final acceptance

Status: accepted locally on 2026-10-17. Milestone 6 and the agreed local portfolio scope are closed. All required installation, real scenarios, inspection and verification criteria passed. Remote CI is unverified and production readiness is not established. Historical 6.1/6.2 results remain dated evidence.

Implemented in this candidate: authenticated bounded retained-run pagination including pending runs; read-only recovery distinct from explicit submission; historical action selection and identity persistence; runtime-only token; observation freshness; copyable SQL and topology; reconciled English/Portuguese installation and study guide. No migrations changed. Existing blue components/modal remain.

## Acceptance inventory

| Capability | Source and verification command | Current classification |
| --- | --- | --- |
| Canonical envelope and validation | event_envelope, consumer_contract | Implemented and verified now |
| Producer atomicity/idempotency and history | demo_runs | Implemented and verified now |
| Ownership/leases/revisions/stale owners | delivery_ownership, postgres_ownership | Implemented and verified now |
| Retry budgets/scheduling/terminal preservation | retry_state, retry_policy, postgres_retry | Implemented and verified now |
| Bounded discovery/active owner protection | maintenance, postgres_discovery | Implemented and verified now |
| Authorized replay and audit | replay_application, replay_http, postgres_replay | Implemented and verified now; opt-in, disabled in normal environment |
| Atomic stock/deduplication/content conflicts | postgres_consumer, consumer_application | Implemented and verified now |
| Quarantine before source ACK | consumer_runtime, integrated_recovery | Implemented and verified now |
| Admission/finite workloads/freshness/lifecycle | worker, operational_readiness, lifecycle | Implemented and verified now |
| Successful/duplicate/concurrent/recovery pipeline | scripts/test-application.py | Implemented and verified now |
| Inspection/reload/historical action selection | real-scenarios.spec.ts | Implemented and verified now |
| Authentication/Origin/secrets | demo_runs, real browser tests | Implemented and verified now |
| Docker/Supervisor/persistence/clean installation | scripts/test-clean-install.py, worker_smoke | Implemented and verified now |
| Accessibility/responsive/keyboard/focus/reduced motion | live-delivery.spec.ts, experience.spec.ts | Implemented and verified now |
| Migrations/checksums/rollback/reapply | scripts/check-migrations.py | Implemented and verified now |
| Scripts/CI/docs | check-docs.py, unittest, local CI-equivalent commands | Implemented and verified locally; remote CI implemented but not verified now |
| Crash/race/ACK-loss visual playback | usePlayback.ts | Illustrative; never live evidence |
| TLS/HA, generalized chaos/repair, automated retention, production readiness | Explicit scope boundary | Outside agreed local project scope |

## Controlled failures

Run `python3 -B scripts/measure-isolated.py verification`: generated owner label, separate PostgreSQL/RabbitMQ containers/network, finite startup/test budgets, exact owned cleanup. Existing tests inject transaction acknowledgement loss, crash/cancellation, confirm uncertainty, broker rejection, quarantine/ACK boundaries and ownership races. Read each test's injection point and independent SQL assertions; names alone do not prove execution. `scripts/test-application.py` tests one service stop at a time in its own temporary composition; the interactive recovery intent rejects only its own event before send. Never run `check-supervisor.py` destructive controls on retained main services for acceptance.

The concurrent consumer-session fix closes a session after its assigned ACK so prefetched unsettled messages return to the action queue. Repeated real concurrent actions must confirm with one effect receipt and stock 15; cleanup must leave no unexpected consumers.

## Verification ledger

Final commands and results are recorded below and in the [machine-readable ledger](../testing/milestone-6-3-results.json). No required acceptance blockers remain. Production extensions remain outside the agreed local scope. The clean-install test copies Git-visible candidate files, not ignored caches/secrets/binaries; no staging, commit or push is required.

### Local commands executed on 2026-10-17

| Command | Final result |
| --- | --- |
| `python3 -B scripts/measure-isolated.py verification` | Format and strict all-target/all-feature Clippy passed; standard suite 102 passed / 141 ignored; selected opt-ins 138 passed, then two real-broker library opt-ins passed. Final binary build and warnings-denied rustdoc passed. |
| `python3 -B scripts/test-application.py` | Final campaign passed: 1 paused-consumer + 24 presentation/real-pipeline/history + 1 unknown-action fixture + 1 unavailable-API = 27 browser tests. Independent SQL and seven empty action queues with no temporary consumers verified. |
| `python3 -B scripts/check-migrations.py` | Nine ordered upgrades/checksums, no-op and supported demo/consumer/replay rollback/reapply passed; exact disposable database removed. |
| `docker compose exec -T frontend npm run build` | Vue type-check and production build passed. Locked installation is additionally exercised in the disposable browser/clean harnesses. |
| `python3 -B scripts/check-docs.py` | 125 Markdown files; zero unresolved local file links. Heading anchors and remote links are outside this checker. |
| `python3 -B -m unittest discover -s scripts -p 'test_*.py'` | Four checker tests passed. |
| `git diff --check` | Passed. |
| `python3 -B scripts/measure-isolated.py campaign` | Passed: one test, 22 controlled trials, zero unexplained outcomes; exact owned resources removed. |
| `python3 -B scripts/test-clean-install.py` | Passed: 289 Git-visible files, nine fresh migrations, 102 standard Rust tests, locked frontend install/type/build, 24 browser tests, independent SQL for 13 runs and seven actions, secret scan and stop/start persistence; exact resources removed. |

The 138 selected integration results include two subprocess dispatcher entries (`boundary_child`, `milestone44_boundary_child`); 136 are substantive integration tests. The separate two library broker tests are substantive. Standard 102 passing tests exclude empty binary/rustdoc targets. Ignored tests in the normal invocation are not reported as passes there. Including the capacity opt-in, all 243 Rust test entries passed across the standard and opt-in invocations: 241 substantive tests and two subprocess dispatcher entries. Empty targets and repeated clean-install executions are not added to that unique count.

Rendered review: real evidence at 320/375/768/1024/1920; historical long action/request/queue identifiers at 320/768/1920; unknown and stale states. Screenshots were inspected at mobile, tablet and desktop widths. Keyboard focus restoration, focus containment, reduced motion and no horizontal overflow passed. No live pipeline responses were mocked.

### Fault evidence and corrections

| Injection boundary | Executed suite and independent oracle |
| --- | --- |
| Crash before send / after broker confirmation | `integrated_recovery`: real process death, lease expiry and durable ownership/outbox queries |
| Lost publication confirmation / cancellation | `rabbitmq_publisher`, `integrated_recovery`: bounded fault proxy, retained claim and broker/SQL inspection; no automatic same-claim replay |
| Definitive mandatory return / rejection | `rabbitmq_publisher`, `integrated_recovery`: retry schedule and terminal preservation, original envelope retained |
| Lost SQL COMMIT acknowledgement | `demo_runs`, `postgres_*`, `consumer_runtime`: proxy suppresses acknowledgement after server commit; independent SQL reconciles original identity/effect |
| Cancellation / lock deadlines / stale owner | Ownership/retry/recovery suites: post-lock database time, current revision and token checks |
| Quarantine confirmation loss / full or missing quarantine | `consumer_runtime`: retained source until preservation observed; no source ACK on failed preservation |
| Consumer crash before/after commit / lost ACK | `consumer_runtime`: redelivery, one durable effect/receipt and retained quarantine evidence |
| Prefetched concurrent action copies | Real browser repeated three-copy actions; one receipt/stock 15, distinct contexts, seven action queues empty with zero temporary consumers afterward |
| Interactive controlled failure | Real browser recovery: run-scoped one-time before-send rejection, stored retry timestamp/revision and final 10→15 |
| Historical unknown action | Owned SQL fixture changes a completed action's state to unknown; browser GET-only inspection preserves all identities/effects. This fixture is not claimed as another crash experiment. Actual interrupted-worker recovery is separately tested in `demo_runs`. |

Each destructive test runs on test-owned resources with finite harness/test deadlines and exact cleanup. No main process was killed to simulate a failure. Earlier unsuccessful harness attempts remain failures: startup timeout, root health-probe cookie permission failures, a Supervisor STARTING/socket race, a stale auto-submission test expectation, and an over-budget history setup. Fixes bound Erlang schedulers, run the broker probe as its broker user, wait for actual app/frontend readiness, wait for explicit action admission before testing close/reopen, and pace test setup under the unchanged 10/s quota. Reliability assertions and existing deadlines were not weakened. An obsolete clean snapshot was deliberately interrupted and cleaned before retesting the completed candidate.


### Clean installation and final main state

`python3 -B scripts/test-clean-install.py` passed on the final runtime candidate. Snapshot SHA-256: `06905b42520df88c35b7ee2deb5b3a72a4e4a0301020e4db7e93dccf338f4077` (289 Git-visible files). No original `.env`/`.demo.env`, database directory, volume, Cargo target/cache, node_modules or generated application binary was copied. Fresh random credentials, project name and host ports were supplied locally. The literal README `start-local.py` workflow ran without repairs, applied nine migrations before workers, created topology and waited for readiness. Standard Rust tests passed with their 141 opt-ins separately ignored; npm ci/type-check/build and 24 real/presentation browser checks passed. Every one of the 13 runs independently had a processed outbox, receipt 10+5=15 and stock 15. Seven actions had correlated Duplicate evidence and one effect/run. Stop/start preserved 13 runs, seven actions, 13 receipts and summed stock 195. Production assets contained none of the generated token/passwords. Missing `.env`, malformed token and credential placeholders were separately rejected before infrastructure startup with actionable redacted errors.

The clean harness uses a temporary TCP proxy solely to preserve the isolated localhost Origin/port for the test browser; the actual frontend/API/SQL/broker pipeline is unchanged. It creates no second permanent application environment. The recorded digest covers files at test start; this final results ledger/status was written afterward. Runtime source was unchanged after the final snapshot.

Capacity verification passed 22 trials across Baseline, Duplicates, Contended, Larger, Paused, Locked, Invalid and Stale; all reported unexplained=0. These are bounded local observations in a shared Docker VM, not production throughput or latency guarantees. The management observer collected 200 samples with zero errors; aggregate sampled maxima were 79 ready and one unacknowledged message. Exact test containers, networks and volumes were removed and fixture SQL/broker cleanup verified.

The normal main `python3 -B scripts/start-local.py` workflow also passed. Existing nine migration versions/checksums were current; no main migration, credential rewrite or backup restoration was necessary. App/frontend/broker containers were recreated for the candidate configuration, PostgreSQL and all durable storage retained. HTTP, relay and consumer are RUNNING, all three readiness endpoints return 200, and `/proc/<pid>/exe` SHA-256 equals the corresponding on-disk candidate binary for each process. This distinguishes source/build artifacts from binaries actually loaded.

All 11 table counts and canonical content fingerprints matched before/after activation: outbox, replay audit, stock, consumer receipts, runs, goods receipts, actions, publications, consumer observations, controlled faults and migration ledger. Combined fingerprint: `227e045692aae560b23fe417d47106467f350481faa40de33eb53b2e698bdd1a`. Main data remains one demo run/action, two global consumer effect receipts including the older non-demo effect, nine migrations; retained demo stock 15 with one effect receipt. Source/action/quarantine queues have zero ready/unacknowledged messages; source queue has one consumer, retained action queue zero. Authenticated retained-run history was inspected without POST. Existing legacy files/volumes were untouched. No queue purge, receipt deletion, volume reset, stage, commit or push occurred.

Remaining blockers: none for the agreed local portfolio acceptance. Remote CI was not run or verified; public deployment, TLS/HA, production recovery/retention and generalized repair remain outside this acceptance.
