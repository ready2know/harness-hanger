<script setup lang="ts">
import { computed, watch } from 'vue'
import { RouterLink } from 'vue-router'

import CatalogEmpty from '@/components/CatalogEmpty.vue'
import CatalogFilters from '@/components/CatalogFilters.vue'
import PluginStatusBadge from '@/components/PluginStatusBadge.vue'
import UnreadableNotice from '@/components/UnreadableNotice.vue'
import { useCatalogFilter, useCatalogPlugins } from '@/composables/useCatalogPlugins'
import { componentRoute, pluginRoute } from '@/lib/routes'

/** Skills and agents load through their plugin, so each one shows its plugin's status. */
const props = defineProps<{ kind: 'skills' | 'agents' }>()

const { plugins, unreadable, error } = useCatalogPlugins()

const rows = computed(() =>
  (plugins.value ?? [])
    .flatMap((entry) =>
      entry.plugin[props.kind].map((component) => ({
        component,
        to: componentRoute(
          props.kind === 'skills' ? 'skill' : 'agent',
          entry.marketplaceId,
          entry.plugin.name,
          component.name,
        ),
        plugin: entry.plugin.name,
        standalone: entry.plugin.source.kind === 'harness',
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
        The {{ kind }} in your catalog. Claude Code loads a plugin's {{ kind }} when the plugin is
        enabled for your user, and standalone {{ kind }} always.
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
          class="hover:bg-muted/50 relative flex flex-col gap-1 px-4 py-3"
        >
          <div class="flex items-center justify-between gap-3">
            <!-- The link covers the whole row; the links and badge below sit above it. -->
            <RouterLink
              :to="row.to"
              class="truncate font-mono text-sm font-medium after:absolute after:inset-0"
              :title="row.component.path ?? undefined"
            >
              {{ row.component.name }}
            </RouterLink>
            <PluginStatusBadge :status="row.status" class="relative" />
          </div>
          <p v-if="row.component.description" class="text-muted-foreground line-clamp-2">
            {{ row.component.description }}
          </p>
          <p class="text-muted-foreground text-xs">
            {{ row.standalone ? 'Standalone in' : '' }}
            <template v-if="!row.standalone">
              <RouterLink
                :to="pluginRoute(row.marketplaceId, row.plugin)"
                class="hover:text-foreground relative underline-offset-4 hover:underline"
              >
                {{ row.plugin }}
              </RouterLink>
              ·
            </template>
            <RouterLink
              :to="{ name: 'marketplace-preview', params: { id: row.marketplaceId } }"
              class="hover:text-foreground relative underline-offset-4 hover:underline"
            >
              {{ row.marketplace }}
            </RouterLink>
          </p>
        </li>
      </ul>
    </template>
  </div>
</template>
