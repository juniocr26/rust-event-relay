<script setup lang="ts">
import { ref } from 'vue'
import { useDemoRun } from '../composables/useDemoRun'
import BaseButton from './BaseButton.vue'
import MessageFlow from './MessageFlow.vue'
import EvidenceProgress from './EvidenceProgress.vue'
const props = defineProps<{ scenario: string }>()
const { anotherAction, eligibleRuns, selectedRequest, loadRuns, token, requestId, evidence, location, state, busy, stale, execute, newRun, complete } = useDemoRun(props.scenario)
const copied = ref('')
async function copy(label: string, value: string) {
  try { await navigator.clipboard.writeText(value); copied.value = `${label} copied` }
  catch { copied.value = 'Copy unavailable. Select and copy the identifier text.' }
}
</script>
<template>
  <p class="expected">Real local pipeline · initial stock 10 + received units 5 → expected stock 15</p>
  <label for="demo-token">Local demo token</label>
  <input id="demo-token" v-model="token" type="password" autocomplete="off" spellcheck="false" :disabled="busy" aria-describedby="token-help" />
  <p id="token-help">Use DEMO_TOKEN from your private .env. It authorizes scenario execution and evidence in this application.</p>
  <fieldset v-if="scenario === 'dedupe'"><legend>Duplicate an existing successful run</legend>
    <p>Republish the original event. The consumer should verify Duplicate and retain one business effect.</p>
    <BaseButton secondary :disabled="busy || token.length !== 64" @click="loadRuns">Load successful runs</BaseButton>
    <label for="duplicate-run">Successful run</label>
    <select id="duplicate-run" v-model="selectedRequest" :disabled="busy">
      <option value="">Create a new example (10 + 5)</option>
      <option v-for="run in eligibleRuns" :key="run.run_id" :value="run.request_id">{{ run.run_id }}</option>
    </select>
  </fieldset>
  <EvidenceProgress :producer="!!evidence" :publication="evidence?.outbox.status === 'processed'" :receipt="!!evidence?.receipt" :stock="evidence?.stock === 15" />
  <MessageFlow live :preview="false" :active="false" title="Persistent delivery evidence" explanation="Each milestone reflects durable database evidence. Diagram arrows do not measure broker activity." />
  <p role="status">{{ state }} <span v-if="stale">{{ evidence ? 'Last evidence is stale.' : 'No fresh evidence is available.' }}</span></p>
  <p v-if="requestId">Producer request: <code>{{ requestId }}</code> <BaseButton secondary class="copy-control" @click="copy('Request ID', requestId)">Copy request ID</BaseButton></p>
  <template v-if="evidence">
    <p>Run ID: <code>{{ evidence.run_id }}</code> <BaseButton secondary class="copy-control" @click="copy('Run ID', evidence.run_id)">Copy Run ID</BaseButton></p>
    <p>Event ID: <code>{{ evidence.event_id }}</code> <BaseButton secondary class="copy-control" @click="copy('Event ID', evidence.event_id)">Copy Event ID</BaseButton></p>
    <details open><summary>Input and producer transaction</summary>
      <p>Committed: stock fixture {{ evidence.initial_stock }}, received units {{ evidence.quantity }}. The consumer applies the increment.</p>
      <p>Warehouse: <code>{{ evidence.warehouse_id }}</code><br />SKU: <code>{{ evidence.sku }}</code></p>
      <p>Producer fact: <code>{{ evidence.fact_id }}</code> · accepted at {{ evidence.accepted_at }}</p>
    </details>
    <details open><summary>Independent observed results</summary>
      <p>Outbox: {{ evidence.outbox.status }} · attempts {{ evidence.outbox.attempt_count }} · completion {{ evidence.outbox.processed_at ?? 'not observed' }}</p>
      <p v-if="evidence.receipt">Consumer receipt: {{ evidence.receipt.consumer_name }} · {{ evidence.receipt.completed_at }}<br />Receipt effect: {{ evidence.receipt.stock_before }} → {{ evidence.receipt.stock_after }}</p>
      <p v-else>Consumer receipt: not observed yet.</p>
      <p v-if="!stale && evidence.stock === 15 && evidence.effect_receipts === 1 && evidence.action && evidence.observations.some(o => o.action_id === evidence?.action?.action_id && o.outcome === 'Duplicate')">Stock remains 15 · One business effect</p>
      <p>Effect receipts: {{ evidence.effect_receipts }}</p>
      <p>Current stock: {{ evidence.stock ?? 'unavailable' }} · observed at {{ evidence.observed_at }}</p>
      <p>Consumer classifications observed: Applied {{ evidence.observations.filter(o => o.outcome === 'Applied').length }} · Duplicate {{ evidence.observations.filter(o => o.outcome === 'Duplicate').length }}<span v-if="evidence.observations.length >= 32"> · observation cap reached; counts are lower bounds</span>.</p>
      <p>Broker confirmations recorded: {{ evidence.publications.filter(p => p.outcome === 'confirmed').length }}. Local observation times are not exact broker timestamps.</p>
      <template v-if="evidence.action">
        <p>Action ID: <code>{{ evidence.action.action_id }}</code> <BaseButton secondary class="copy-control" @click="copy('Action ID', evidence.action.action_id)">Copy Action ID</BaseButton></p>
        <p>Action request: {{ evidence.action.request_id }} · state {{ evidence.action.state }} · bounded publication calls {{ evidence.action.copies }} · queue <code>{{ evidence.action.queue_name }}</code></p>
        <p>Observed action contexts: {{ [...new Set(evidence.observations.filter(o => o.action_id === evidence?.action?.action_id).map(o => o.context))].join(', ') || 'not observed' }}. Arrival count and order are not guaranteed.</p>
      </template>
      <p v-if="evidence.fault">Controlled condition: {{ evidence.fault.condition }} · injected {{ evidence.fault.injected_at ?? 'not observed' }} · retry due {{ evidence.fault.retry_available_at ?? 'not observed' }} · retry revision {{ evidence.fault.retry_revision ?? 'not observed' }}. This is not an infrastructure outage.</p>
      <details><summary>Recorded observations</summary><ul><li v-for="o in evidence.observations" :key="o.observation_id">{{ o.outcome }} · action {{ o.action_id ?? 'original delivery' }} · {{ o.context }} · {{ o.observed_at }}</li></ul></details>
      <p v-if="evidence.action">Action publication: {{ evidence.publications.filter(p => p.action_id === evidence?.action?.action_id).map(p => p.outcome).join(', ') || 'not observed' }} · correlated Duplicate observations: {{ evidence.observations.filter(o => o.action_id === evidence?.action?.action_id && o.outcome === 'Duplicate').length }}</p>
      <p>Publication and consumer completion are independent. Exact broker-confirm and ACK-receipt times are unavailable.</p>
    </details>
    <details><summary>Where to verify</summary>
      <p>PostgreSQL: {{ location?.database_host }}:{{ location?.database_port }} · database {{ location?.database }} · user {{ location?.database_user }}. Use your private .env credentials.</p>
      <p><a :href="location?.management_url" target="_blank" rel="noopener noreferrer">RabbitMQ management</a> · vhost {{ location?.vhost }} · exchange {{ location?.exchange }} → queue {{ location?.queue }} (routing key event). Queue statistics are aggregate.</p>
      <p>relay: publication, retry and replay state. consumer: processing receipts and inventory effect. public: migration metadata. demo: runs and producer business records.</p>
      <pre>SELECT * FROM demo.runs WHERE run_id = '{{ evidence.run_id }}';
