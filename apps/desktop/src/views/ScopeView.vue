<script setup lang="ts">
import {
  DownloadIcon,
  FolderIcon,
  GlobeIcon,
  LoaderCircleIcon,
  SearchIcon,
  TrashIcon,
  TriangleAlertIcon,
} from '@lucide/vue'
import { storeToRefs } from 'pinia'
import { computed, onMounted, ref, watch } from 'vue'

import ComponentList from '@/components/ComponentList.vue'
import PagedList from '@/components/PagedList.vue'
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
import { Switch } from '@/components/ui/switch'
import {
  getScopeState,
  installPlugin,
  listCatalogPlugins,
  setPluginEnabled,
  uninstallPlugin,
  type CatalogPlugin,
  type Scope,
  type ScopedPlugin,
  type ScopeState,
} from '@/lib/api'
import { count, relativePath } from '@/lib/format'
import { useProjectsStore } from '@/stores/projects'

/** `null` for the global setup. */
const props = defineProps<{ projectId: number | null }>()

const store = useProjectsStore()
const { projects } = storeToRefs(store)
onMounted(() => {
  if (!projects.value) store.load()
})
const project = computed(() =>
  props.projectId === null ? null : projects.value?.find((p) => p.id === props.projectId),
)

const state = ref<ScopeState>()
const error = ref<string>()
const catalog = ref<CatalogPlugin[]>()
const catalogError = ref<string>()
const installQuery = ref('')
/**
 * Changes in progress and errors of changes that failed, by `projectId/pluginId`: a global
 * plugin has the same ID in every project, and a change can outlive a visit to its project.
 */
const busy = ref<Record<string, boolean>>({})
const rowErrors = ref<Record<string, string>>({})
const keyOf = (pluginId: string) => `${props.projectId}/${pluginId}`
/** The plugin that waits for the person to confirm its uninstall. */
const confirming = ref<ScopedPlugin>()

// The router reuses this view from one project, or Global, to the next.
watch(
  () => props.projectId,
  async (projectId, _, onCleanup) => {
    let stale = false
    onCleanup(() => (stale = true))
    state.value = undefined
    error.value = undefined
    catalog.value = undefined
    catalogError.value = undefined
    installQuery.value = ''
    confirming.value = undefined
    try {
      const loaded = await getScopeState(projectId)
      if (!stale) state.value = loaded
    } catch (e) {
      if (!stale) error.value = String(e)
    }
    if (projectId === null) return
    try {
      const loaded = await listCatalogPlugins()
      if (!stale) catalog.value = loaded.plugins
    } catch (e) {
      if (!stale) catalogError.value = String(e)
    }
  },
  { immediate: true },
)

/** Runs a change through Claude Code, then shows the state that it left. */
async function change(pluginId: string, run: (projectId: number) => Promise<void>) {
  const projectId = props.projectId
  if (projectId === null) return
  const key = `${projectId}/${pluginId}`
  if (busy.value[key]) return
  busy.value[key] = true
  delete rowErrors.value[key]
  try {
    await run(projectId)
  } catch (e) {
    rowErrors.value[key] = String(e)
  }
  try {
    // Only the project on screen shows the new state; another one reloads when it is opened.
    if (props.projectId === projectId) state.value = await getScopeState(projectId)
  } catch (e) {
    if (props.projectId === projectId) error.value = String(e)
  } finally {
    busy.value[key] = false
  }
}

function toggle(plugin: ScopedPlugin, enabled: boolean) {
  return change(plugin.id, (projectId) => setPluginEnabled(projectId, plugin.id, enabled))
}

function uninstall(plugin: ScopedPlugin) {
  confirming.value = undefined
  return change(plugin.id, (projectId) => uninstallPlugin(projectId, plugin.id))
}

function install(entry: CatalogPlugin) {
  return change(idOf(entry), (projectId) =>
    installPlugin(projectId, entry.marketplaceId, entry.plugin.name),
  )
}

function idOf(entry: CatalogPlugin): string {
  return `${entry.plugin.name}@${entry.marketplace}`
}

/** Catalog plugins that can go into this project: not installed for the user, nor here. */
const candidates = computed(() => {
  if (!catalog.value || !state.value) return []
  const installed = new Set(state.value.plugins.map((plugin) => plugin.id))
  const needle = installQuery.value.trim().toLowerCase()
  return catalog.value
    .filter(
      (entry) =>
        entry.plugin.source.kind !== 'harness' &&
        entry.status === 'notInstalled' &&
        !installed.has(idOf(entry)),
    )
    .filter(
      (entry) =>
        !needle ||
        [entry.plugin.name, entry.plugin.description, entry.marketplace].some((text) =>
          text?.toLowerCase().includes(needle),
        ),
    )
    .sort((a, b) => a.plugin.name.localeCompare(b.plugin.name))
})

const scopeLabels: Record<Scope, string> = {
  user: 'Global',
  project: 'Project',
  local: 'Personal',
}

