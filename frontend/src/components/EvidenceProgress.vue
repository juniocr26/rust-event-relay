<script setup lang="ts">
defineProps<{ producer: boolean; publication: boolean; receipt: boolean; stock: boolean }>()
</script>
<template>
  <ol class="stages" aria-label="Independent durable evidence">
    <li v-for="stage in [
      { label: 'Producer commit', observed: producer },
      { label: 'Relay completion', observed: publication },
      { label: 'Consumer receipt', observed: receipt },
      { label: 'Stock 15', observed: stock },
    ]" :key="stage.label" class="stage" :class="stage.observed ? 'done' : 'pending'">
      <strong>{{ stage.label }}</strong><small>{{ stage.observed ? '✓ Observed' : '○ Not observed' }}</small>
    </li>
  </ol>
</template>
