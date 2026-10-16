**Current local state — Milestone 4.5 (2026-10-16):** [Verified rollout and recovery](milestone-4-5-rollout.md). Six migrations applied; current HTTP/relay/consumer artifacts installed and running; consumer autostart=true. Protected backup/isolated restore and real 10 → 15 → 15 demonstration verified. Replay API remains disabled; frontend remains simulation. Earlier observations below retain their dates.

[Português brasileiro](../../pt-BR/operations/milestone-4-4-review.md) | [Catalog](../index.md)

# Milestone 4.4 — full verification and Milestone 4 closure

Audit date: 2026-10-16. Base: `cabfbcaa3f31f2be08657c400ef98275acc78df4`; initial tracked/untracked Git status was clean. No files were staged, committed or pushed. Only this repository was edited. No applicable project AGENTS.md was found in the checkout or its ancestor directories; dependency-local instructions do not govern application files. No available artifact/plugin skill applied to this source/runtime audit.

**Decision: Milestone 4 closed with explicit nonblocking limitations.** The defined local PostgreSQL inventory effect and durable receipt, real RabbitMQ delivery/quarantine protocol, failure recovery, bounded lifecycle and illustrative presentation have executable evidence. This is not complete-project readiness, production readiness, exactly-once delivery or shared consumer rollout. Earlier [4.1](../architecture/milestone-4-1.md), [4.2](../architecture/milestone-4-2.md), [4.3](../architecture/milestone-4-3.md), [ADR 010](../adr/010-atomic-consumer-and-quarantine.md) and [Milestone 3 review](milestone-3-review.md) retain their historical results.

## Acceptance matrix

“Verified” means source inspection plus the specific executed observations below, not merely compilation/health. Test names refer to [consumer_runtime.rs](../../../backend/tests/consumer_runtime.rs), [postgres_consumer.rs](../../../backend/tests/postgres_consumer.rs), [integrated_recovery.rs](../../../backend/tests/integrated_recovery.rs) and their real protocol harnesses. Default tests with fakes establish policy, not broker/database behavior.

