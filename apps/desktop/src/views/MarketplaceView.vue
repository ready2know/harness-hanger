<script setup lang="ts">
import { DownloadIcon, PlusIcon, StoreIcon } from '@lucide/vue'
import { onMounted, ref } from 'vue'
import { RouterLink, useRouter } from 'vue-router'

import AddMarketplaceDialog from '@/components/AddMarketplaceDialog.vue'
import ImportDialog from '@/components/ImportDialog.vue'
import SourceIcon from '@/components/SourceIcon.vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { listMarketplaces, type MarketplaceSummary } from '@/lib/api'
import { count, formatLocation } from '@/lib/format'

const router = useRouter()

const marketplaces = ref<MarketplaceSummary[]>()
const error = ref<string>()
const addOpen = ref(false)
const importOpen = ref(false)

async function load() {
  try {
    marketplaces.value = await listMarketplaces()
    error.value = undefined
  } catch (e) {
    error.value = String(e)
  }
}

onMounted(load)

function openPreview(marketplace: MarketplaceSummary) {
  router.push({ name: 'marketplace-preview', params: { id: marketplace.id } })
}
</script>

<template>
  <div class="mx-auto flex w-full max-w-4xl flex-col gap-6 p-8">
    <header class="flex items-start justify-between gap-4">
      <div class="flex flex-col gap-1">
        <h1 class="text-2xl font-semibold">Marketplace</h1>
        <p class="text-muted-foreground text-sm">
          Plugin marketplaces you added. Open one to see its plugins, skills, agents, and commands.
        </p>
      </div>
      <div v-if="error || marketplaces?.length" class="flex shrink-0 gap-2">
        <Button variant="outline" @click="importOpen = true">
          <DownloadIcon />
          Import
        </Button>
        <Button @click="addOpen = true">
          <PlusIcon />
          Add marketplace
        </Button>
      </div>
    </header>

    <p v-if="error" role="alert" class="text-destructive">{{ error }}</p>
    <p v-else-if="!marketplaces" role="status" class="text-muted-foreground">Loading…</p>

    <div
      v-else-if="marketplaces.length === 0"
      class="flex flex-col items-center gap-3 rounded-xl border border-dashed p-12 text-center"
    >
      <StoreIcon class="text-muted-foreground size-8" />
      <div class="flex flex-col gap-1">
        <h2 class="font-medium">No marketplaces yet</h2>
        <p class="text-muted-foreground max-w-sm text-sm">
          Add one from GitHub or from a local clone, or import the ones a harness such as Claude
          Code already has.
        </p>
      </div>
      <div class="flex gap-2">
        <Button variant="outline" @click="importOpen = true">
          <DownloadIcon />
          Import
        </Button>
        <Button @click="addOpen = true">
          <PlusIcon />
          Add marketplace
        </Button>
      </div>
    </div>

    <ul v-else class="grid gap-3 md:grid-cols-2">
      <li v-for="marketplace in marketplaces" :key="marketplace.id">
        <RouterLink
          :to="{ name: 'marketplace-preview', params: { id: marketplace.id } }"
          class="focus-visible:ring-ring/50 block h-full rounded-xl outline-none focus-visible:ring-3"
        >
          <Card class="hover:bg-muted/50 h-full transition-colors">
            <!-- One shrinkable column, so long paths truncate instead of widening the card. -->
            <CardHeader class="grid-cols-1">
              <CardTitle class="flex items-center justify-between gap-2">
                <span class="min-w-0 truncate">
                  {{ marketplace.name ?? 'Unreadable marketplace' }}
                </span>
                <template v-if="!marketplace.error">
                  <Badge v-if="marketplace.source.kind === 'harness'" variant="secondary">
                    Standalone
                  </Badge>
                  <Badge v-else variant="secondary">
                    {{ count(marketplace.pluginCount, 'plugin') }}
                  </Badge>
                </template>
              </CardTitle>
              <CardDescription class="flex flex-col gap-2">
                <span
                  class="flex min-w-0 items-center gap-1.5 font-mono text-xs"
                  :title="formatLocation(marketplace)"
                >
                  <SourceIcon :source="marketplace.source" />
                  <span class="min-w-0 truncate">{{ formatLocation(marketplace) }}</span>
                </span>
                <span v-if="marketplace.error" class="text-destructive wrap-anywhere">
                  {{ marketplace.error }}
                </span>
                <span v-else-if="marketplace.description" class="line-clamp-2">
                  {{ marketplace.description }}
                </span>
              </CardDescription>
            </CardHeader>
          </Card>
        </RouterLink>
      </li>
    </ul>

    <AddMarketplaceDialog v-model:open="addOpen" @added="openPreview" />
    <ImportDialog v-model:open="importOpen" @imported="load" />
  </div>
</template>
