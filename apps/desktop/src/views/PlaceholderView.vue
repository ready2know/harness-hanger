<script setup lang="ts">
defineProps<{ title: string }>()
</script>

<template>
  <div class="mx-auto flex w-full max-w-3xl flex-col gap-6 p-8">
    <h1 class="text-2xl font-semibold">{{ title }}</h1>
    <p class="text-muted-foreground">Not built yet.</p>
  </div>
</template>
