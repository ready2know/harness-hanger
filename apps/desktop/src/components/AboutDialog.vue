<script setup lang="ts">
import { ref, watch } from 'vue'

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { getAppInfo } from '@/lib/api'

const open = defineModel<boolean>('open', { required: true })

const version = ref<string>()
const error = ref<string>()

watch(open, async (isOpen) => {
  if (!isOpen || version.value) return
  error.value = undefined
  try {
    version.value = (await getAppInfo()).version
  } catch (e) {
    error.value = String(e)
  }
})
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent>
      <DialogHeader>
        <DialogTitle>harness-hanger</DialogTitle>
        <DialogDescription>
          <span v-if="error" class="text-destructive">{{ error }}</span>
          <template v-else-if="version">Version {{ version }}</template>
          <template v-else>Loading…</template>
        </DialogDescription>
      </DialogHeader>
    </DialogContent>
  </Dialog>
</template>
