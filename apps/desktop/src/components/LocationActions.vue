<script setup lang="ts">
import { ExternalLinkIcon, FolderOpenIcon } from '@lucide/vue'
import { openUrl, revealItemInDir } from '@tauri-apps/plugin-opener'
import { computed, ref } from 'vue'

import { Button } from '@/components/ui/button'

/** Where to see a file or a folder: on this machine, and in a browser. */
const props = defineProps<{ path: string | null; webUrl: string | null }>()

const error = ref<string>()

const revealLabel = navigator.userAgent.includes('Mac')
  ? 'Show in Finder'
  : navigator.userAgent.includes('Windows')
    ? 'Show in Explorer'
    : 'Show in folder'
const webLabel = computed(() =>
  props.webUrl?.startsWith('https://github.com/') ? 'View on GitHub' : 'Open in browser',
)

async function run(action: () => Promise<void>) {
  error.value = undefined
  try {
    await action()
  } catch (e) {
    error.value = String(e)
  }
}
</script>

<template>
  <div v-if="path || webUrl" class="flex flex-col gap-2">
    <div class="flex flex-wrap gap-2">
      <Button v-if="path" variant="outline" size="sm" @click="run(() => revealItemInDir(path!))">
        <FolderOpenIcon />
        {{ revealLabel }}
      </Button>
      <Button v-if="webUrl" variant="outline" size="sm" @click="run(() => openUrl(webUrl!))">
        <ExternalLinkIcon />
        {{ webLabel }}
      </Button>
    </div>
    <p v-if="error" role="alert" class="text-destructive text-xs wrap-anywhere">{{ error }}</p>
  </div>
</template>
