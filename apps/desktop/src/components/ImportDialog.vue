<script setup lang="ts">
import { CheckIcon, LoaderCircleIcon, XIcon } from '@lucide/vue'
import { computed, ref, watch } from 'vue'

import SourceIcon from '@/components/SourceIcon.vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  getImportCandidates,
  importFromHarness,
  type Discovery,
  type ImportItem,
  type MarketplaceSource,
} from '@/lib/api'
import { count, formatSource } from '@/lib/format'

const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ imported: [] }>()

interface Row {
  key: string
  harness: string
  item: ImportItem
  title: string
  source: MarketplaceSource | null
  /** Where it is, or why it cannot be imported. */
  detail: string
  /** What comes with it. */
  contents: string | null
  /** Why it cannot be picked. */
  blocked: 'In catalog' | 'Cannot import' | null
}

type Outcome = { state: 'importing' } | { state: 'done' } | { state: 'failed'; error: string }

const discoveries = ref<Discovery[]>()
const loadError = ref<string>()
const selected = ref(new Set<string>())
const outcomes = ref<Record<string, Outcome>>({})
const importing = ref(false)
const finished = ref(false)

function rowsOf(discovery: Discovery): Row[] {
  const rows: Row[] = []
  const standalone = discovery.standalone
  if (standalone) {
    rows.push({
      key: `${discovery.harness}/standalone`,
      harness: discovery.harness,
      item: { kind: 'standalone' },
      title: 'Standalone skills, agents, and commands',
      source: standalone.source,
      detail: standalone.path,
      contents: [
        standalone.skills && count(standalone.skills, 'skill'),
        standalone.agents && count(standalone.agents, 'agent'),
        standalone.commands && count(standalone.commands, 'command'),
      ]
        .filter(Boolean)
        .join(' · '),
      blocked: standalone.inCatalog ? 'In catalog' : null,
    })
  }
  for (const marketplace of discovery.marketplaces) {
    const plugins = marketplace.installedPlugins
    const shown = plugins.slice(0, 5).join(', ')
    rows.push({
      key: `${discovery.harness}/marketplace/${marketplace.name}`,
      harness: discovery.harness,
      item: { kind: 'marketplace', name: marketplace.name },
      title: marketplace.name,
      source: marketplace.source,
      detail: marketplace.source
        ? formatSource(marketplace.source)
        : (marketplace.problem ?? 'Cannot be imported'),
      contents: plugins.length
        ? `Installed plugins: ${shown}${plugins.length > 5 ? ` and ${plugins.length - 5} more` : ''}`
        : null,
      blocked: marketplace.inCatalog ? 'In catalog' : marketplace.source ? null : 'Cannot import',
    })
  }
  return rows
}

const rows = computed(() => (discoveries.value ?? []).flatMap(rowsOf))
const chosen = computed(() => rows.value.filter((row) => selected.value.has(row.key)))

watch(open, async (isOpen, _, onCleanup) => {
  if (!isOpen) return
  let stale = false
  // A slow load from an earlier opening must not reset what was picked since.
  onCleanup(() => (stale = true))
  discoveries.value = undefined
  loadError.value = undefined
  outcomes.value = {}
  finished.value = false
  try {
    const loaded = await getImportCandidates()
    if (stale) return
    discoveries.value = loaded
    // Everything new is picked; people untick what they do not want.
    selected.value = new Set(rows.value.filter((row) => !row.blocked).map((row) => row.key))
  } catch (e) {
    if (!stale) loadError.value = String(e)
  }
})

function toggle(key: string, checked: boolean | 'indeterminate') {
  const next = new Set(selected.value)
  if (checked === true) next.add(key)
  else next.delete(key)
  selected.value = next
}

function setOpen(value: boolean) {
  // Stays open while importing, so that each outcome is seen.
  if (!importing.value) open.value = value
}

async function importChosen() {
  importing.value = true
  let imported = false
  for (const row of chosen.value) {
    outcomes.value[row.key] = { state: 'importing' }
    try {
      await importFromHarness(row.harness, row.item)
      outcomes.value[row.key] = { state: 'done' }
      imported = true
    } catch (e) {
      outcomes.value[row.key] = { state: 'failed', error: String(e) }
    }
  }
  importing.value = false
  finished.value = true
  if (imported) emit('imported')
}
</script>

