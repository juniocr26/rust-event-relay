<script setup lang="ts">
defineProps<{ active: boolean }>()
</script>
<template>
  <div class="pulse" :class="{ active }" aria-hidden="true">
    <span
      v-for="(height, i) in [3, 6, 10, 23, 34, 19, 12, 7, 4, 3, 3]"
      :key="i"
      :style="{ height: `${active ? height : 3}px` }"
    />
  </div>
  <div class="caption">Illustrative activity · not RabbitMQ metrics</div>
</template>
