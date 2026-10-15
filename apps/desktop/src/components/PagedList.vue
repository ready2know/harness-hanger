<script setup lang="ts" generic="T">
import { useLocalStorage } from '@vueuse/core'
import { computed, ref, useId, watch } from 'vue'

import {
  Pagination,
  PaginationContent,
  PaginationEllipsis,
  PaginationItem,
  PaginationNext,
  PaginationPrevious,
} from '@/components/ui/pagination'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'

/**
 * Shows `items` one page at a time, with controls below to turn pages and to pick the page size.
 * To start again at the first page, for example when a search changes, give it a new `key`.
 */
const props = defineProps<{ items: T[] }>()
defineSlots<{ default(props: { items: T[] }): unknown }>()

const pageSizes = ['10', '25', '50', '100', 'all'] as const
type PageSize = (typeof pageSizes)[number]

// One size for every list, kept across launches.
const storedSize = useLocalStorage<string>('list-page-size', '10')
const pageSize = computed<PageSize>({
  get: () => pageSizes.find((size) => size === storedSize.value) ?? '10',
  set: (size) => (storedSize.value = size),
})
const perPage = computed(() =>
  pageSize.value === 'all' ? Math.max(props.items.length, 1) : Number(pageSize.value),
)

const page = ref(1)
const pageCount = computed(() => Math.max(1, Math.ceil(props.items.length / perPage.value)))
const first = computed(() => (page.value - 1) * perPage.value)
const shown = computed(() => props.items.slice(first.value, first.value + perPage.value))

// The new page starts with the item that started the old one. This runs before the watcher
// below, which would otherwise move the page first.
watch(pageSize, (_, previous) => {
  const previousPerPage = previous === 'all' ? Infinity : Number(previous)
  const firstIndex = page.value === 1 ? 0 : (page.value - 1) * previousPerPage
  page.value = Math.floor(firstIndex / perPage.value) + 1
})

// A shorter list can leave fewer pages than the one shown.
watch(pageCount, (count) => (page.value = Math.min(page.value, count)))

const labelId = useId()

const root = ref<HTMLElement>()

function turnTo(to: number) {
  page.value = to
  // Turned from the controls below a long list, the new page is seen from its top.
  if (root.value && root.value.getBoundingClientRect().top < 0) {
    root.value.scrollIntoView({ block: 'start' })
  }
}
</script>

<template>
  <div ref="root" class="flex scroll-mt-6 flex-col gap-3">
    <slot :items="shown" />
    <div
      v-if="items.length > Number(pageSizes[0])"
      class="text-muted-foreground flex flex-wrap items-center justify-between gap-x-4 gap-y-2 text-sm"
    >
      <div class="flex items-center gap-2">
        <span :id="labelId">Per page</span>
        <Select v-model="pageSize">
          <SelectTrigger size="sm" :aria-labelledby="labelId" class="text-foreground">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem v-for="size in pageSizes" :key="size" :value="size">
              {{ size === 'all' ? 'All' : size }}
            </SelectItem>
          </SelectContent>
        </Select>
        <span aria-live="polite">
          {{ first + 1 }}–{{ first + shown.length }} of {{ items.length }}
        </span>
      </div>
      <Pagination
        v-if="pageCount > 1"
        v-slot="{ page: current }"
        :page="page"
        :total="items.length"
        :items-per-page="perPage"
        :sibling-count="1"
        show-edges
        class="mx-0 w-auto"
        @update:page="turnTo"
      >
        <PaginationContent v-slot="{ items: links }">
          <PaginationPrevious size="sm" />
          <template v-for="(link, index) in links" :key="index">
            <PaginationItem
              v-if="link.type === 'page'"
              :value="link.value"
              :is-active="link.value === current"
              size="icon-sm"
            >
              {{ link.value }}
            </PaginationItem>
            <PaginationEllipsis v-else :index="index" />
          </template>
          <PaginationNext size="sm" />
        </PaginationContent>
      </Pagination>
    </div>
  </div>
</template>
