<script setup lang="ts">
import { RouterLink, type RouteLocationRaw } from 'vue-router'

import PagedList from '@/components/PagedList.vue'
import { Badge } from '@/components/ui/badge'
import type { Component } from '@/lib/api'
import type { PageSizeSetting } from '@/lib/pageSizes'

defineProps<{
  /** `plugin` is shown as a badge. `to` is the component's page, when it has one. */
  items: { component: Component; plugin?: string; to?: RouteLocationRaw }[]
  /** Shown when there are no items. */
  empty: string
  /** Which page size the list shares with others. The catalog's, unless given. */
  pageSize?: PageSizeSetting
}>()
</script>

<template>
  <p v-if="items.length === 0" class="text-muted-foreground py-6 text-center">{{ empty }}</p>
  <PagedList v-else v-slot="{ items: visible }" :items="items" :setting="pageSize">
    <ul class="divide-y rounded-xl border">
      <li
        v-for="({ component, plugin, to }, index) in visible"
        :key="`${index}/${plugin}/${component.name}`"
        :class="['relative flex flex-col gap-1 px-4 py-3', to && 'hover:bg-muted/50']"
      >
        <div class="flex items-center justify-between gap-3">
          <!-- The link covers the whole row. -->
          <RouterLink
            v-if="to"
            :to="to"
            class="truncate font-mono text-sm font-medium after:absolute after:inset-0"
            :title="component.path ?? undefined"
          >
            {{ component.name }}
          </RouterLink>
          <span
            v-else
            class="truncate font-mono text-sm font-medium"
            :title="component.path ?? undefined"
          >
            {{ component.name }}
          </span>
          <Badge v-if="plugin" variant="outline" class="shrink-0">{{ plugin }}</Badge>
        </div>
        <p v-if="component.description" class="text-muted-foreground line-clamp-2">
          {{ component.description }}
        </p>
      </li>
    </ul>
  </PagedList>
</template>
