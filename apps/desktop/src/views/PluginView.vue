<script setup lang="ts">
import { ArrowLeftIcon } from '@lucide/vue'
import { computed, ref, watch } from 'vue'
import { RouterLink } from 'vue-router'

import ComponentList from '@/components/ComponentList.vue'
import ExternalLink from '@/components/ExternalLink.vue'
import LocationActions from '@/components/LocationActions.vue'
import MarkdownContent from '@/components/MarkdownContent.vue'
import PluginStatusBadge from '@/components/PluginStatusBadge.vue'
import SourceIcon from '@/components/SourceIcon.vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { getPluginDetails, type ComponentKind, type PluginDetails } from '@/lib/api'
import { formatLocation, relativePath } from '@/lib/format'
import { componentRoute } from '@/lib/routes'

/** `name` is the plugin's name in the catalog marketplace `marketplaceId`. */
const props = defineProps<{ marketplaceId: number; name: string }>()

const details = ref<PluginDetails>()
const error = ref<string>()
const tab = ref('readme')

// The router reuses this view from one plugin to the next.
watch(
  () => [props.marketplaceId, props.name] as const,
  async ([marketplaceId, name], _, onCleanup) => {
    let stale = false
    // A late answer for the previous plugin must not show under this one.
    onCleanup(() => (stale = true))
    details.value = undefined
    error.value = undefined
    try {
      const loaded = await getPluginDetails(marketplaceId, name)
      if (stale) return
      details.value = loaded
      tab.value = loaded.readme ? 'readme' : 'skills'
    } catch (e) {
      if (!stale) error.value = String(e)
    }
  },
  { immediate: true },
)

/** The plugin's directory, when it is inside the marketplace. */
const folder = computed(() => {
  const source = details.value?.plugin.source
  return source?.kind === 'local' ? source.path : null
})

function componentsOf(kind: ComponentKind, list: 'skills' | 'agents' | 'commands') {
  const loaded = details.value
  if (!loaded) return []
  return loaded.plugin[list].map((component) => ({
    component,
    to: componentRoute(kind, loaded.marketplace.id, loaded.plugin.name, component.name),
  }))
}

const skills = computed(() => componentsOf('skill', 'skills'))
const agents = computed(() => componentsOf('agent', 'agents'))
const commands = computed(() => componentsOf('command', 'commands'))
</script>

<template>
  <div class="mx-auto flex w-full max-w-4xl flex-col gap-6 p-8">
    <Button as-child variant="ghost" size="sm" class="-ml-2 self-start">
      <RouterLink :to="{ name: 'plugins' }">
        <ArrowLeftIcon />
        Plugins
      </RouterLink>
    </Button>

    <p v-if="error" role="alert" class="text-destructive wrap-anywhere">{{ error }}</p>
    <p v-else-if="!details" role="status" class="text-muted-foreground">Loading…</p>

    <template v-else>
      <header class="flex flex-col gap-3">
        <div class="flex items-start justify-between gap-4">
          <div class="flex min-w-0 flex-wrap items-center gap-2">
            <h1 class="text-2xl font-semibold break-all">{{ details.plugin.name }}</h1>
            <Badge v-if="details.plugin.version" variant="secondary">
              {{ details.plugin.version }}
            </Badge>
          </div>
          <PluginStatusBadge :status="details.status" />
        </div>
        <p v-if="details.plugin.description" class="text-muted-foreground">
          {{ details.plugin.description }}
        </p>
        <div v-if="details.plugin.keywords.length" class="flex flex-wrap gap-1.5" aria-label="Tags">
          <Badge v-for="keyword in details.plugin.keywords" :key="keyword" variant="secondary">
            {{ keyword }}
          </Badge>
        </div>
        <LocationActions :path="folder" :web-url="details.webUrl" />
      </header>

      <dl
        class="grid grid-cols-[max-content_minmax(0,1fr)] gap-x-6 gap-y-3 rounded-xl border p-4 text-sm"
      >
        <dt class="text-muted-foreground">Marketplace</dt>
        <dd class="flex min-w-0 flex-col gap-0.5">
          <RouterLink
            :to="{ name: 'marketplace-preview', params: { id: details.marketplace.id } }"
            class="self-start font-medium underline-offset-4 hover:underline"
          >
            {{ details.marketplace.name }}
          </RouterLink>
          <span class="text-muted-foreground flex items-center gap-1.5 font-mono text-xs">
            <SourceIcon :source="details.marketplace.source" />
            <span class="truncate" :title="formatLocation(details.marketplace)">
              {{ formatLocation(details.marketplace) }}
            </span>
          </span>
        </dd>

        <template v-if="details.plugin.source.kind === 'external'">
          <dt class="text-muted-foreground">Installed from</dt>
          <dd class="wrap-anywhere">
            {{ details.plugin.source.source }}
            <span v-if="details.plugin.source.location" class="font-mono text-xs">
              {{ details.plugin.source.location }}
            </span>
          </dd>
        </template>
        <template v-else-if="folder">
          <dt class="text-muted-foreground">Folder</dt>
          <dd class="font-mono text-xs leading-5 break-all" :title="folder">
            {{
              folder === details.marketplace.root
                ? 'The marketplace root'
                : relativePath(folder, details.marketplace.root)
            }}
          </dd>
        </template>

        <template v-if="details.plugin.author">
          <dt class="text-muted-foreground">Author</dt>
          <dd>{{ details.plugin.author }}</dd>
        </template>
        <template v-if="details.plugin.category">
          <dt class="text-muted-foreground">Category</dt>
          <dd>{{ details.plugin.category }}</dd>
        </template>
        <template v-if="details.plugin.license">
          <dt class="text-muted-foreground">License</dt>
          <dd>{{ details.plugin.license }}</dd>
        </template>
        <template v-if="details.plugin.homepage">
          <dt class="text-muted-foreground">Homepage</dt>
          <dd><ExternalLink :href="details.plugin.homepage" /></dd>
        </template>
        <template v-if="details.plugin.repository">
          <dt class="text-muted-foreground">Repository</dt>
          <dd><ExternalLink :href="details.plugin.repository" /></dd>
        </template>
      </dl>

      <p
        v-if="details.plugin.source.kind === 'external'"
        class="text-muted-foreground rounded-xl border border-dashed p-6 text-center text-sm"
      >
        Claude Code fetches this plugin from elsewhere when it installs it, so its skills, agents,
        and commands are not known yet.
      </p>
      <Tabs v-else v-model="tab">
        <TabsList>
          <TabsTrigger v-if="details.readme" value="readme">README</TabsTrigger>
          <TabsTrigger value="skills">Skills {{ skills.length }}</TabsTrigger>
          <TabsTrigger value="agents">Agents {{ agents.length }}</TabsTrigger>
          <TabsTrigger value="commands">Commands {{ commands.length }}</TabsTrigger>
        </TabsList>
        <TabsContent v-if="details.readme" value="readme" class="rounded-xl border p-6">
          <MarkdownContent :source="details.readme" />
        </TabsContent>
        <TabsContent value="skills">
          <ComponentList :items="skills" empty="No skills." />
        </TabsContent>
        <TabsContent value="agents">
          <ComponentList :items="agents" empty="No agents." />
        </TabsContent>
        <TabsContent value="commands">
          <ComponentList :items="commands" empty="No commands." />
        </TabsContent>
      </Tabs>
    </template>
  </div>
</template>
