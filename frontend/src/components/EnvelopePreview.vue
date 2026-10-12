<script setup lang="ts">
defineProps<{ output: string; broker: string; durable: string }>()
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
      ><code>{ event_id: "demo-001", payload: { order_id: "order-demo" } }</code>
    </div>
    <div>
      <strong>Output</strong><code>{{ output }}</code>
    </div>
  </section>
</template>
