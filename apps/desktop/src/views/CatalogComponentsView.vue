<script setup lang="ts">
import { computed, watch } from 'vue'
import { RouterLink } from 'vue-router'

import CatalogEmpty from '@/components/CatalogEmpty.vue'
import CatalogFilters from '@/components/CatalogFilters.vue'
import PluginStatusBadge from '@/components/PluginStatusBadge.vue'
import UnreadableNotice from '@/components/UnreadableNotice.vue'
import { useCatalogFilter, useCatalogPlugins } from '@/composables/useCatalogPlugins'

/** Skills and agents load through their plugin, so each one shows its plugin's status. */
const props = defineProps<{ kind: 'skills' | 'agents' }>()

const { plugins, unreadable, error } = useCatalogPlugins()

const rows = computed(() =>
  (plugins.value ?? [])
    .flatMap((entry) =>
      entry.plugin[props.kind].map((component) => ({
        component,
        plugin: entry.plugin.name,
        marketplace: entry.marketplace,
        marketplaceId: entry.marketplaceId,
        status: entry.status,
      })),
    )
    .sort((a, b) => a.component.name.localeCompare(b.component.name)),
)
const { query, status, counts, shown, reset } = useCatalogFilter(rows, (row) => [
  row.component.name,
  row.component.description,
  row.plugin,
  row.marketplace,
])

// The router reuses this view between Skills and Agents.
watch(() => props.kind, reset)

const title = computed(() => (props.kind === 'skills' ? 'Skills' : 'Agents'))
</script>

<template>
  <div class="mx-auto flex w-full max-w-4xl flex-col gap-6 p-8">
    <header class="flex flex-col gap-1">
      <h1 class="text-2xl font-semibold">{{ title }}</h1>
      <p class="text-muted-foreground text-sm">
        The {{ kind }} in the marketplaces you added. Claude Code loads one when its plugin is
        enabled for your user.
      </p>
    </header>

    <p v-if="error" role="alert" class="text-destructive wrap-anywhere">{{ error }}</p>
    <p v-else-if="!plugins" role="status" class="text-muted-foreground">Loading…</p>
    <template v-else>
      <UnreadableNotice v-if="unreadable.length" :marketplaces="unreadable" :noun="kind" />
      <CatalogEmpty v-if="plugins.length === 0 && unreadable.length === 0" :noun="kind" />
      <p
        v-else-if="plugins.length > 0 && rows.length === 0"
        class="text-muted-foreground py-6 text-center"
      >
        The plugins in your marketplaces include no {{ kind }}. Plugins installed from elsewhere are
        not counted, because their contents are unknown.
      </p>
    </template>

    <template v-if="rows.length">
      <CatalogFilters v-model:query="query" v-model:status="status" :counts="counts" />
      <p v-if="shown.length === 0" class="text-muted-foreground py-6 text-center">
        No {{ kind }} match.
      </p>
      <ul v-else class="divide-y rounded-xl border">
        <li
          v-for="(row, index) in shown"
          :key="`${index}/${row.marketplaceId}/${row.plugin}/${row.component.name}`"
          class="flex flex-col gap-1 px-4 py-3"
        >
          <div class="flex items-center justify-between gap-3">
            <span
              class="truncate font-mono text-sm font-medium"
              :title="row.component.path ?? undefined"
            >
              {{ row.component.name }}
            </span>
            <PluginStatusBadge :status="row.status" />
          </div>
          <p v-if="row.component.description" class="text-muted-foreground line-clamp-2">
            {{ row.component.description }}
          </p>
          <p class="text-muted-foreground text-xs">
            {{ row.plugin }} ·
            <RouterLink
              :to="{ name: 'marketplace-preview', params: { id: row.marketplaceId } }"
              class="hover:text-foreground underline-offset-4 hover:underline"
            >
              {{ row.marketplace }}
            </RouterLink>
          </p>
        </li>
      </ul>
    </template>
  </div>
</template>