| Area / contract | Classification | Evidence / limits |
| --- | --- | --- |
| Config parsers, conditional requirements, Compose injection and local secrets | Implemented and verified | [Full variable inventory](../docker/environment-inventory.md); sanitized configured-versus-container comparisons; authenticated TCP/management/protocol tests. No credential output/change. |
| Atomic stock and complete historical receipt | Implemented and verified | 12 PostgreSQL cases: 10+5=15, duplicate remains 15, distinct ID increments, immutable content conflicts, concurrent serialization, overflow, rejected statements/COMMIT, stored corruption, receipt deferred trigger and restricted-role writes. |
| Receipt/effect correspondence | Implemented and verified | Adapter writes both in one transaction; rollback/rejection/crash schedules independently inspect stock and receipts. Completeness trigger rejects placeholders. Arbitrary privileged SQL writers are outside the guarantee. |
| Representation v1 / replay identity | Implemented and verified | Lossless BYTEA and checked model equality, not JSONB numeric equality or hash. New `milestone44_sql_replay_preserves_identity_and_consumer_effect` exercises real replay ledger → relay publication → verified consumer Duplicate with one effect. Fixture uses PostgreSQL timestamp precision before initial publication. |
| Persistent mandatory publication / same-channel single ACK | Implemented and verified | Real confirms/returns, original delivery acker, no batch ACK; `milestone44_lost_source_ack_redelivers_committed_duplicate` parses and withholds actual basic.ack frame, independently sees committed receipt, closes only its proxy, observes redelivery/Duplicate. Client ACK send success is not server acknowledgement. |
| Concurrent same/distinct identities | Implemented and verified | New two real subscriptions + concurrent handler futures; same identity has one effect, two new IDs on one stock row both increment. PostgreSQL tests also coordinate winner/loser locks and conflicting content. No fairness/throughput guarantee. |
| Crash before commit | Implemented and verified | New compiled consumer blocked at independently observed stock lock, exact Child SIGKILL/reap, stock 10/no receipt, redelivery commits 15/one receipt. |
| Crash after commit before ACK | Implemented and verified | New test-only ACK barrier delegates real processing to production handler/adapters, kills its child after durable phase, independently sees 15/one receipt, real redelivery returns Duplicate and stock remains 15. No production fault flag. |
| Lost COMMIT response / cancellation after durable effect | Implemented and verified | Real PostgreSQL protocol proxy discards/withholds observed server COMMIT response. Existing and new integrated tests observe stock/receipt independently, cancel without ACK, then recover through Duplicate. No rollback inferred from timeout. |
| Invalid/unsupported/conflicting preservation | Implemented and verified | Actual raw bytes, message_id/properties and type-tagged bounded sidecar round-trip; forged source metadata remains nested. Failure categories are closed diagnostics. Source ACK follows routed confirm only. |
| Quarantine unavailable/unroutable/uncertain/full | Implemented and verified | Missing exchange, missing binding mandatory return, withheld confirmation, exact test queue x-max-length=1/reject-publish NACK; original remains unacknowledged and recoverable. Test-only capacity queue is never installed on shared topology. |
| Crash after quarantine confirm before ACK | Implemented and verified | New child ACK barrier, exact kill/reap, redelivery creates two preserved copies with original bytes and no stock effect. Existing lost-confirm test likewise permits copies. Quarantine publication/source ACK are not atomic. |
| Oversize operator recovery | Implemented and verified | New >1 MiB actual publication: compiled consumer stops cleanly, original bytes remain in source, no receipt/quarantine truncation. Requires explicit operator recovery, not automatic discard. |
| Prefetch, connection loss, restart, cancellation, bounded shutdown | Implemented and verified | Existing compiled consumer credit/drain/restart, server cancellation/resubscription, idle TERM, unavailable database and paced recovery tests; one active operation, prefetch 1..32. Finite tested schedules, not all interleavings. |
| Fresh/upgrade/down/up migrations | Implemented and verified | Six ordered real SQLx upgrades and SHA-384 checksums, no-op validation, consumer/replay down/re-up in exact disposable database; absence verified afterward. |
| Shared replay and consumer schema/runtime | Blocked by configuration or rollout | Four shared migrations only; replay/consumer schema absent, replay intentionally disabled, installed Supervisor has only HTTP/relay. No shared migrations/consumer activation authorized by this audit. |
| Supervisor named/direct/interactive lifecycle | Implemented and verified | Five worker smoke cases plus isolated consumer opt-in case, real test-owned manager/socket/children; intentional stop and unexpected-exit restart. Installed-versus-source comparison separately identifies drift. |
| Presentation / browser behavior | Implemented and verified | 13 Chromium tests at 320/375/768/1024/1920 widths; keyboard/focus/modal, playback cleanup/replay, reduced motion, wrapping, English, favicon, no console/resource/API errors. Mobile/desktop screenshots inspected. |
| Live frontend/backend integration | Planned and outside current scope | **Presentation frontend verified; live backend integration is not implemented.** No fetch/XHR API calls, Vite proxy, CORS or browser authentication integration. |
| CI jobs | Implemented but unverified remotely | Source reviewed; local equivalent checks passed. Added docs job and Supervisor dependency/full consumer suite to integration job. No remote CI execution claimed. |
| Production HA/TLS, backup/restore, benchmark, full accessibility certification | Planned and outside current scope | No evidence claimed; persistent volumes alone prove neither backup nor power-loss recovery. |
| Broker DLQ for SQL dead_letter / external consumer effects | Not applicable | SQL terminal state needs no RabbitMQ queue. Transaction covers local stock/receipt only; HTTP/email/etc. are absent. |
| Outstanding confirmed source defects | Defective: none remaining in audited scope | Corrections below passed affected checks. Shared installation drift is a rollout finding, not proof deployed code changed. |

