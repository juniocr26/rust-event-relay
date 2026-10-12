<script setup lang="ts">
import type { Step } from '../scenarios/types'
defineProps<{ steps: Step[]; index: number; running: boolean }>()
</script>
<template>
  <ol class="stages" aria-label="Scenario progress">
    <li
      v-for="(step, i) in steps"
      :key="i"
      :class="[
        'stage',
        i < index || (i === index && !running) ? step.state : i === index ? 'active' : 'pending',
      ]"
      :aria-current="i === index && running ? 'step' : undefined"
    >
      <span>{{ String(i + 1).padStart(2, '0') }}</span
      ><strong>{{ step.label }}</strong
      ><small>{{
        i < index || (i === index && !running)
          ? {
              done: '✓ Completed',
              interrupted: '! Interrupted',
              uncertain: '? Uncertain',
              skipped: '− Skipped',
              preview: '◇ Future preview',
              pending: 'Pending',
              active: 'Running',
            }[step.state]
          : i === index
            ? '● Running'
            : '○ Waiting'
      }}</small>
    </li>
  </ol>
</template>
