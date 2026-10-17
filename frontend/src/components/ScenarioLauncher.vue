<script setup lang="ts">
import BaseSelect from './BaseSelect.vue'
import BaseButton from './BaseButton.vue'
import { scenarios } from '../scenarios'
defineProps<{ modelValue: string }>()
defineEmits<{ 'update:modelValue': [value: string]; open: [] }>()
</script>
<template>
  <main class="landing">
    <div class="art" aria-hidden="true">
      <div class="orbit" />
      <div class="orbit two" />
      <div class="orbit three" />
      <div class="packet" />
      <span class="art-label">One event. A reliable journey.</span>
    </div>
    <div class="brand">Reliable Event Relay</div>
    <section class="intro">
      <span class="eyebrow">An interactive journey</span>
      <h1>See what happens<br />when things go wrong.</h1>
      <p>Follow a message through delivery and recovery. One scenario, one step at a time.</p>
      <BaseSelect
        id="scenario"
        label="Choose an experience"
        :model-value="modelValue"
        :options="scenarios"
        @update:model-value="$emit('update:modelValue', $event)"
      /><BaseButton @click="$emit('open')">Open experience →</BaseButton>
      <div class="caption">{{ ['normal','dedupe','concurrent','recovery'].includes(modelValue) ? 'Real execution · persistent evidence' : 'Illustrative scenario · no real fault execution' }}</div>
    </section>
  </main>
</template>
