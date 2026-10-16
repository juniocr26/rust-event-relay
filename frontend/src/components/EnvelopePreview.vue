<script setup lang="ts">
defineProps<{ output: string; broker: string; durable: string; inventory?: boolean }>()
</script>
<template>
  <dl class="evidence">
    <div>
      <dt>Broker acceptance</dt>
      <dd>{{ broker }}</dd>
    </div>
    <div>
      <dt>Durable database state</dt>
      <dd>{{ durable }}</dd>
    </div>
    <div>
      <dt>Stable event identity</dt>
      <dd>demo-001</dd>
    </div>
  </dl>
  <section class="envelope" aria-label="Input and output envelope">
    <div>
      <strong>Input</strong
      ><code>{{ inventory ? '{ event_id: "demo-001", payload: { warehouse_id: "local", sku: "SKU-1", quantity: 5 } }' : '{ event_id: "demo-001", payload: { order_id: "order-demo" } }' }}</code>
    </div>
    <div>
      <strong>Output</strong><code>{{ output }}</code>
    </div>
  </section>
</template>
