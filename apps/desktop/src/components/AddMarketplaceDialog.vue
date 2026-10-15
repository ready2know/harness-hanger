<script setup lang="ts">
import { CheckIcon, FolderOpenIcon, LoaderCircleIcon, XIcon } from '@lucide/vue'
import { open as openFolderPicker } from '@tauri-apps/plugin-dialog'
import { computed, ref, watch } from 'vue'

import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Textarea } from '@/components/ui/textarea'
import { addMarketplace, type MarketplaceSummary } from '@/lib/api'

const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{
  /** The one marketplace asked for was added, and the dialog closed. */
  added: [marketplace: MarketplaceSummary]
  /** Of several marketplaces asked for, at least one was added. The dialog shows each outcome. */
  addedSeveral: []
}>()

type Outcome = { state: 'adding' } | { state: 'done' } | { state: 'failed'; error: string }

const text = ref('')
const error = ref<string>()
const adding = ref(false)
/** The sources of a batch, in order, fixed once it starts. */
const queue = ref<string[]>([])
const outcomes = ref<Record<string, Outcome>>({})
const finished = ref(false)

/** One source per line. Blank lines, `#` comments, and repeats are left out. */
const sources = computed(() => [
  ...new Set(
    text.value
      .split('\n')
      .map((line) => line.trim())
      .filter((line) => line && !line.startsWith('#')),
  ),
])

watch(open, (isOpen) => {
  if (!isOpen) return
  text.value = ''
  error.value = undefined
  queue.value = []
  outcomes.value = {}
  finished.value = false
})

// An error is about the text it was raised for.
watch(text, () => (error.value = undefined))

function setOpen(value: boolean) {
  // Stays open while adding, so that each outcome lands where it was asked for.
  if (!adding.value) open.value = value
}

async function browse() {
  error.value = undefined
  try {
    const picked = await openFolderPicker({
      directory: true,
      multiple: true,
      title: 'Choose marketplace folders',
    })
    const folders = Array.isArray(picked) ? picked : picked ? [picked] : []
    if (folders.length) text.value = [text.value.trimEnd(), ...folders].filter(Boolean).join('\n')
  } catch (e) {
    error.value = String(e)
  }
}

async function submit() {
  if (adding.value || sources.value.length === 0) return
  error.value = undefined
  adding.value = true
  try {
    if (sources.value.length === 1) await addOne(sources.value[0]!)
    else await addSeveral(sources.value)
  } finally {
    adding.value = false
  }
}

async function addOne(source: string) {
  try {
    const marketplace = await addMarketplace(source)
    open.value = false
    emit('added', marketplace)
  } catch (e) {
    error.value = String(e)
  }
}

async function addSeveral(batch: string[]) {
  queue.value = batch
  let added = false
  for (const source of batch) {
    outcomes.value[source] = { state: 'adding' }
    try {
      await addMarketplace(source)
      outcomes.value[source] = { state: 'done' }
      added = true
    } catch (e) {
      outcomes.value[source] = { state: 'failed', error: String(e) }
    }
  }
  finished.value = true
  if (added) emit('addedSeveral')
}

const failed = computed(
  () => queue.value.filter((source) => outcomes.value[source]?.state === 'failed').length,
)
</script>

<template>
  <Dialog :open="open" @update:open="setOpen">
    <DialogContent class="sm:max-w-xl" :show-close-button="!adding">
      <form class="contents" @submit.prevent="submit">
        <DialogHeader>
          <DialogTitle>Add marketplace</DialogTitle>
          <DialogDescription>
            Paste a GitHub repository (<code>owner/repo</code> or its URL) or choose a local clone.
            To add several, put each on its own line. harness-hanger keeps its own catalog, so
            Claude Code does not see them.
          </DialogDescription>
        </DialogHeader>

        <template v-if="queue.length === 0">
          <!-- Read-only rather than disabled while adding, so that it keeps the focus for a retry. -->
          <Textarea
            v-model="text"
            aria-label="GitHub repositories or local paths, one per line"
            :placeholder="'anthropics/claude-plugins-official\nhttps://github.com/owner/repo\n/path/to/local/clone'"
            :readonly="adding"
            autofocus
            class="max-h-60 font-mono read-only:opacity-50"
            @keydown.enter.meta.prevent="submit"
            @keydown.enter.ctrl.prevent="submit"
          />
          <div class="flex items-center justify-between gap-3">
            <Button type="button" variant="outline" size="sm" :disabled="adding" @click="browse">
              <FolderOpenIcon />
              Browse…
            </Button>
            <span class="text-muted-foreground text-sm" aria-live="polite">
              {{ sources.length > 1 ? `${sources.length} marketplaces` : '' }}
            </span>
          </div>
          <p v-if="error" role="alert" class="text-destructive text-sm wrap-anywhere">
            {{ error }}
          </p>
        </template>

        <ul v-else class="max-h-[55vh] divide-y overflow-y-auto rounded-lg border">
          <li v-for="source in queue" :key="source" class="flex items-start gap-3 px-3 py-2.5">
            <div class="flex min-w-0 flex-1 flex-col gap-0.5">
              <span class="truncate font-mono text-xs" :title="source">{{ source }}</span>
              <span
                v-if="outcomes[source]?.state === 'failed'"
                role="alert"
                class="text-destructive text-xs wrap-anywhere"
              >
                {{ (outcomes[source] as { error: string }).error }}
              </span>
            </div>
            <span class="mt-0.5 size-4 shrink-0" aria-live="polite">
              <LoaderCircleIcon
                v-if="outcomes[source]?.state === 'adding'"
                class="size-4 animate-spin"
                aria-label="Adding"
              />
              <CheckIcon
                v-else-if="outcomes[source]?.state === 'done'"
                class="size-4 text-emerald-600 dark:text-emerald-400"
                aria-label="Added"
              />
              <XIcon
                v-else-if="outcomes[source]?.state === 'failed'"
                class="text-destructive size-4"
                aria-label="Not added"
              />
            </span>
          </li>
        </ul>

        <DialogFooter class="items-center">
          <span v-if="finished" class="text-muted-foreground mr-auto text-sm">
            {{ queue.length - failed }} added{{ failed ? `, ${failed} not` : '' }}
          </span>
          <Button v-if="finished" type="button" @click="setOpen(false)">Done</Button>
          <template v-else>
            <Button type="button" variant="ghost" :disabled="adding" @click="setOpen(false)">
              Cancel
            </Button>
            <Button type="submit" :disabled="adding || sources.length === 0">
              <LoaderCircleIcon v-if="adding" class="animate-spin" />
              {{ adding ? 'Adding…' : sources.length > 1 ? `Add ${sources.length}` : 'Add' }}
            </Button>
          </template>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
