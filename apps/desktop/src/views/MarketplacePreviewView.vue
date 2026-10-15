<script setup lang="ts">
import {
  ArrowLeftIcon,
  ExternalLinkIcon,
  SearchIcon,
  TrashIcon,
  TriangleAlertIcon,
} from '@lucide/vue'
import { computed, ref, watch } from 'vue'
import { RouterLink, useRouter } from 'vue-router'

import ComponentList from '@/components/ComponentList.vue'
import SourceIcon from '@/components/SourceIcon.vue'
import { Badge } from '@/components/ui/badge'
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
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { getMarketplaceDetails, removeMarketplace, type MarketplaceDetails } from '@/lib/api'
import { count, formatLocation, pluginContents, relativePath } from '@/lib/format'

const props = defineProps<{ id: number }>()

const router = useRouter()

const details = ref<MarketplaceDetails>()
const error = ref<string>()
const query = ref('')
const tab = ref('plugins')
const removeOpen = ref(false)
const removing = ref(false)
const removeError = ref<string>()

// The router reuses this view from one marketplace to the next.
watch(
  () => props.id,
  async (id, _, onCleanup) => {
    let stale = false
    // A late answer for the previous ID must not show, or be removed, under this one.
    onCleanup(() => (stale = true))
    details.value = undefined
    error.value = undefined
    query.value = ''
    tab.value = 'plugins'
    removeOpen.value = false
    try {
      const loaded = await getMarketplaceDetails(id)
      if (stale) return
      details.value = loaded
      // A harness's own components are not plugins, so there is no Plugins tab.
      if (loaded.summary.source.kind === 'harness') tab.value = 'skills'
    } catch (e) {
      if (!stale) error.value = String(e)
    }
  },
  { immediate: true },
)

const plugins = computed(() => details.value?.preview?.plugins ?? [])

function matches(...texts: (string | null)[]): boolean {
  const needle = query.value.trim().toLowerCase()
  return !needle || texts.some((text) => text?.toLowerCase().includes(needle))
}

const shownPlugins = computed(() =>
  plugins.value.filter((plugin) => matches(plugin.name, plugin.description, plugin.category)),
)

function componentsOf(kind: 'skills' | 'agents' | 'commands') {
  return plugins.value
    .flatMap((plugin) =>
      plugin[kind].map((component) => ({
        component,
        plugin: plugin.source.kind === 'harness' ? 'standalone' : plugin.name,
      })),
    )
    .filter(({ component }) => matches(component.name, component.description))
    .sort(
      (a, b) =>
        a.component.name.localeCompare(b.component.name) || a.plugin.localeCompare(b.plugin),
    )
}

const skills = computed(() => componentsOf('skills'))
const agents = computed(() => componentsOf('agents'))
const commands = computed(() => componentsOf('commands'))

function setRemoveOpen(value: boolean) {
  // Stays open while removing, so that a failure is seen.
  if (removing.value) return
  removeOpen.value = value
  removeError.value = undefined
}

async function remove() {
  removing.value = true
  removeError.value = undefined
  try {
    await removeMarketplace(props.id)
    removeOpen.value = false
    await router.push({ name: 'marketplace' })
  } catch (e) {
    removeError.value = String(e)
  } finally {
    removing.value = false
  }
}
</script>

