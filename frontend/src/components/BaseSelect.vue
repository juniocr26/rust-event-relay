<script setup lang="ts">
defineProps<{
  modelValue: string
  options: { id: string; title: string }[]
  id: string
  label: string
}>()
defineEmits<{ 'update:modelValue': [value: string] }>()
</script>
<template>
  <label :for="id">{{ label }}</label
  ><select
    :id="id"
    :value="modelValue"
    @change="$emit('update:modelValue', ($event.target as HTMLSelectElement).value)"
  >
    <option v-for="option in options" :key="option.id" :value="option.id">
      {{ option.title }}
    </option>
  </select>
</template>
