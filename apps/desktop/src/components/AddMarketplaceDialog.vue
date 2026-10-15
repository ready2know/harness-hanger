<script setup lang="ts">
import { FolderOpenIcon, LoaderCircleIcon } from '@lucide/vue'
import { open as openFolderPicker } from '@tauri-apps/plugin-dialog'
import { ref, watch } from 'vue'

import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { addMarketplace, type MarketplaceSummary } from '@/lib/api'

const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ added: [marketplace: MarketplaceSummary] }>()

const source = ref('')
const adding = ref(false)
const error = ref<string>()

watch(open, (isOpen) => {
  if (isOpen) {
    source.value = ''
    error.value = undefined
  }
})

function setOpen(value: boolean) {
  // Stays open while a GitHub clone runs, so that its outcome lands where it was asked for.
  if (!adding.value) open.value = value
}

async function browse() {
  try {
    const folder = await openFolderPicker({ directory: true, title: 'Choose a marketplace folder' })
    if (folder) source.value = folder
  } catch (e) {
    error.value = String(e)
  }
}

async function submit() {
  const trimmed = source.value.trim()
  if (!trimmed || adding.value) return
  adding.value = true
  error.value = undefined
  try {
    const marketplace = await addMarketplace(trimmed)
    open.value = false
    emit('added', marketplace)
  } catch (e) {
    error.value = String(e)
  } finally {
    adding.value = false
  }
}
</script>

<template>
  <Dialog :open="open" @update:open="setOpen">
    <DialogContent class="sm:max-w-lg" :show-close-button="!adding">
      <form class="contents" @submit.prevent="submit">
        <DialogHeader>
          <DialogTitle>Add marketplace</DialogTitle>
          <DialogDescription>
            Paste a GitHub repository, or choose a local clone. harness-hanger keeps its own
            catalog, so Claude Code does not see it.
          </DialogDescription>
        </DialogHeader>

        <div class="flex gap-2">
          <Input
            v-model="source"
            aria-label="GitHub repository or local path"
            placeholder="https://github.com/owner/repo or /path/to/clone"
            :disabled="adding"
            autofocus
          />
          <Button type="button" variant="outline" :disabled="adding" @click="browse">
            <FolderOpenIcon />
            Browse…
          </Button>
        </div>
        <p v-if="error" role="alert" class="text-destructive wrap-anywhere">{{ error }}</p>

        <DialogFooter>
          <Button type="button" variant="ghost" :disabled="adding" @click="setOpen(false)">
            Cancel
          </Button>
          <Button type="submit" :disabled="adding || !source.trim()">
            <LoaderCircleIcon v-if="adding" class="animate-spin" />
            {{ adding ? 'Adding…' : 'Add' }}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
