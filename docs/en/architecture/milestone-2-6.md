[Português brasileiro](../../pt-BR/architecture/milestone-2-6.md) | [README](../../../README.md)

# Milestone 2.6 — Integrated recovery experiment plan

Starting commit: `d6f529b`; clean working tree. The requested 2.5 report is absent. Versioned source, milestones 2.3–2.5 and ADR 007 are the baseline. No applicable project/ancestor AGENTS.md was found in either repository. Shared state observed: HTTP RUNNING, relay STOPPED, ownership migration pending. No shared rollout is authorized.

## Matrix (written before test implementation)

All cases insert a known set of IDs into an exact `relay_it_<UUID>` database, apply migrations there only, and use exact `relay.it.<UUID>.exchange/queue` resources. Observer messages require explicit ACK. Database polling uses database time, bounded by a deadline; polling intervals do not establish the boundary.

| Case | Controlled boundary/fault and synchronization | Durable expectation / possible broker state | Recovery and assertions / limits |
| --- | --- | --- | --- |
| Baseline | Unmodified relay, poll processed | Processed, count 1, ownership cleared; original envelope accepted | ACK original envelope/identity; graceful TERM. Fixtures do not prove producer atomicity or consumer effects. |
| A | Test executable uses real adapters/use case, publisher entry barrier writes ready file only after committed acquisition; parent SIGKILLs exact child | Pending/count 1/live token; zero messages because adapter send has not begun | Observe real database-clock expiry; restart unmodified relay; fresh token/count 2, original identity, processed. Instrumented process, not unmodified binary at crash boundary. |
| B | Same executable; publisher delegates real confirmed publish, then writes barrier before returning to use case; SIGKILL | Pending/count 1; one contract-confirmed message, no completion started | Real expiry then real relay; second message with same ID, count 2/processed. Explicit duplicate window. |
| C | Reuse isolated byte-withholding AMQP proxy after setup; real use case | Uncertain/retained lease, pending; broker inspected independently | Retired publisher and replacement decision; fresh connection must not replay claim; real expiry/fresh acquisition completes. Accepted first message can duplicate. Byte proxy is not selective AMQP frame loss. |
| D | Reuse PostgreSQL protocol COMMIT response-loss proxy only for completion through a forwarding repository | Publication confirmed; CommitUncertain; independent direct pool can observe processed | No release or republish; restart real worker, count unchanged, no second message. This injection loses acknowledgement after observed server COMMIT, not every uncertain commit. |
| Failed completion | Isolated deferred trigger rejects processed UPDATE at COMMIT | Confirmed publication, ordinary failure, pending lease retained | Drop fixture trigger; real expiry and fresh acquisition; duplicate same identity, later processed. |
| Two workers | Two unmodified binaries; isolated completion trigger waits on test-owned advisory lock; inspect durable tokens and actual lock wait | Two pending durable distinct owners/count 1; two confirmed publications before completion | Unlock, both processed, original IDs. Supports ownership exclusivity for this schedule; no fairness/order/universal duplicate claim. |
| Stale owner | Real acquisition A, database-clock expiry, acquisition B | Pending with B token/count 2 | A complete/release rejected; A can still publish externally; B confirmed publication/complete accepted. Tokens do not fence RabbitMQ. |
| Known unavailable | Real binary routed to test-owned unavailable listener/proxy before acquisition; inspect attempt count; signal in reconnect wait | Pending/count 0, no broker send | Restore proxy forwarding; fresh admission and completion; TERM/INT bounded. No shared service fault. |
| Broker disconnect during delivery | Real worker core with test-scoped lifecycle; withhold confirms after setup, independently observe first message, abort only owned proxy | Uncertain, pending/count 1/live token; broker contains original message | Observe actual second connect, retained live lease, no immediate replay; elapsed expiry/count 2 processed. Lifecycle construction is instrumented; worker policy and both adapters are real. |
| Blocked database / shutdown | Isolated lock holder, session lock/statement deadlines; real binary drain deadline during blocked completion | No prematurely committed processed transition; confirmed message can exist | Release owned lock and lease-based restart; cooperative timeouts only. Separate graceful shutdown from SIGKILL. |

## Cleanup ownership

The outer harness retains exact child handles, proxy tasks and a temporary directory even if the assertion task panics. It kills/reaps only its children, aborts/joins only its proxies, deletes only its queue and exchange (binding removed with queue), closes its observer, removes only its temporary directory, and lets the isolated database harness close pools/drop/verify its exact database. Resource names are printed without credentials or payloads. Abrupt harness death can leave resources: use the printed exact database/exchange/queue identities, verify no owned child remains, then drop/delete those exact identities; never wildcard delete or purge shared queues.

## Executed observations — 2026-10-12

The matrix above was written before implementation. Final execution is recorded in [sanitized transcript](../../../Reports/MILESTONE_2_6_EXPERIMENTS.txt), with each exact inserted ID, fixture database/broker identity, observed message IDs and final durable snapshots. The suite has **10 experiment tests plus one internal child entry**; the crash test runs two fixtures. A separate cleanup regression deliberately panics, then independently verifies the owned child is reaped and the exact database, topology and temporary directory are absent. Its expected panic diagnostics do not mean a failed suite.

