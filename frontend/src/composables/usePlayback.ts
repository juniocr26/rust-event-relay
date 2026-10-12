import { computed, onBeforeUnmount, ref } from 'vue'
import type { Scenario } from '../scenarios/types'
export function usePlayback(scenario: Scenario, duration = 3600) {
  const index = ref(-1),
    running = ref(false)
  let timer: ReturnType<typeof setTimeout> | undefined
  function cancel() {
    if (timer !== undefined) clearTimeout(timer)
    timer = undefined
    running.value = false
  }
  function advance() {
    if (index.value < scenario.steps.length - 1) {
      index.value++
      timer = setTimeout(advance, duration)
    } else {
      timer = undefined
      running.value = false
    }
  }
  function run() {
    cancel()
    index.value = -1
    running.value = true
    advance()
  }
  onBeforeUnmount(cancel)
  return {
    index,
    running,
    run,
    cancel,
    current: computed(() => scenario.steps[index.value]),
    entries: computed(() => scenario.steps.slice(0, index.value + 1)),
    finished: computed(() => index.value === scenario.steps.length - 1 && !running.value),
  }
}
