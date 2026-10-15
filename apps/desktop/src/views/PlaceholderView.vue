<script setup lang="ts">
defineProps<{
  title: string
  /** What the page will do. */
  description?: string
}>()
</script>

<template>
  <div class="mx-auto flex w-full max-w-3xl flex-col gap-2 p-8">
    <h1 class="text-2xl font-semibold">{{ title }}</h1>
    <p v-if="description" class="text-muted-foreground">{{ description }}</p>
    <p class="text-muted-foreground text-sm">Not built yet.</p>
  </div>
</template>
