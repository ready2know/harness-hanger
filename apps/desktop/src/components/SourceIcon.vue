<script setup lang="ts">
import { FolderIcon, GitBranchIcon, UserIcon } from '@lucide/vue'

import type { MarketplaceSource } from '@/lib/api'

defineProps<{ source: MarketplaceSource }>()
</script>

<template>
  <GitBranchIcon v-if="source.kind === 'github'" class="size-3.5 shrink-0" />
  <UserIcon v-else-if="source.kind === 'harness'" class="size-3.5 shrink-0" />
  <FolderIcon v-else class="size-3.5 shrink-0" />
</template>