## Corrections and investigation

1. **Stale presentation claim:** selecting duplicate previously asserted that the consumer did not exist. This contradicted implemented 4.2/4.3 and made study output misleading. Updated scenario/flow/waiting/input text to illustrate current inventory deduplication, include 10+5=15, retain explicit simulation labeling and update the browser expectation. No API integration added. [Scenario source](../../../frontend/src/scenarios/index.ts), [browser regression](../../../frontend/tests/experience.spec.ts).
2. **Shared Vite cache:** after lockfile installation, a separate read-only Chromium probe of shared port 5173 found no landing button and one failed resource, while clean port 5174 passed. Restarted **only frontend**. Repeat shared probe found one landing button, zero console/runtime errors and zero failed resources. Acceptance still owns a separate cache/server; database, broker and app were untouched.
3. **Verification gaps:** added nine substantive consumer cases and one child entry, including real protocol ACK loss rather than a fake successful result. New replay fixture initially used the wrong authorized scope and failed the database constraint; corrected to `outbox:replay` and database precision, retaining the Duplicate assertion. Compile-time proxy guard type mismatch was corrected with a generic JoinHandle guard. Final suites passed; no assertions weakened.
4. **CI/portability/documentation:** consumer Supervisor acceptance had been skipped remotely and assumed `/app/backend`. Isolated config now uses CARGO_MANIFEST_DIR; CI installs Supervisor and runs the full suite. Added local Markdown file-link validator plus four boundary tests and a docs CI job. Six historical sibling-repository links were changed to explicit external path references so a standalone clone does not depend on engineering-library. Historical claims/results were retained; no sibling file was changed. Validator excludes remote links/heading anchors and is not a prose/translation certification.

No confirmed inventory/ACK/quarantine production-code defect was found by these finite tests. Additional fault evidence was added rather than changing established contracts or dependencies.

## Shared database, broker and runtime observation

Read-only metadata: PostgreSQL healthy and authenticated at internal postgres:5432 / host 127.0.0.1:5433. Shared outbox rows=0. Applied successful versions 20261007000000, 20261007175358, 20261009000000, 20261012000000; all four SHA-384 checksums match source. Pending versions 20261013000000 (replay), 20261014000000 (consumer). No migration was applied to shared state. Schema preflight fails before consumer subscription if consumer tables are missing.

The configured development database role is superuser and CREATEDB. This supports disposable harness creation but is **not** evidence that deployed least privilege is installed. A new disposable NOLOGIN role regression verifies the documented grants permit Applied/Duplicate while denying receipt removal/identity updates/DDL; exact role absence is checked after cleanup. Consumer schema grants/receipt retention, owner/DDL bypass and effect limitations are explained in [4.2](../architecture/milestone-4-2.md). Install reviewed restricted roles/grants before broader deployment; do not change stored passwords to satisfy examples.

| Intended vhost `relay` resource | Expected from current code | Actual shared observation |
| --- | --- | --- |
| Relay exchange/source queue | durable direct relay.events → durable classic relay.local, routing event | Present; x-queue-type=classic, non-auto-delete; ready=0, unacked=0, consumers=0 |
| Consumer source | same explicit relay.events/relay.local/event defaults | Existing compatible resources, consumer disabled; another logical responsibility would need its own queue |
| Quarantine exchange/queue | durable direct relay.quarantine → durable relay.quarantine.inventory, routing inventory | Both absent; expected until explicit consumer rollout. Earlier 4.1 proposed names are historical, not current topology. |
| DLX/TTL/expiry/capacity policies | no automatic deletion/DLX/TTL; future capacity must reject-publish | No policies/operator policies; source effective policy empty. No shared test capacity policy installed. |
| Management/permissions/connections | authenticated management at loopback 15672; configure/write/read ^relay[.].* | Verified via API/node CLI; one visible existing connection, zero consumers at final inspection. No browser login certification in 4.4. |

