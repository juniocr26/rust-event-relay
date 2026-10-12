<script setup lang="ts">
withDefaults(defineProps<{ secondary?: boolean; disabled?: boolean }>(), {
  secondary: false,
  disabled: false,
})
defineEmits<{ click: [event: MouseEvent] }>()
</script>
<template>
  <button
    type="button"
    :class="['button', { secondary }]"
    :disabled="disabled"
    @click="$emit('click', $event)"
  >
    <slot />
  </button>
</template>
