# Frontend

See [English setup](../docs/en/guides/frontend-backend.md) and [instruções em português](../docs/pt-BR/guides/frontend-backend.md).

Node 22.12+; `npm ci`; `npm run dev`; `npm run type-check`; `npm run build`.

The full-viewport Vue experience rebuilds the approved blue prototype without its export wrapper. A compact launcher opens a native modal over the landing page. All five scenarios are local illustrations: no API requests, chaos commands, broker operations or consumer execution. Consumer idempotency is a future design preview; Milestone 3 is outside this scope.

Typed scenario data lives in `src/scenarios/`; `usePlayback` coordinates one timer, readable 3.6-second steps and cleanup on replay/unmount. Simulated event timestamps are independent of playback time. `BaseButton` and `BaseSelect` own shared controls; `BaseModal` owns native dialog semantics, focus containment/restoration, Escape and body scroll locking. `ScenarioLauncher` and `ScenarioExperience` compose `StageProgress`, `MessageFlow`, `ActivityPulse`, `EnvelopePreview` and `EventOutput`. Shared CSS tokens control surfaces, color, spacing and typography. No production delivery decisions move out of Rust.

Browser tests: `npx playwright install chromium --with-deps`, then `npm test` (Linux system dependencies may require root). Tests start Vite when necessary, validate 320/375/768/1024/1920px layouts, keyboard/modal lifecycle, close during playback, replay, scenario-specific states and reduced motion. Screenshots are temporary `/tmp/relay-<width>.png` artifacts. Tests use virtual time; the user-facing presentation remains deliberately paced.

Future API integration requires a separately scoped adapter and actual backend observations. Broker acceptance must remain distinct from consumer processing; ownership tokens do not fence external publication; uncertain completion is not proof of commit or rollback. External effects do not automatically share local database atomicity. `VITE_*` configuration is public; keep credentials out of browser configuration.
