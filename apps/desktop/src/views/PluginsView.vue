<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'

import CatalogEmpty from '@/components/CatalogEmpty.vue'
import CatalogFilters from '@/components/CatalogFilters.vue'
import PluginStatusBadge from '@/components/PluginStatusBadge.vue'
import UnreadableNotice from '@/components/UnreadableNotice.vue'
import { Badge } from '@/components/ui/badge'
import { useCatalogFilter, useCatalogPlugins } from '@/composables/useCatalogPlugins'
import { pluginContents } from '@/lib/format'

const { plugins, unreadable, error } = useCatalogPlugins()

// A harness's own skills, agents, and commands are not a plugin; the Skills and Agents pages show
// them.
const rows = computed(() =>
  (plugins.value ?? [])
    .filter((entry) => entry.plugin.source.kind !== 'harness')
    .sort(
      (a, b) =>
        a.plugin.name.localeCompare(b.plugin.name) || a.marketplace.localeCompare(b.marketplace),
    ),
)
const { query, status, counts, shown } = useCatalogFilter(rows, (entry) => [
  entry.plugin.name,
  entry.plugin.description,
  entry.plugin.category,
  entry.marketplace,
])
</script>

<template>
  <div class="mx-auto flex w-full max-w-4xl flex-col gap-6 p-8">
    <header class="flex flex-col gap-1">
      <h1 class="text-2xl font-semibold">Plugins</h1>
      <p class="text-muted-foreground text-sm">
        The plugins of the marketplaces you added, and whether Claude Code has each one enabled for
        your user.
      </p>
    </header>

    <p v-if="error" role="alert" class="text-destructive wrap-anywhere">{{ error }}</p>
    <p v-else-if="!plugins" role="status" class="text-muted-foreground">Loading…</p>
    <template v-else>
      <UnreadableNotice v-if="unreadable.length" :marketplaces="unreadable" noun="plugins" />
      <CatalogEmpty v-if="plugins.length === 0 && unreadable.length === 0" noun="plugins" />
    </template>

    <template v-if="plugins?.length">
      <CatalogFilters v-model:query="query" v-model:status="status" :counts="counts" />
      <p v-if="shown.length === 0" class="text-muted-foreground py-6 text-center">
        No plugins match.
      </p>
      <ul v-else class="divide-y rounded-xl border">
        <li
          v-for="(entry, index) in shown"
          :key="`${index}/${entry.marketplaceId}/${entry.plugin.name}`"
          class="flex flex-col gap-1 px-4 py-3"
        >
          <div class="flex items-center justify-between gap-3">
            <div class="flex min-w-0 flex-wrap items-center gap-2">
              <span class="font-medium">{{ entry.plugin.name }}</span>
              <Badge v-if="entry.plugin.version" variant="secondary">
                {{ entry.plugin.version }}
              </Badge>
            </div>
            <PluginStatusBadge :status="entry.status" />
          </div>
          <p v-if="entry.plugin.description" class="text-muted-foreground line-clamp-2">
            {{ entry.plugin.description }}
          </p>
          <p class="text-muted-foreground text-xs">
            <RouterLink
              :to="{ name: 'marketplace-preview', params: { id: entry.marketplaceId } }"
              class="hover:text-foreground underline-offset-4 hover:underline"
            >
              {{ entry.marketplace }}
            </RouterLink>
            · {{ pluginContents(entry.plugin) }}
          </p>
        </li>
      </ul>
    </template>
  </div>
</template>
