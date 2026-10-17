import { ref, watch, onBeforeUnmount } from 'vue'
export interface Action {
  action_id: string; request_id: string; run_id: string; copies: number; state: string; queue_name: string
}
export interface Observation { observation_id: string; action_id: string | null; context: string; outcome: 'Applied' | 'Duplicate'; observed_at: string }
export interface Evidence {
  run_id: string; request_id: string; event_id: string; warehouse_id: string; sku: string
  scenario: string; initial_stock: number; quantity: number; accepted_at: string; fact_id: string
  outbox: { status: string; created_at: string; processed_at: string | null; attempt_count: number; available_at: string }
  receipt: { consumer_name: string; event_id: string; quantity: number; stock_before: number; stock_after: number; completed_at: string } | null
  effect_receipts: number; actions: Action[]; stock: number | null; observed_at: string; action: Action | null; observations: Observation[]
  publications: { action_id: string | null; ordinal: number; outcome: string; observed_at: string }[]
  fault: { condition: string; injected_at: string | null; retry_available_at: string | null; retry_revision: number | null } | null
}
export interface Location { database: string; database_user: string; database_host: string; database_port: string; management_url: string; vhost: string; exchange: string; queue: string; routing_key: string }
// Credential is only in page memory. Reload requires reauthentication; IDs survive in sessionStorage.
const pageToken = ref('')
export function useDemoRun(scenario: string) {
  const storageKey = `relay-request:${scenario}`
  const token = pageToken, requestId = ref<string | null>(sessionStorage.getItem(storageKey)), evidence = ref<Evidence | null>(null)
  const eligibleRuns = ref<{ run_id: string; request_id: string; event_id: string }[]>([]), selectedRequest = ref(sessionStorage.getItem(`${storageKey}:selected`) ?? '')
  const history = ref<{ run_id: string; request_id: string; scenario: string }[]>([]), historyOffset = ref(0), historyBusy = ref(false)
  const selectedAction = ref(sessionStorage.getItem(`${storageKey}:action`) ?? '')
  watch(selectedAction, value => sessionStorage.setItem(`${storageKey}:action`, value))
  const location = ref<Location | null>(null), state = ref('Ready for real execution'), busy = ref(false), stale = ref(false)
  let freshnessTimer: ReturnType<typeof setTimeout> | undefined
  let controller: AbortController | undefined, timer: ReturnType<typeof setTimeout> | undefined, generation = 0
  const stop = () => { generation++; controller?.abort(); clearTimeout(timer); clearTimeout(freshnessTimer); busy.value = false; if (evidence.value) stale.value = true }
  async function call<T>(path: string, signal: AbortSignal, body?: object): Promise<T> {
    const response = await fetch(path, {
      method: body ? 'POST' : 'GET', signal,
      headers: { Authorization: `Bearer ${token.value}`, 'X-Demo-Client': 'demo61', ...(body ? { 'Content-Type': 'application/json' } : {}) },
      ...(body ? { body: JSON.stringify(body) } : {}),
    })
    if (!response.ok) throw new Error(response.status === 404 ? 'No committed run is visible for this request. An in-progress transaction may still commit. Inspection creates no work.' : response.status === 401 ? 'Authorization required. Check the demo token.' : response.status === 422 ? 'Transaction rejected. Inspect the same request identity.' : response.status === 409 ? 'Request identity conflicts with the submitted intent.' : response.status === 429 ? 'Application budget exhausted or read rate exceeded.' : `Observation unavailable (HTTP ${response.status}). Submission outcome may be unknown.`)
    return response.json() as Promise<T>
  }
  const complete = (e: Evidence) => {
    const effect = e.effect_receipts === 1 && !!e.receipt && e.stock === 15 && e.receipt.stock_before === 10 && e.receipt.stock_after === 15 && e.outbox.status === 'processed'
    if (!effect) return false
    if (scenario === 'recovery') return !!e.fault?.retry_available_at && e.outbox.attempt_count >= 2
    if (scenario === 'dedupe' || scenario === 'concurrent') {
      const count = scenario === 'dedupe' ? 1 : 3
      return e.action?.state === 'confirmed' && e.observations.filter(o => o.action_id === e.action?.action_id && o.outcome === 'Duplicate').length >= count
    }
    return true
  }
  async function execute(inspectOnly = false) {
    stop(); if (!inspectOnly) selectedAction.value = ''; const mine = generation; busy.value = true
    if (selectedRequest.value) { requestId.value = selectedRequest.value; sessionStorage.setItem(`${storageKey}:selected`, selectedRequest.value) }
    requestId.value ??= crypto.randomUUID(); sessionStorage.setItem(storageKey, requestId.value)
    const deadline = Date.now() + 60_000
    async function step(submit: boolean) {
      if (mine !== generation) return
      if (Date.now() >= deadline) { state.value = 'Observation timed out. The durable run may still complete.'; stale.value = true; busy.value = false; return }
      const active = new AbortController(); controller = active
      const timeout = setTimeout(() => active.abort(), Math.min(9000, deadline - Date.now()))
      try {
        if (!location.value) location.value = await call<Location>('/demo/config', active.signal)
        if (mine !== generation) return
        let result = await call<Evidence>(submit ? '/demo/runs' : `/demo/runs/${requestId.value}`, active.signal, submit ? { request_id: requestId.value, scenario, initial_stock: 10, quantity: 5 } : undefined)
        if (mine !== generation) return
        const actionKey = `relay-action:${result.run_id}`
        let actionRequest = sessionStorage.getItem(actionKey)
        if (!inspectOnly && selectedRequest.value && !actionRequest) { actionRequest = crypto.randomUUID(); sessionStorage.setItem(actionKey, actionRequest) }
        if (!inspectOnly && (scenario === 'dedupe' || scenario === 'concurrent') && result.receipt && result.stock === 15 && result.outbox.status === 'processed' && (!result.action || !!actionRequest && !result.actions.some(a => a.request_id === actionRequest))) {
          const action = await call<Action>('/demo/actions', active.signal, { request_id: actionRequest ?? requestId.value, run_id: result.run_id, copies: scenario === 'dedupe' ? 1 : 3 })
          if (mine !== generation) return
          result = { ...result, action }
        }
        if (inspectOnly && selectedAction.value) result = { ...result, action: result.actions.find(a => a.action_id === selectedAction.value) ?? null }
        else if (actionRequest) result = { ...result, action: result.actions.find(a => a.request_id === actionRequest) ?? null }
        evidence.value = result; stale.value = false
        clearTimeout(freshnessTimer); freshnessTimer = setTimeout(() => { stale.value = true }, 15_000)
        if (result.action?.state === 'unknown' || result.action?.state === 'failed') { state.value = 'Action outcome is unknown or incomplete. Inspect retained observations; no automatic republication.'; busy.value = false; return }
        if (complete(result)) {
          state.value = scenario === 'normal' ? 'Confirmed: receipt committed and stock 15; relay completion observed.' : scenario === 'recovery' ? 'Recovered: injected before-send rejection, durable retry and stock 15 observed.' : 'Verified: Duplicate processing observed; one effect receipt and stock 15.'
          busy.value = false; return
        }
        if (result.outbox.status === 'dead_letter') { state.value = 'Relay reached a terminal state. Consumer effects are independent; inspect evidence.'; busy.value = false; return }
        state.value = inspectOnly && actionRequest && !result.action ? 'Action request is not visible in this snapshot. Its transaction may still commit; inspection does not republish.' : result.action ? 'Original effect committed. Observing bounded duplicate action…' : 'Producer committed. Observing independent delivery evidence…'
        timer = setTimeout(() => void step(false), 1000)
      } catch (error) {
        if (mine !== generation) return
        stale.value = true; busy.value = false
        state.value = error instanceof Error && error.name !== 'AbortError' ? error.message : 'Observation request timed out. Submission outcome is unknown; recover the same request identity.'
      } finally { clearTimeout(timeout) }
    }
    await step(!inspectOnly && !selectedRequest.value)
  }
  function selectionChanged() {
    stop(); stale.value = !!evidence.value
    state.value = 'Selection changed. Inspect to refresh; displayed evidence belongs to its labeled Run ID.'
  }
  async function inspect() {
    if (!selectedRequest.value && !requestId.value) return
    await execute(true)
  }
  async function loadHistory(offset = 0) {
    stop(); historyBusy.value = true; state.value = 'Loading retained runs…'
    const mine = generation, active = new AbortController(); controller = active
    const timeout = setTimeout(() => active.abort(), 9000)
    try {
      const rows = await call<typeof history.value>(`/demo/history?limit=10&offset=${offset}`, active.signal)
      if (mine === generation) { history.value = rows; historyOffset.value = offset; state.value = 'Retained runs loaded. Select a run and inspect; no work is submitted.' }
    } catch (error) { if (mine === generation) state.value = error instanceof Error ? error.message : 'History unavailable' }
    finally { clearTimeout(timeout); historyBusy.value = false }
  }
  function anotherAction() {
    if (!evidence.value || busy.value) return
    selectedRequest.value = evidence.value.request_id
    sessionStorage.setItem(`relay-action:${evidence.value.run_id}`, crypto.randomUUID())
    void execute()
  }
  async function loadRuns() {
    const mine = generation, active = new AbortController(); controller = active
    const timeout = setTimeout(() => active.abort(), 9000)
    try { const runs = await call<typeof eligibleRuns.value>('/demo/runs', active.signal); if (mine === generation) eligibleRuns.value = runs }
    catch (error) { if (mine === generation) state.value = error instanceof Error ? error.message : 'Runs unavailable' }
    finally { clearTimeout(timeout) }
  }
  function newRun() {
    if (!evidence.value || !complete(evidence.value)) return
    stop(); selectedAction.value = ''; selectedRequest.value = ''; sessionStorage.removeItem(`${storageKey}:selected`); sessionStorage.removeItem(storageKey); requestId.value = null; evidence.value = null
    state.value = 'Ready for a new real execution'; stale.value = false
  }
  onBeforeUnmount(stop)
  return { selectionChanged, inspect, loadHistory, history, historyOffset, historyBusy, selectedAction, anotherAction, eligibleRuns, selectedRequest, loadRuns, token, requestId, evidence, location, state, busy, stale, execute, newRun, complete }
}
