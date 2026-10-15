<script setup lang="ts">
import { TriangleAlertIcon } from '@lucide/vue'
import { RouterLink } from 'vue-router'

import type { MarketplaceSummary } from '@/lib/api'
import { count, formatSource } from '@/lib/format'

defineProps<{
  marketplaces: MarketplaceSummary[]
  /** What is missing because of them, for example `skills`. */
  noun: string
}>()
</script>

<template>
  <div
    role="alert"
    class="flex gap-3 rounded-xl border border-amber-500/40 bg-amber-500/10 p-4 text-sm"
  >
    <TriangleAlertIcon class="mt-0.5 size-4 shrink-0 text-amber-600 dark:text-amber-400" />
    <div class="flex min-w-0 flex-col gap-1">
      <p class="font-medium">
        Some {{ noun }} are missing, because {{ count(marketplaces.length, 'marketplace') }}
        could not be read:
      </p>
      <ul class="flex flex-col gap-1">
        <li v-for="marketplace in marketplaces" :key="marketplace.id" class="wrap-anywhere">
          <RouterLink
            :to="{ name: 'marketplace-preview', params: { id: marketplace.id } }"
            class="font-medium underline-offset-4 hover:underline"
          >
            {{ marketplace.name ?? formatSource(marketplace.source) }}
          </RouterLink>
          <span class="text-muted-foreground"> — {{ marketplace.error }}</span>
        </li>
      </ul>
    </div>
  </div>
</template>