SELECT * FROM demo.goods_receipts WHERE run_id = '{{ evidence.run_id }}';
SELECT id,status,attempt_count,processed_at FROM relay.outbox_events WHERE id = '{{ evidence.event_id }}';
SELECT consumer_name,event_id,stock_before,quantity,stock_after,completed_at FROM consumer.receipts WHERE event_id = '{{ evidence.event_id }}';
SELECT * FROM demo.actions WHERE run_id = '{{ evidence.run_id }}';
SELECT * FROM demo.consumer_observations WHERE run_id = '{{ evidence.run_id }}';
SELECT * FROM demo.publications WHERE run_id = '{{ evidence.run_id }}';
SELECT * FROM demo.faults WHERE run_id = '{{ evidence.run_id }}';
SELECT units FROM consumer.stock WHERE warehouse_id = '{{ evidence.warehouse_id }}' AND sku = '{{ evidence.sku }}';</pre>
    </details>
  </template>
  <p role="status">{{ copied }}</p>
  <footer class="experience-footer"><p>Closing stops browser observation. Durable execution and evidence remain. The request identity is retained in this tab for safe recovery; the token stays in memory.</p>
    <BaseButton :disabled="busy || token.length !== 64" @click="execute">{{ busy ? 'Observing…' : requestId ? 'Recover same request →' : 'Run real delivery →' }}</BaseButton>
    <BaseButton v-if="scenario === 'dedupe' && evidence?.action && !busy && evidence.actions.length < 3" secondary @click="anotherAction">Request another duplicate</BaseButton>
    <BaseButton v-if="evidence && complete(evidence) && !busy" secondary @click="newRun">Prepare a new run</BaseButton>
  </footer>
</template>
<style scoped>
input { display:block; width:min(100%,32rem); padding:.6rem; border:1px solid #5388c8; border-radius:.4rem; }
code, pre { overflow-wrap:anywhere; white-space:pre-wrap; }
details { margin-block:1rem; } summary { cursor:pointer; } button { cursor:pointer; } .copy-control { padding:.2rem .5rem; font-size:.8rem; margin-left:.4rem; } p { margin-block:.7rem; }
</style>
