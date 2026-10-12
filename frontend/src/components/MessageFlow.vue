<script setup lang="ts">
import ActivityPulse from './ActivityPulse.vue'
defineProps<{ active: boolean; title: string; explanation: string; preview: boolean }>()
</script>
<template>
  <section class="visual" aria-label="Illustrative message journey">
    <div class="flow">
      <div class="station">{{ preview ? 'RabbitMQ arrivals' : 'PostgreSQL' }}</div>
      <span class="message" :class="{ moving: active }" aria-hidden="true">→</span>
      <div class="station">{{ preview ? 'Future consumer' : 'RabbitMQ' }}</div>
      <span class="message" :class="{ moving: active }" aria-hidden="true">→</span>
      <div class="station">Observed outcome</div>
    </div>
    <div class="current" role="status">{{ title }}</div>
    <div class="hint">{{ explanation }}</div>
    <ActivityPulse :active="active" />
  </section>
</template>