| Scenario | Observed database / broker facts |
| --- | --- |
| Baseline (existing worker_smoke, strengthened) | One acquisition; processed_at present, all ownership fields cleared; exact envelope/payload and message_id match; explicit ACK and successful TERM. |
| A, SIGKILL before send | Pending/count 1 with token, unchanged after kill; queue empty. Actual expiry; restarted binary held at isolated completion barrier with fresh durable token/count 2; original message; unlock → processed/count 2, all ownership cleared. |
| B, SIGKILL after adapter-confirmed acceptance | Original message explicitly ACKed before kill; pending/count 1 unchanged. Actual expiry; restarted binary fresh token/count 2; second original ID message, processed/count 2. |
| C, withheld confirmation | Real adapter returns Uncertain/retired; use case LeaseRetained; pending/count 1. Independent observer sees accepted original message. New publisher's use case returns NoEvent during retained lease; actual expiry → fresh token/count 2, confirmed publication and Applied completion, duplicate original ID. |
| Broker disconnect / worker replacement | First original broker message observed while confirms withheld; owned proxy aborted to disconnect transport during publication. Observed Uncertain, exactly two lifecycle connects; replacement occurs while pending/count 1 lease is still live; eventual count 2 processed and second original ID message after eligibility. |
| D, completion COMMIT ACK lost | Confirmed publication; completion CommitUncertain. Proxy independently observes COMMIT completion; direct pool sees processed/count 1 with no owner. Direct acquisition returns None; restarted binary reaches an actual NoEvent invocation; no second message/count change. |
| Ordinary completion rejection / stale owner | Deferred server constraint rejects processed commit as OperationFailed; pending/count 1 and first confirmed message. Actual expiry → B fresh token/count 2. A complete/release both OwnershipLost, B token unchanged. Former publisher can still send original envelope; B then sends/gets confirm and completes. Three observed original-ID messages show external publication is not fenced. |
| Concurrent binaries | Two IDs, durable pending/count 1 with distinct tokens and live leases; two actual database lock waits at completion barrier. Exact broker ID set equals inserted set. Unlock → both processed/count 1; TERM and INT succeed; no extra queued message. This schedule does not measure fairness or all ownership races. |
| Unavailable before acquisition | Owned handshake-stalling proxy yields connection-failed log; pending/count 0, empty queue. Enable only future proxy connections → original message and processed/count 1. TERM in idle and INT in reconnect exit within four-second assertion. |
| Database deadline | Isolated advisory lock blocks completion; dedicated pool lock_timeout 100 ms / statement_timeout 300 ms; ordinary OperationFailed within two seconds, confirmed first message, pending/count 1. Unlock and real expiry; restarted binary count 2 processed, duplicate original ID. |
| In-flight shutdown | Unmodified binary confirmed first publication and waited in isolated completion trigger. TERM exceeds 200 ms drain and cancels operation; exits within four seconds under 2000 ms configured overall cleanup. Pending/count 1 persists. Unlock, actual eight-second acquisition lease expiry, restarted binary → processed/count 2 and duplicate original ID. |

No production defect was confirmed and no production source/schema/configuration was changed. Fixes during implementation were confined to test imports, deterministic coordination, observer ordering and cleanup ownership. Existing fault implementations were extracted unchanged in behavior into [support/faults.rs](../../../tests/support/faults.rs); historical adapter regressions still use them. The baseline observer now verifies every cleared ownership field and stable AMQP message identity.

## Reproduction and duration

Use the existing configured PostgreSQL/RabbitMQ services and app image. Do not apply the shared ownership migration. PostgreSQL credentials need CREATE DATABASE; broker credentials need topology permissions. Tests use plaintext local proxy connections. Run on Linux with `kill` available; the Docker image already supplies these requirements. No shared process is signalled. Child helper and relay binary do not launch descendants; tracked direct children are killed/reaped before deletion.

```bash
docker compose exec -T app cargo test --locked --test worker_smoke -- --ignored --test-threads=1
docker compose exec -T app cargo test --locked --test integrated_recovery -- --ignored --test-threads=1 --nocapture
```

The new suite takes approximately 27 seconds locally after compilation; existing three smoke cases take approximately 23 seconds. Compilation/CI hardware can add time. Each observation is bounded at eight seconds, each experiment task at 30 seconds; broker cleanup at eight seconds and outer isolated-database fixture at 45 seconds. SIGTERM/SIGINT assertions allow four seconds including signalling/reaping, distinct from the production cooperative cleanup budget. Recovery polling uses real PostgreSQL clock expiration; no new experiment edits lease timestamps. Existing older regressions explicitly include synthetic timestamp fixtures.

The [CI job](../../../.github/workflows/ci.yaml) provisions both disposable services and selects the baseline binary smoke plus recovery suite. It is configured, **not remotely executed** in this session. Ordinary `cargo test` keeps all long infrastructure cases ignored. Native equivalents require the same POSTGRES/RABBITMQ environment variables and a reachable local broker; credentials are never command-line literals.

Exact cleanup is verified using database catalog absence, failed passive broker declarations after deletion, removed temporary directories, and awaited children/proxy tasks. Test observer ACKs remove messages intentionally; this is not relay loss. The deliberate-failure regression verifies cleanup after a panic. SIGKILL/power loss of the harness itself cannot promise cleanup; inspect the transcript's exact names, verify the corresponding owned processes/connections are gone, and remove only those identified objects. Do not use prefix-wide deletion.

## Acceptance and next work

[Milestone 2 review](../operations/milestone-2-review.md) maps requirements to evidence and closes implemented scope under stated assumptions. [Validation record](../testing/validation-results.md#milestone-26--2026-10-12) distinguishes local executions from CI configuration. Shared rollout stays pending with relay STOPPED/autostart=false. Milestone 3 requires decisions about durable retries, classification, poison isolation, quarantine/dead-letter and auditable replay; no such policy is implemented here.

