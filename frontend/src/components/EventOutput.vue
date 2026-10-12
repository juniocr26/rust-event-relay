<script setup lang="ts">
import type { Step } from '../scenarios/types'
defineProps<{ entries: Step[] }>()
</script>
<template>
  <section class="terminal" aria-label="Event output">
    <div class="terminal-head">Event output · simulated event time</div>
    <div role="log" aria-live="polite" aria-relevant="additions">
      <p v-if="!entries.length">Waiting to start…</p>
      <div v-for="(entry, i) in entries" :key="i" class="output-line">
        <span class="trace-time">[{{ entry.time }}]</span> {{ entry.trace }}
        <div class="trace-detail">{{ entry.explanation }}</div>
      </div>
    </div>
  </section>
</template>
