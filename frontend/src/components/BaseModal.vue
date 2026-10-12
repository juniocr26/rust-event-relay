<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref } from 'vue'
import BaseButton from './BaseButton.vue'
defineProps<{ title: string; description: string }>()
const emit = defineEmits<{ close: [] }>()
const dialog = ref<HTMLDialogElement>()
let previous: HTMLElement | null = null
let overflow = ''
onMounted(() => {
  previous = document.activeElement as HTMLElement
  overflow = document.body.style.overflow
  document.body.style.overflow = 'hidden'
  dialog.value?.showModal()
  dialog.value?.querySelector<HTMLButtonElement>('button')?.focus()
})
onBeforeUnmount(() => {
  dialog.value?.close()
  document.body.style.overflow = overflow
  previous?.focus({ preventScroll: true })
})
function contain(event: KeyboardEvent) {
  if (event.key !== 'Tab') return
  const items = [
    ...dialog.value!.querySelectorAll<HTMLElement>(
      'button:not(:disabled), select:not(:disabled), [tabindex="0"]',
    ),
  ]
  const first = items[0],
    last = items[items.length - 1]
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault()
    last?.focus()
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault()
    first?.focus()
  }
}
</script>
<template>
  <dialog
    ref="dialog"
    class="modal"
    aria-modal="true"
    aria-labelledby="experience-title"
    aria-describedby="experience-description"
    @cancel.prevent="emit('close')"
    @keydown="contain"
  >
    <header class="modal-header">
      <span class="eyebrow">Scenario experience</span
      ><BaseButton secondary @click="emit('close')">Close</BaseButton>
    </header>
    <div class="modal-body">
      <h2 id="experience-title">{{ title }}</h2>
      <p id="experience-description">{{ description }}</p>
      <slot />
    </div>
  </dialog>
</template>
