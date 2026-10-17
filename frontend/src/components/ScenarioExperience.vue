<script setup lang="ts">
import type { Scenario } from '../scenarios/types'
import { usePlayback } from '../composables/usePlayback'
import BaseModal from './BaseModal.vue'
import BaseButton from './BaseButton.vue'
import StageProgress from './StageProgress.vue'
import MessageFlow from './MessageFlow.vue'
import EnvelopePreview from './EnvelopePreview.vue'
import EventOutput from './EventOutput.vue'
import RealDelivery from './RealDelivery.vue'
const props = defineProps<{ scenario: Scenario }>()
defineEmits<{ close: [] }>()
const { index, running, current, entries, finished, run } = usePlayback(props.scenario)
</script>
<template>
  <BaseModal :title="scenario.title" :description="scenario.description" @close="$emit('close')"
    ><RealDelivery v-if="['normal','dedupe','concurrent','recovery'].includes(scenario.id)" :scenario="scenario.id" /><template v-else><p class="expected">Illustrative scenario · Expected: {{ scenario.expected }}</p>
    <StageProgress :steps="scenario.steps" :index="index" :running="running" /><MessageFlow
      :active="running && !!current?.activity"
      :title="current?.title ?? 'Ready when you are.'"
      :explanation="current?.explanation ?? 'Run the scenario to follow each step.'"
      :preview="scenario.id === 'dedupe'"
    /><EnvelopePreview
      :broker="
        current?.broker ?? (scenario.id === 'dedupe' ? 'Illustration · waiting' : 'Not sent')
      "
      :durable="
        current?.durable ?? (scenario.id === 'dedupe' ? 'Illustration · waiting' : 'Pending')
      "
      :inventory="scenario.id === 'dedupe'"
      :output="current?.output ?? 'Waiting for playback'"
    /><EventOutput :entries="entries" />
    <footer class="experience-footer">
      <div class="result" role="status">
        <strong>{{
          finished ? scenario.outcome : running ? '● Playback in progress' : '○ Ready to start'
        }}</strong>
        <p>
          {{
            finished
              ? scenario.result
              : 'Local illustration. Playback pacing is separate from simulated event time.'
          }}
        </p>
      </div>
      <BaseButton :disabled="running" @click="run">{{
        running ? 'Running…' : finished ? 'Run again →' : 'Run scenario →'
      }}</BaseButton>
    </footer></template></BaseModal
  >
</template>