No shared queue was consumed, test-published, purged, deleted or redeclared. SQL dead_letter is independent of broker quarantine. Only exact UUID test topology was created/removed.

Versioned Docker uses lowercase root build paths, explicit chmod COPY, developer user, `/app/backend`, mounted source/cache/target, persistent PostgreSQL directory/RabbitMQ volume, loopback ports and health dependencies. Entrypoint creates a private 0700 Supervisor directory; socket is 0600, named programs use TERM/process groups/30s stop waits, consumer autostart=false; source Compose grace=95s. Direct and interactive supervisorctl/supervisor work in isolated tests. `start all` / `restart all` can start consumer despite autostart=false: use named commands only.

Installed shared normal.conf differs from source; relay-consumer launcher is absent. Existing relay-worker/relay-http/supervisor wrappers match source. HTTP PID 11 and relay PID 12 remained running throughout. Loaded executable hashes differ from rebuilt on-disk files; `/proc/<pid>/exe` points at deleted old inodes. Exact loaded source revision cannot be established because binaries contain no revision marker. All three current binaries compiled successfully; Cargo rebuilt artifacts in `/app/target`, so a later named restart would load newer code. Compilation does not update existing processes. No app rebuild/recreation/reload/restart occurred. Only frontend was restarted for the confirmed stale resource.

## Executed checks and substantive counts

| Check | Actual result |
| --- | --- |
| cargo fmt --check; strict Clippy --all-targets --all-features | Passed on final test source |
| cargo test --locked | 87 passed, 120 ignored; zero doc-test examples. Ignored integration is not standard-suite evidence. |
| PostgreSQL repository / ownership-schema / ownership / retry / discovery / replay / consumer | 15 / 1 / 12 / 18 / 7 / 10 / 12 substantive passed |
| RabbitMQ publisher / closed owned connection | 2 / 1 substantive passed |
| integrated_recovery | 16 substantive passed + 1 internal child no-op |
| consumer_runtime (final) | 19 substantive passed + 1 internal child no-op; includes 9 new substantive 4.4 cases |
| worker_smoke (full, isolated Supervisor) | 5 substantive passed; direct/interactive lifecycle passed |
| Unique opt-in total | 118 substantive passed + 2 internal child no-op entries. Repeated development runs are not added to this total. |
| cargo build --locked --bins; strict rustdoc | All three binaries and no-deps docs passed with RUSTDOCFLAGS=-D warnings |
| npm ci; type-check; production build | Passed; browser initially missing, installed Chromium with Linux dependencies and reran successfully |
| npm test | 13 Chromium cases passed; final affected dedupe case passed again with stronger current-text/input/10+5 assertions; browser/cache owned by test server at 5174; rendered mobile/desktop inspected |
| Migration script; Compose validation; read-only PostgreSQL/RabbitMQ checks | Passed; exact disposable migration database removed; shared checksums/settings/topology inspected separately |
| Python / shell syntax | Eight Python files parsed; six shell entrypoints/launchers/scripts passed sh -n |
| Documentation validator / tests / git diff --check | Passed; 4 validator tests; local file links only, no remote/heading validation |

Commands run from repository root (Cargo working directory in app is `/app/backend`):

```sh
docker compose exec -T app cargo fmt --check
docker compose exec -T app cargo clippy --locked --all-targets --all-features -- -D warnings
docker compose exec -T app cargo test --locked
docker compose exec -T app cargo test --locked --test postgres_repository --test delivery_ownership_schema --test postgres_ownership --test postgres_retry --test postgres_discovery --test postgres_replay --test postgres_consumer --test rabbitmq_publisher --test integrated_recovery --test consumer_runtime -- --ignored --test-threads=1
docker compose exec -T app cargo test --locked --test worker_smoke -- --ignored --test-threads=1
docker compose exec -T app cargo test --locked --lib infrastructure::rabbitmq::tests::closed_owned_connection_is_unavailable -- --ignored
docker compose exec -T app cargo build --locked --bins
docker compose exec -T -e RUSTDOCFLAGS='-D warnings' app cargo doc --locked --no-deps
python3 scripts/check-migrations.py
sh scripts/check-postgres.sh
python3 scripts/check-rabbitmq.py --require-topology
docker compose config --quiet
docker compose exec -T frontend npm ci
docker compose exec -T -u root frontend npx playwright install --with-deps chromium
docker compose exec -T -u root frontend npm run type-check
docker compose exec -T -u root frontend npm run build
docker compose exec -T -u root frontend npm test
python3 -B scripts/check-docs.py
python3 -B -m unittest discover -s scripts -p 'test_*.py'
git diff --check
```

