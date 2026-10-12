<script setup lang="ts">
import { computed, ref } from 'vue'
import ScenarioLauncher from './components/ScenarioLauncher.vue'
import ScenarioExperience from './components/ScenarioExperience.vue'
import { scenarios } from './scenarios'
const selected = ref('normal'),
  open = ref(false)
const scenario = computed(() => scenarios.find((s) => s.id === selected.value)!)
</script>
<template>
  <ScenarioLauncher v-model="selected" @open="open = true" /><ScenarioExperience
    v-if="open"
    :scenario="scenario"
    @close="open = false"
  />
</template>
