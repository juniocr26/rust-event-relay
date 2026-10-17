[Português brasileiro](../../pt-BR/operations/milestone-6-2.md) | [Catalog](../index.md)

# Milestone 6.2 — durable duplicate delivery

Use the main application at http://localhost:5173, with PostgreSQL, RabbitMQ and the existing supervised HTTP/relay/consumer. No separate demo runtime is needed. ADR 012's action worker, publisher, temporary consumer sessions and evidence tables are reused. The new migration permits three actions per run (100 retained globally), without changing prior migration checksums or deleting evidence.

Select Duplicate delivery, open the modal, enter the local DEMO_TOKEN, and load successful runs. Select a run and execute. Alternatively create a fresh 10 + 5 example. A new deliberate action has a new durable request UUID; retries reuse it. Closing the modal stops polling, not work. Reopening/reloading preserves the selected request and action identity in sessionStorage. “Request another duplicate” explicitly admits another action; recovering an unknown action never republishes it.

## State and identity

Admission serializes under the existing transaction advisory lock. Identical request/run/copies returns the stored action even at capacity; conflicting request reuse returns 409. Admission requires original relay completion and a committed inventory receipt. No producer fact, stock fixture, outbox acquisition or receipt is recreated. Accepted actions commit as queued; one worker claims executing before broker I/O. Queued actions resume after restart. Any interrupted executing action becomes unknown, including a crash before send. It is never automatically republished. Execution completes confirmed only after bounded publisher confirms and consumer Duplicate processing/ACK-send success; publication observations remain a separate source of truth. The inherited failed state is reserved; failures currently become unknown conservatively. SQL and AMQP cannot commit atomically.

Each action owns durable queue/routing key `relay.demo.<action UUID>` in the existing vhost/exchange. That stored unique topology supplies correlation without modifying envelope, message_id or original business timestamps. Only its temporary sessions are assigned its action context; the repository additionally validates action/run association. Immutable-content validation and normal deduplication always run first. No new transport metadata is introduced. Queue correlation assumes the existing trusted local broker principal; it is not cryptographic origin proof.

The stored original receipt proves one committed business effect, including 10→15. A separate correlated Duplicate record proves the real processor compared immutable content and committed its classification without another stock mutation. Historical receipts do not imply a recorded Applied observation. A count of effect receipts and current stock are queried independently in the statement snapshot. A publisher confirmation does not prove consumer completion. An observation commits before ACK: uncertain ACK may redeliver and add another bounded Duplicate record, with the same single effect. Observation counts are lower bounds, not unique delivery counts. Lost confirms may mean the broker accepted a message even when publication is unknown. Neither exactly two arrivals nor exact broker ACK receipt is promised.

## Bounds and verification

Existing shared local bearer principal, exact browser Origin check, 1 KiB body, 10 requests/second, two HTTP handlers, 8-second HTTP timeout, four queued/executing actions, one executing worker, two action consumer contexts/pool connections, 20-second execution deadline and 1–3 publisher calls remain. Three retained actions/run and 100 total actions bound queue growth. Publication and consumer observation tables retain at most 32 rows/run each. Saturated counts are lower bounds. No purge/TTL/receipt deletion. New admission rejects capacity without evicting evidence. Polling is sequential at one second, for at most 60 seconds; request timeout is at most nine seconds. Pending, unknown, unavailable and stale results remain distinct.

Read-only SQL is available in the modal. For a selected UUID:

```sql
SELECT * FROM demo.actions WHERE run_id = '<run UUID>' ORDER BY accepted_at;
SELECT * FROM demo.consumer_observations WHERE action_id = '<action UUID>';
SELECT * FROM demo.publications WHERE action_id = '<action UUID>';
SELECT event_id, immutable_content, stock_before, quantity, stock_after, completed_at
FROM consumer.receipts WHERE event_id = '<event UUID>';
SELECT count(*) FROM consumer.receipts WHERE event_id = '<event UUID>';
SELECT s.units FROM consumer.stock s JOIN demo.runs r
ON (s.warehouse_id,s.sku)=(r.warehouse_id,r.sku) WHERE r.run_id = '<run UUID>';
```

RabbitMQ management at http://localhost:15672 shows the retained action queue; aggregate queue statistics alone do not prove Duplicate. Unknown actions retain their SQL and broker evidence for inspection; no automatic resend or general replay control is provided. Expanded recovery remains 6.3.

```sh
python3 scripts/start-local.py
python3 -B scripts/check-migrations.py
python3 -B scripts/consolidate-storage.py --apply
# Backup + disposable restore verification precede any pending main migration.
docker compose exec -T app cargo build --locked --bins
docker compose exec -T app supervisorctl restart http
python3 -B scripts/test-application.py
```

The new down migration restores the single-action constraint and refuses when multiple actions exist; it never deletes evidence to force rollback. Older action/run down migrations are destructive and must not be applied to retained application data. Forward upgrades preserve historical records. The real browser harness uses a disposable copy of main Compose, no pipeline mocks, and cleans up only its exact test resources.

References: [RabbitMQ confirms and acknowledgements](https://www.rabbitmq.com/docs/confirms), [PostgreSQL Read Committed](https://www.postgresql.org/docs/current/transaction-iso.html).

## Local validation — 2026-10-17

Executed: 27 library tests; 11 isolated demo PostgreSQL tests; 52 existing consumer/application/broker regressions; one additional real dropped-ACK correlated-observation test; 25 real browser checks across the main-Compose disposable harness. Final browser campaign passed, including historical selection, repeated actions, modal close/reopen, reload token lifecycle, five viewport widths, focus and reduced motion. Rendered mobile and historical-action screenshots were inspected. Strict all-target/all-feature Clippy, Rust binary build/rustdoc/formatting, Vue type/build, nine ordered migration upgrades/checksums/no-op/down/re-up, four documentation-checker tests, 121 Markdown link checks and Compose validation passed. The initial browser campaign exposed an outdated token-lifecycle assertion, corrected before the final passing campaign. Remote CI was not executed.

Main backup/restore proof: `.local-backups/consolidation-20261017T173714Z` (private, untracked). The prior outbox/receipt/stock snapshot matched exactly after migration. Running app/frontend containers still used pre-consolidation environment variables; only those two application containers were recreated to activate the existing Compose configuration. Infrastructure containers/data were preserved.

Main acceptance run `01a14af3-4765-747a-ac4d-c471ffde7fbe`, event `01a14af3-4765-747a-ac4d-c47252bdb61b`, action `01a14af3-4b6f-7090-9215-4572e226394b`: independent SQL verified stock 15, one effect receipt and one correlated Duplicate observation. Nine migrations successful; one retained demo run/action; two total effect receipts including the original pre-existing receipt. Main source/action/quarantine queues had zero ready/unacknowledged messages; the source consumer remained active. Disposable test databases/Compose resources were removed. No staging, commit or push.

A browser snapshot becomes stale after 15 seconds without a successful refresh; its database observation timestamp remains visible.

The complete default `cargo test --locked` suite also passed; opt-in integration tests remain separately identified above.

A repeated campaign exposed an inherited concurrent-session reservation deadlock: after its first ACK, slot-2 could reserve the third copy while slot-1 still waited for it. Slot-2 now closes immediately after its assigned processing, releasing any prefetched unsettled copy for broker redelivery. This introduces no new publication call and preserves deduplication. The browser campaign additionally runs three real concurrent actions to cover this boundary. The freshness check observes the actual 15-second aging timer with no mocked pipeline response.