The new restricted-role PostgreSQL case was also run explicitly with `cargo test --locked --test postgres_consumer restricted_consumer_grants -- --ignored`. The long opt-in command first passed existing cases; final modified consumer_runtime was rerun in full, along with final strict Clippy/format/default tests. Shared destructive `scripts/check-supervisor.py` was deliberately not run; isolated worker/consumer cases provided lifecycle evidence instead. CI was inspected/configured, not remotely run. No dependency upgrade.

## Cleanup, operator actions and study path

Final independent catalog/API/filesystem inspection: zero relay_it_/relay_migration_ databases, zero relay.it. queues/exchanges, zero owned NOLOGIN test roles and recovery/Supervisor/boundary temporary paths; shared source still ready=0/unacked=0/consumers=0. Child handles were killed/reaped only when spawned by the tests; protocol proxies were retired and fixture cleanup checked exact resource absence. Ignored build caches, browser downloads, screenshots/logs remain verification artifacts. Cleanup after abrupt termination of the whole harness is not promised or tested by 4.4; the existing recovery harness separately tests assertion-failure cleanup.

No missing/empty/invalid required secret blocks local acceptance. For shared rollout: review backups/recovery and actual queue content, apply pending migrations in order explicitly, verify restricted database/broker permissions, review source admission/capacity/policies, rebuild/recreate only app to install known Supervisor config/environment/launchers and deliberately select named HTTP/relay restarts. Keep consumer stopped until an operator explicitly authorizes source consumption. If replay is wanted, provision `REPLAY_OPERATORS` with the exact protected format in the [inventory](../docker/environment-inventory.md); do not send secrets in chat. Quarantine replay/purge remains separately designed work.

Study `handle_delivery` → `PostgresInventoryProcessor::process` → receipt-completeness migration → RabbitDelivery::ack / quarantine publisher, then read the tests at each boundary. Start stock at 10: event A adds 5 and commits stock 15 plus receipt A; only then ACK. Duplicate A must match immutable content and returns its historical receipt, leaving stock 15. Different B adds 5 to 20. Conflicting A cannot change stock and must be preserved before ACK. Lost COMMIT response or ACK means uncertain delivery progress; inspect/reprocess the same identity, never blindly add/compensate. Quarantine confirmation followed by crash can produce two copies and still no inventory effect. Replay preserves A's identity; it cannot make A add another five or undo its effect. These are local-transaction guarantees under tested assumptions, not exactly-once delivery or external-effect atomicity.

Primary references checked during this audit: [Compose environment precedence](https://docs.docker.com/compose/how-tos/environment-variables/envvars-precedence/), [RabbitMQ 4.3 acknowledgements/confirms](https://www.rabbitmq.com/docs/confirms), [PostgreSQL 18 READ COMMITTED](https://www.postgresql.org/docs/18/transaction-iso.html). Installed Lapin/SQLx source and actual tests govern client-specific behavior. Remaining nonblocking limits: no live frontend API, production HA/TLS/least-privilege rollout, broker power-loss experiment, backup/restore, performance benchmark, Firefox/WebKit or full accessibility certification. Recommended next step is a separately reviewed local rollout, followed by independent stock/receipt/topology observation; broader features need their own scope.