/** Where the plugin comes from, and what decides whether it is on here. */
function explain(plugin: ScopedPlugin): string {
  const state = plugin.enabled ? 'on' : 'off'
  if (props.projectId === null) {
    return plugin.decidedBy ? `Switched ${state} for your user` : 'Off: no settings switch it on'
  }
  if (plugin.installedIn === 'local') return `Installed only for you, in this project`
  const installed =
    plugin.installedIn === 'project'
      ? 'Installed for everyone who works on this project'
      : 'Installed globally'
  if (plugin.decidedBy === 'local') return `${installed}; switched ${state} here only for you`
  if (plugin.decidedBy === 'project' && plugin.installedIn === 'user') {
    return `${installed}; switched ${state} for everyone in this project`
  }
  return installed
}

const standaloneItems = computed(() => {
  const standalone = state.value?.standalone
  if (!standalone) return []
  return [
    ...standalone.skills.map((component) => ({ component, plugin: 'skill' })),
    ...standalone.agents.map((component) => ({ component, plugin: 'agent' })),
    ...standalone.commands.map((component) => ({ component, plugin: 'command' })),
  ]
})
</script>

<template>
  <div class="mx-auto flex w-full max-w-4xl flex-col gap-8 p-8">
    <header class="flex flex-col gap-1">
      <h1 class="flex items-center gap-2 text-2xl font-semibold">
        <GlobeIcon v-if="projectId === null" class="text-muted-foreground size-5" />
        <FolderIcon v-else class="text-muted-foreground size-5" />
        {{ projectId === null ? 'Global' : (project?.name ?? 'Project') }}
      </h1>
      <p v-if="projectId === null" class="text-muted-foreground text-sm">
        What Claude Code loads for your user, in every project. To change it, use
        <code>/plugin</code> in Claude Code, or switch a plugin off for one project on its page.
      </p>
      <template v-else-if="project">
        <p class="text-muted-foreground truncate font-mono text-xs" :title="project.path">
          {{ project.path }}
        </p>
        <p v-if="project.error" role="alert" class="text-destructive text-sm">
          {{ project.error }}
        </p>
      </template>
      <p v-else-if="projects" role="alert" class="text-destructive text-sm">
        This project is not in the list. Add it in Settings.
      </p>
    </header>

    <p v-if="error" role="alert" class="text-destructive wrap-anywhere">{{ error }}</p>
    <p v-else-if="!state" role="status" class="text-muted-foreground">Loading…</p>

    <template v-else>
      <section class="flex flex-col gap-3" aria-labelledby="plugins-heading">
        <h2 id="plugins-heading" class="text-lg font-semibold">
          Plugins
          <span class="text-muted-foreground font-normal">{{ state.plugins.length }}</span>
        </h2>
        <p v-if="state.plugins.length === 0" class="text-muted-foreground text-sm">
          No plugins are installed {{ projectId === null ? 'for your user' : 'here' }}.
        </p>
        <ul v-else class="divide-y rounded-xl border">
          <li
            v-for="plugin in state.plugins"
            :key="plugin.id"
            class="flex items-start gap-3 px-4 py-3"
          >
            <div class="flex min-w-0 flex-1 flex-col gap-1">
              <div class="flex flex-wrap items-center gap-2">
                <span class="font-medium">{{ plugin.name }}</span>
                <span class="text-muted-foreground text-xs">@{{ plugin.marketplace }}</span>
                <Badge v-if="plugin.version" variant="secondary">{{ plugin.version }}</Badge>
                <Badge v-if="projectId !== null" variant="outline">
                  {{ scopeLabels[plugin.installedIn] }}
                </Badge>
              </div>
              <p v-if="plugin.description" class="text-muted-foreground line-clamp-2 text-sm">
                {{ plugin.description }}
              </p>
              <p class="text-muted-foreground text-xs">{{ explain(plugin) }}</p>
              <p v-if="rowErrors[keyOf(plugin.id)]" role="alert" class="text-destructive text-xs">
                {{ rowErrors[keyOf(plugin.id)] }}
              </p>
            </div>
            <div class="flex shrink-0 items-center gap-2">
              <LoaderCircleIcon
                v-if="busy[keyOf(plugin.id)]"
                class="text-muted-foreground size-4 animate-spin"
                aria-label="Changing"
              />
              <Button
                v-if="projectId !== null && plugin.installedIn !== 'user'"
                variant="ghost"
                size="icon-sm"
                :aria-label="`Uninstall ${plugin.name}`"
                :disabled="busy[keyOf(plugin.id)]"
                @click="confirming = plugin"
              >
                <TrashIcon />
              </Button>
              <Switch
                v-if="projectId !== null"
                :model-value="plugin.enabled"
                :disabled="busy[keyOf(plugin.id)]"
                :aria-label="`${plugin.name} on in this project`"
                @update:model-value="toggle(plugin, $event)"
              />
              <Badge
                v-else
                :variant="plugin.enabled ? 'secondary' : 'outline'"
                :class="
                  plugin.enabled
                    ? 'bg-emerald-500/15 text-emerald-700 dark:bg-emerald-500/20 dark:text-emerald-400'
                    : 'text-muted-foreground'
                "
              >
                {{ plugin.enabled ? 'Enabled' : 'Disabled' }}
              </Badge>
            </div>
          </li>
        </ul>
      </section>

      <section
        v-if="projectId !== null"
        class="flex flex-col gap-3"
        aria-labelledby="install-heading"
      >
        <div class="flex flex-wrap items-end justify-between gap-3">
          <div class="flex flex-col gap-1">
            <h2 id="install-heading" class="text-lg font-semibold">Install into this project</h2>
            <p class="text-muted-foreground text-sm">
              From the marketplaces in your catalog. A plugin from a GitHub marketplace is for
              everyone who works on this project: Claude Code records it, and its marketplace, in
              <code>.claude/settings.json</code>. One from a local folder is only for you, in
              <code>.claude/settings.local.json</code>, because the folder is only on this machine.
              Plugins installed globally are not offered; switch them off above instead.
            </p>
          </div>
          <div class="relative w-full sm:w-64">
            <SearchIcon
              class="text-muted-foreground pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2"
            />
            <Input
              v-model="installQuery"
              placeholder="Search"
              aria-label="Search plugins to install"
              class="pl-8"
            />
          </div>
        </div>
        <p v-if="catalogError" role="alert" class="text-destructive text-sm wrap-anywhere">
          {{ catalogError }}
        </p>
        <p v-else-if="!catalog" role="status" class="text-muted-foreground text-sm">Loading…</p>
        <p v-else-if="candidates.length === 0" class="text-muted-foreground text-sm">
          {{ installQuery ? 'No plugins match.' : 'Nothing to install: add a marketplace first.' }}
        </p>
        <PagedList v-else :key="installQuery" v-slot="{ items: visible }" :items="candidates">
          <ul class="divide-y rounded-xl border">
            <li
              v-for="entry in visible"
              :key="`${entry.marketplaceId}/${entry.plugin.name}`"
              class="flex items-start gap-3 px-4 py-3"
            >
              <div class="flex min-w-0 flex-1 flex-col gap-1">
                <div class="flex flex-wrap items-center gap-2">
                  <span class="font-medium">{{ entry.plugin.name }}</span>
                  <span class="text-muted-foreground text-xs">@{{ entry.marketplace }}</span>
                </div>
                <p
                  v-if="entry.plugin.description"
                  class="text-muted-foreground line-clamp-2 text-sm"
                >
                  {{ entry.plugin.description }}
                </p>
                <p
                  v-if="rowErrors[keyOf(idOf(entry))]"
                  role="alert"
                  class="text-destructive text-xs"
                >
                  {{ rowErrors[keyOf(idOf(entry))] }}
                </p>
              </div>
              <Button
                variant="outline"
                size="sm"
                class="shrink-0"
                :disabled="busy[keyOf(idOf(entry))]"
                @click="install(entry)"
              >
                <LoaderCircleIcon v-if="busy[keyOf(idOf(entry))]" class="animate-spin" />
                <DownloadIcon v-else />
                Install
              </Button>
            </li>
          </ul>
        </PagedList>
      </section>

      <section class="flex flex-col gap-3" aria-labelledby="standalone-heading">
        <div class="flex flex-col gap-1">
          <h2 id="standalone-heading" class="text-lg font-semibold">
            Standalone skills, agents, and commands
          </h2>
          <p class="text-muted-foreground text-sm">
            Outside plugins, in
            <code>{{ projectId === null ? '~/.claude' : '.claude' }}</code
            >. Claude Code always loads them
            {{ projectId === null ? 'for your user' : 'in this project' }}.
          </p>
        </div>
        <ComponentList :items="standaloneItems" empty="None." />
        <details v-if="state.warnings.length" class="rounded-xl border px-4 py-3">
          <summary class="flex cursor-pointer items-center gap-2 text-sm font-medium">
            <TriangleAlertIcon class="size-4 text-amber-500" />
            {{ count(state.warnings.length, 'warning') }}
          </summary>
          <ul class="mt-3 flex flex-col gap-2">
            <li v-for="(warning, index) in state.warnings" :key="index" class="flex flex-col">
              <span class="text-muted-foreground font-mono text-xs break-all">
                {{ state.project ? relativePath(warning.path, state.project) : warning.path }}
              </span>
              <span class="text-sm">{{ warning.message }}</span>
            </li>
          </ul>
        </details>
      </section>
    </template>

    <Dialog :open="!!confirming" @update:open="(open) => !open && (confirming = undefined)">
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Uninstall {{ confirming?.name }}?</DialogTitle>
          <DialogDescription v-if="confirming?.installedIn === 'project'">
            It is installed for everyone who works on this project, so Claude Code removes it from
            the shared <code>.claude/settings.json</code>.
          </DialogDescription>
          <DialogDescription v-else>
            It is installed only for you, in this project.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="ghost" @click="confirming = undefined">Cancel</Button>
          <Button variant="destructive" @click="confirming && uninstall(confirming)">
            Uninstall
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