<template>
  <Dialog :open="open" @update:open="setOpen">
    <DialogContent class="sm:max-w-2xl" :show-close-button="!importing">
      <DialogHeader>
        <DialogTitle>Import from a harness</DialogTitle>
        <DialogDescription>
          Marketplaces come with their plugins. A marketplace from GitHub is copied from the git
          clone that the harness keeps, when there is one, so it usually needs no network. Nothing
          in the harness changes.
        </DialogDescription>
      </DialogHeader>

      <p v-if="loadError" role="alert" class="text-destructive wrap-anywhere">{{ loadError }}</p>
      <p v-else-if="!discoveries" role="status" class="text-muted-foreground">Looking…</p>

      <div v-else class="flex max-h-[55vh] flex-col gap-5 overflow-y-auto">
        <section
          v-for="discovery in discoveries"
          :key="discovery.harness"
          class="flex flex-col gap-2"
          :aria-label="discovery.name"
        >
          <h3 class="flex items-baseline gap-2 font-medium">
            {{ discovery.name }}
            <span class="text-muted-foreground truncate font-mono text-xs font-normal">
              {{ discovery.configDir }}
            </span>
          </h3>
          <p v-if="!discovery.configDir" class="text-muted-foreground text-sm">
            Not found on this machine.
          </p>
          <ul v-else class="divide-y rounded-lg border">
            <li
              v-for="row in rowsOf(discovery)"
              :key="row.key"
              class="flex items-start gap-3 px-3 py-2.5"
            >
              <Checkbox
                :id="row.key"
                class="mt-0.5"
                :model-value="selected.has(row.key)"
                :disabled="!!row.blocked || importing || finished"
                @update:model-value="toggle(row.key, $event)"
              />
              <label :for="row.key" class="flex min-w-0 flex-1 flex-col gap-0.5">
                <span class="flex flex-wrap items-center gap-2">
                  <span class="font-medium">{{ row.title }}</span>
                  <Badge v-if="row.blocked" variant="outline" class="text-muted-foreground">
                    {{ row.blocked }}
                  </Badge>
                </span>
                <span class="text-muted-foreground flex min-w-0 items-center gap-1.5 text-xs">
                  <SourceIcon v-if="row.source" :source="row.source" />
                  <span class="truncate" :class="{ 'font-mono': row.source }">
                    {{ row.detail }}
                  </span>
                </span>
                <span v-if="row.contents" class="text-muted-foreground text-xs">
                  {{ row.contents }}
                </span>
                <span
                  v-if="outcomes[row.key]?.state === 'failed'"
                  role="alert"
                  class="text-destructive text-xs wrap-anywhere"
                >
                  {{ (outcomes[row.key] as { error: string }).error }}
                </span>
              </label>
              <span class="mt-0.5 size-4 shrink-0" aria-live="polite">
                <LoaderCircleIcon
                  v-if="outcomes[row.key]?.state === 'importing'"
                  class="size-4 animate-spin"
                  aria-label="Importing"
                />
                <CheckIcon
                  v-else-if="outcomes[row.key]?.state === 'done'"
                  class="size-4 text-emerald-600 dark:text-emerald-400"
                  aria-label="Imported"
                />
                <XIcon
                  v-else-if="outcomes[row.key]?.state === 'failed'"
                  class="text-destructive size-4"
                  aria-label="Not imported"
                />
              </span>
            </li>
          </ul>
          <p v-if="discovery.unmatchedPlugins.length" class="text-muted-foreground text-xs">
            Cannot be imported, because {{ discovery.name }} does not say where their marketplace
            is: {{ discovery.unmatchedPlugins.join(', ') }}
          </p>
        </section>
      </div>

      <DialogFooter>
        <Button v-if="finished" @click="setOpen(false)">Done</Button>
        <template v-else>
          <Button variant="ghost" :disabled="importing" @click="setOpen(false)">Cancel</Button>
          <Button :disabled="importing || chosen.length === 0" @click="importChosen">
            <LoaderCircleIcon v-if="importing" class="animate-spin" />
            {{ importing ? 'Importing…' : `Import ${chosen.length}` }}
          </Button>
        </template>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