<template>
  <div class="mx-auto flex w-full max-w-4xl flex-col gap-6 p-8">
    <Button as-child variant="ghost" size="sm" class="-ml-2 self-start">
      <RouterLink :to="{ name: 'marketplace' }">
        <ArrowLeftIcon />
        Marketplaces
      </RouterLink>
    </Button>

    <p v-if="error" role="alert" class="text-destructive">{{ error }}</p>
    <p v-else-if="!details" role="status" class="text-muted-foreground">Loading…</p>

    <template v-else>
      <header class="flex items-start justify-between gap-4">
        <div class="flex min-w-0 flex-col gap-1">
          <h1 class="truncate text-2xl font-semibold">
            {{ details.summary.name ?? 'Unreadable marketplace' }}
          </h1>
          <p v-if="details.summary.description" class="text-muted-foreground">
            {{ details.summary.description }}
          </p>
          <p class="text-muted-foreground flex items-center gap-1.5 font-mono text-xs">
            <SourceIcon :source="details.summary.source" />
            <span class="min-w-0 truncate" :title="formatLocation(details.summary)">
              {{ formatLocation(details.summary) }}
            </span>
            <template v-if="details.preview?.owner">
              <span aria-hidden="true">·</span>
              <span class="shrink-0 font-sans whitespace-nowrap"
                >by {{ details.preview.owner }}</span
              >
            </template>
          </p>
        </div>
        <Button variant="outline" class="shrink-0" @click="setRemoveOpen(true)">
          <TrashIcon />
          Remove
        </Button>
      </header>

      <p
        v-if="!details.preview"
        role="alert"
        class="bg-destructive/10 text-destructive rounded-xl p-4 text-sm wrap-anywhere"
      >
        Cannot read this marketplace: {{ details.summary.error }}
      </p>

      <Tabs v-else v-model="tab">
        <div class="flex flex-wrap items-center justify-between gap-3">
          <TabsList>
            <TabsTrigger v-if="details.summary.source.kind !== 'harness'" value="plugins">
              Plugins {{ shownPlugins.length }}
            </TabsTrigger>
            <TabsTrigger value="skills">Skills {{ skills.length }}</TabsTrigger>
            <TabsTrigger value="agents">Agents {{ agents.length }}</TabsTrigger>
            <TabsTrigger value="commands">Commands {{ commands.length }}</TabsTrigger>
          </TabsList>
          <div class="relative w-full sm:w-64">
            <SearchIcon
              class="text-muted-foreground pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2"
            />
            <Input v-model="query" placeholder="Search" aria-label="Search" class="pl-8" />
          </div>
        </div>

        <TabsContent value="plugins">
          <p v-if="shownPlugins.length === 0" class="text-muted-foreground py-6 text-center">
            No plugins match.
          </p>
          <ul v-else class="divide-y rounded-xl border">
            <li
              v-for="(plugin, index) in shownPlugins"
              :key="`${index}/${plugin.name}`"
              class="flex flex-col gap-1 px-4 py-3"
            >
              <div class="flex flex-wrap items-center gap-2">
                <span class="font-medium">{{ plugin.name }}</span>
                <Badge v-if="plugin.version" variant="secondary">{{ plugin.version }}</Badge>
                <Badge v-if="plugin.category" variant="outline">{{ plugin.category }}</Badge>
                <Badge
                  v-if="plugin.source.kind === 'external'"
                  variant="outline"
                  :title="plugin.source.location ?? undefined"
                >
                  <ExternalLinkIcon />
                  {{ plugin.source.source }}
                </Badge>
              </div>
              <p v-if="plugin.description" class="text-muted-foreground line-clamp-2">
                {{ plugin.description }}
              </p>
              <p class="text-muted-foreground text-xs">{{ pluginContents(plugin) }}</p>
            </li>
          </ul>
        </TabsContent>
        <TabsContent value="skills">
          <ComponentList :items="skills" :empty="query ? 'No skills match.' : 'No skills.'" />
        </TabsContent>
        <TabsContent value="agents">
          <ComponentList :items="agents" :empty="query ? 'No agents match.' : 'No agents.'" />
        </TabsContent>
        <TabsContent value="commands">
          <ComponentList :items="commands" :empty="query ? 'No commands match.' : 'No commands.'" />
        </TabsContent>
      </Tabs>

      <details v-if="details.preview?.warnings.length" class="rounded-xl border px-4 py-3">
        <summary class="flex cursor-pointer items-center gap-2 text-sm font-medium">
          <TriangleAlertIcon class="size-4 text-amber-500" />
          {{ count(details.preview.warnings.length, 'warning') }}
        </summary>
        <ul class="mt-3 flex flex-col gap-2">
          <li
            v-for="(warning, index) in details.preview.warnings"
            :key="index"
            class="flex flex-col"
          >
            <span class="text-muted-foreground font-mono text-xs break-all">
              {{ relativePath(warning.path, details.summary.root) }}
            </span>
            <span class="text-sm">{{ warning.message }}</span>
          </li>
        </ul>
      </details>

      <Dialog :open="removeOpen" @update:open="setRemoveOpen">
        <DialogContent :show-close-button="!removing">
          <DialogHeader>
            <DialogTitle>Remove {{ details.summary.name ?? 'this marketplace' }}?</DialogTitle>
            <DialogDescription v-if="details.summary.source.kind === 'github'">
              harness-hanger deletes its clone. You can add the marketplace again later.
            </DialogDescription>
            <DialogDescription v-else-if="details.summary.source.kind === 'harness'">
              Only the catalog entry is removed. The skills, agents, and commands stay where they
              are, and the harness keeps loading them.
            </DialogDescription>
            <DialogDescription v-else>
              Only the catalog entry is removed. The folder stays where it is.
            </DialogDescription>
          </DialogHeader>
          <p v-if="removeError" role="alert" class="text-destructive">{{ removeError }}</p>
          <DialogFooter>
            <Button variant="ghost" :disabled="removing" @click="setRemoveOpen(false)">
              Cancel
            </Button>
            <Button variant="destructive" :disabled="removing" @click="remove">
              {{ removing ? 'Removing…' : 'Remove' }}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </template>
  </div>
</template>
