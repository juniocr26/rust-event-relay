[Other language](../../pt-BR/operations/replay.md)

# Operator replay procedure

Privileged API is disabled when REPLAY_OPERATORS is unset or empty. Existing `/health` remains HTTP liveness, independent of database/broker readiness. Apply migration `20261013000000_add_audited_replay.up.sql` only after separate shared-rollout authorization. Startup never runs migrations; enabling replay verifies table/privileges read-only and fails sanitized if missing. Existing relay can operate with migration 5 absent because worker does not use replay tables.

## Local configuration

Generate independent random tokens with an OS CSPRNG (32 random bytes encoded as 64 lowercase hex characters). Keep the secret configuration outside Git, shell history and logs. Configure the HTTP process through protected environment/secret injection:

```json
[{"token":"<64 lowercase hex characters from 32 random bytes>","actor":"local-operator","allow_replay":true,"maximum_budget":10}]
```

REPLAY_OPERATORS holds this JSON. It accepts at most 16 distinct actors/tokens, 8192 config bytes, actor 1–64 ASCII alphanumeric/._- bytes and maximum budget 1–100. Invalid nonempty configuration fails startup. Do not use this placeholder as a credential. Compose passes the variable only to app; never put privileged secrets in frontend VITE_* variables. Changing configuration requires explicitly authorized HTTP activation/redeployment; the running process does not hot-reload credentials. Bind privileged access to localhost/trusted administration network; HTTP port is mapped to host loopback today. Before remote deployment require TLS with a trusted terminating proxy, restrict direct backend access, and prevent Authorization header logging. Tokens in environment are accessible to trusted process/container administrators. No cookies or CORS grants are selected; credential headers are explicitly supplied, so ambient cookie CSRF does not apply. XSS/secret theft remain outside that protection.

## Reproducible requests

First obtain current event UUID, dead_letter cycle/revision and validation evidence through authorized database administration. This API is not an event listing/payload inspection endpoint. Choose a request UUID once and save it with the complete intended body before sending. Choose a bounded nonsecret reason; free text is stored in restricted audit and must not contain credentials/content.

Create `replay.json` with actual observed identifiers (values below are syntax examples):

```json
{"request_id":"0199a000-0000-7000-8000-000000000001","event_id":"0199a000-0000-7000-8000-000000000002","expected_revision":4,"expected_cycle":1,"reason":"routing restored after operator review","budget":10,"delay_ms":0,"ticket":null}
```

Use a protected curl configuration file whose Authorization header is injected by your secret mechanism; the examples contain no credentials and avoid putting tokens in command arguments/URLs:

```sh
curl --config /secure/operator-curl.conf --request POST   --header 'Content-Type: application/json' --data-binary @replay.json   http://127.0.0.1:8080/operator/replays
curl --config /secure/operator-curl.conf   http://127.0.0.1:8080/operator/replays/0199a000-0000-7000-8000-000000000001
```

200 returns request/event identity, cycle, revision, baseline, budget, decided_at and available_at. Repetition has the same 200 receipt. 401 rejects invalid/missing bearer identity; 403 rejects missing replay permission/over-budget policy; 400 invalid body/fields; 409 intent/state conflict; 422 invalid stored content/metadata/capacity; 503 persistence failure or explicit commit_uncertain_inspect_same_request. Lookup returns 404 for absent or another actor's receipt. Bodies exceeding 4096 bytes are rejected with sanitized 400. Health is unchanged; GET never opens a cycle.

After network cancellation/503 commit uncertainty, inspect the same request through authenticated GET or resubmit the identical saved request. Do not change reason/budget/expected predicates or request ID to resolve uncertainty. A 404 can precede a later in-flight commit. Same-request resubmission coordinates against concurrent invocation, but lock/statement timeouts can still fail; no internal retry loop. Transport/auth/business rejection diagnostics are sanitized, not transactional rejection audit.

A receipt re-enables work only. Independently observe durable event/audit state and broker acceptance. Normal worker may already have advanced status/revision; receipt remains historical. Do not infer consumer effect or undo prior broker/consumer effects. Repair requires a separate authorization/audit protocol; this milestone implements none.

See [ADR 009](../adr/009-authorized-idempotent-replay.md) and [validation/rollout](../architecture/milestone-3-5.md).
