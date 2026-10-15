<script setup lang="ts">
import { SearchIcon } from '@lucide/vue'
import type { AcceptableValue } from 'reka-ui'

import type { StatusFilter } from '@/composables/useCatalogPlugins'
import { Input } from '@/components/ui/input'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { statusLabels } from '@/lib/format'

defineProps<{ counts: Record<StatusFilter, number> }>()
const query = defineModel<string>('query', { required: true })
const status = defineModel<StatusFilter>('status', { required: true })

const filters: { value: StatusFilter; label: string }[] = [
  { value: 'all', label: 'All' },
  { value: 'enabled', label: statusLabels.enabled },
  { value: 'disabled', label: statusLabels.disabled },
  { value: 'notInstalled', label: statusLabels.notInstalled },
]

function select(value: AcceptableValue | AcceptableValue[]) {
  // Clicking the selected filter again would clear it; keep one selected.
  if (typeof value === 'string' && value) status.value = value as StatusFilter
}
</script>

<template>
  <div class="flex flex-wrap items-center justify-between gap-3">
    <ToggleGroup
      type="single"
      variant="outline"
      size="sm"
      orientation="horizontal"
      aria-label="Filter by status in Claude Code"
      :model-value="status"
      @update:model-value="select"
    >
      <ToggleGroupItem v-for="filter in filters" :key="filter.value" :value="filter.value">
        {{ filter.label }}
        <span class="text-muted-foreground tabular-nums">{{ counts[filter.value] }}</span>
      </ToggleGroupItem>
    </ToggleGroup>
    <div class="relative w-full sm:w-64">
      <SearchIcon
        class="text-muted-foreground pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2"
      />
      <Input v-model="query" placeholder="Search" aria-label="Search" class="pl-8" />
    </div>
  </div>
</template>
