import { computed, onMounted, ref, type Ref } from 'vue'

import { listCatalogPlugins, type CatalogPlugins, type PluginStatus } from '@/lib/api'

/** Loads every plugin in the catalog, with its status in Claude Code, when the component mounts. */
export function useCatalogPlugins() {
  const catalog = ref<CatalogPlugins>()
  const error = ref<string>()
  onMounted(async () => {
    try {
      catalog.value = await listCatalogPlugins()
    } catch (e) {
      error.value = String(e)
    }
  })
  /** `undefined` until loaded. */
  const plugins = computed(() => catalog.value?.plugins)
  const unreadable = computed(() => catalog.value?.unreadable ?? [])
  return { plugins, unreadable, error }
}

export type StatusFilter = PluginStatus | 'all'

/** Search and status filters over rows, with how many rows match the search in each status. */
export function useCatalogFilter<Row extends { status: PluginStatus }>(
  rows: Ref<Row[]>,
  searchable: (row: Row) => (string | null)[],
) {
  const query = ref('')
  const status = ref<StatusFilter>('all')

  const searched = computed(() => {
    const needle = query.value.trim().toLowerCase()
    if (!needle) return rows.value
    return rows.value.filter((row) =>
      searchable(row).some((text) => text?.toLowerCase().includes(needle)),
    )
  })
  const counts = computed(() => {
    const counts: Record<StatusFilter, number> = {
      all: searched.value.length,
      enabled: 0,
      disabled: 0,
      notInstalled: 0,
    }
    for (const row of searched.value) counts[row.status]++
    return counts
  })
  const shown = computed(() =>
    status.value === 'all'
      ? searched.value
      : searched.value.filter((row) => row.status === status.value),
  )

  function reset() {
    query.value = ''
    status.value = 'all'
  }

  return { query, status, counts, shown, reset }
}
