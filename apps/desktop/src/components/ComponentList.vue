<script setup lang="ts">
import { Badge } from '@/components/ui/badge'
import type { Component } from '@/lib/api'

defineProps<{
  items: { component: Component; plugin: string }[]
  /** Shown when there are no items. */
  empty: string
}>()
</script>

<template>
  <p v-if="items.length === 0" class="text-muted-foreground py-6 text-center">{{ empty }}</p>
  <ul v-else class="divide-y rounded-xl border">
    <li
      v-for="({ component, plugin }, index) in items"
      :key="`${index}/${plugin}/${component.name}`"
      class="flex flex-col gap-1 px-4 py-3"
    >
      <div class="flex items-center justify-between gap-3">
        <span class="truncate font-mono text-sm font-medium" :title="component.path ?? undefined">
          {{ component.name }}
        </span>
        <Badge variant="outline" class="shrink-0">{{ plugin }}</Badge>
      </div>
      <p v-if="component.description" class="text-muted-foreground line-clamp-2">
        {{ component.description }}
      </p>
    </li>
  </ul>
</template>
