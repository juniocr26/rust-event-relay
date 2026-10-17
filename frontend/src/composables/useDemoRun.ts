import { ref, onBeforeUnmount } from 'vue'
export interface Evidence {
  run_id: string; request_id: string; event_id: string; warehouse_id: string; sku: string
  scenario: 'normal'; initial_stock: number; quantity: number; accepted_at: string; fact_id: string
  outbox: { status: string; created_at: string; processed_at: string | null; attempt_count: number; available_at: string }
  receipt: { consumer_name: string; event_id: string; quantity: number; stock_before: number; stock_after: number; completed_at: string } | null
  stock: number | null; observed_at: string
}
export function useDemoRun() {
  const token = ref(''), requestId = ref<string | null>(sessionStorage.getItem('demo61-request')), evidence = ref<Evidence | null>(null)
  const state = ref('Ready for real execution'), busy = ref(false), stale = ref(false)
  let controller: AbortController | undefined, timer: ReturnType<typeof setTimeout> | undefined, generation = 0
  const stop = () => { generation++; controller?.abort(); clearTimeout(timer); busy.value = false }
  async function call(method: 'GET' | 'POST', signal: AbortSignal): Promise<Evidence | null> {
    const response = await fetch(method === 'POST' ? '/demo/runs' : `/demo/runs/${requestId.value}`, {
      method, signal, headers: { Authorization: `Bearer ${token.value}`, 'X-Demo-Client': 'demo61', ...(method === 'POST' ? { 'Content-Type': 'application/json' } : {}) },
      ...(method === 'POST' ? { body: JSON.stringify({ request_id: requestId.value, scenario: 'normal', initial_stock: 10, quantity: 5 }) } : {}),
    })
    if (response.status === 404 && method === 'GET') return null
    if (!response.ok) throw new Error(response.status === 401 ? 'Authorization required. Check the demo token.' : response.status === 422 ? 'Producer transaction rejected. No new producer commit was accepted.' : response.status === 409 ? 'Request identity conflicts with the submitted intent.' : response.status === 429 ? 'Demo budget exhausted or read rate exceeded.' : `Observation unavailable (HTTP ${response.status}). Submission outcome may be unknown.`)
    return response.json() as Promise<Evidence>
  }
  async function execute() {
    stop(); const mine = generation; busy.value = true
    // Identity survives all uncertain submissions across close/reopen and reload in this browser tab.
    requestId.value ??= crypto.randomUUID()
    sessionStorage.setItem('demo61-request', requestId.value)
    const deadline = Date.now() + 60_000
    async function step(submit: boolean) {
      if (mine !== generation) return
      if (Date.now() >= deadline) { state.value = 'Observation timed out. The durable run may still complete.'; stale.value = true; busy.value = false; return }
      const activeController = new AbortController(); controller = activeController
      const requestTimeout = setTimeout(() => activeController.abort(), Math.min(9000, Math.max(1, deadline - Date.now())))
      try {
        const result = await call(submit ? 'POST' : 'GET', activeController.signal)
        if (mine !== generation) return
        stale.value = false
        if (result) evidence.value = result
        if (result?.receipt && result.stock === 15 && result.receipt.stock_before === 10 && result.receipt.stock_after === 15 && result.outbox.status === 'processed') {
          state.value = 'Confirmed: receipt committed and stock 15; relay completion observed.'; busy.value = false; return
        }
        if (result?.outbox.status === 'dead_letter') { state.value = 'Relay reached a terminal state. Inspect retained evidence; consumer effects are independent.'; busy.value = false; return }
        state.value = result ? 'Producer committed. Observing independent delivery evidence…' : 'No committed request observed. Retry the same submission identity.'
        if (!result) { busy.value = false; return }
        if (Date.now() >= deadline) { state.value = 'Observation timed out. The durable run may still complete.'; stale.value = true; busy.value = false; return }
        timer = setTimeout(() => void step(false), 1000)
      } catch (error) {
        if (mine !== generation) return
        stale.value = true; busy.value = false
        state.value = error instanceof Error && error.name !== 'AbortError' ? error.message : 'Observation request timed out. Submission outcome is unknown; recover the same request identity.'
      } finally { clearTimeout(requestTimeout) }
    }
    // Repeat POST is safe: durable reconciliation returns the original run.
    await step(true)
  }
  function newRun() {
    if (!evidence.value?.receipt || evidence.value.stock !== 15 || evidence.value.outbox.status !== 'processed') return
    stop(); sessionStorage.removeItem('demo61-request'); requestId.value = null; evidence.value = null
    state.value = 'Ready for a new real execution'; stale.value = false
  }
  onBeforeUnmount(stop)
  return { token, requestId, evidence, state, busy, stale, execute, newRun }
}
