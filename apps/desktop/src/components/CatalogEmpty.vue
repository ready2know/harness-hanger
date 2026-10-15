<script setup lang="ts">
import { StoreIcon } from '@lucide/vue'
import { RouterLink } from 'vue-router'

import { Button } from '@/components/ui/button'

/** For example `skills`. */
defineProps<{ noun: string }>()
</script>

<template>
  <div class="flex flex-col items-center gap-3 rounded-xl border border-dashed p-12 text-center">
    <StoreIcon class="text-muted-foreground size-8" />
    <div class="flex flex-col gap-1">
      <h2 class="font-medium">No {{ noun }} yet</h2>
      <p class="text-muted-foreground max-w-sm text-sm">
        The {{ noun }} of the marketplaces you add show up here.
      </p>
    </div>
    <Button as-child>
      <RouterLink :to="{ name: 'marketplace' }">Go to Marketplace</RouterLink>
    </Button>
  </div>
</template>
