<script setup lang="ts">
import { ArrowLeftIcon } from '@lucide/vue'
import { computed, ref, watch } from 'vue'
import { RouterLink, type RouteLocationRaw } from 'vue-router'

import LocationActions from '@/components/LocationActions.vue'
import MarkdownContent from '@/components/MarkdownContent.vue'
import PluginStatusBadge from '@/components/PluginStatusBadge.vue'
import SourceIcon from '@/components/SourceIcon.vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { getComponentDetails, type ComponentDetails, type ComponentKind } from '@/lib/api'
import { formatLocation, frontmatterFields, frontmatterTags, relativePath } from '@/lib/format'
import { pluginRoute } from '@/lib/routes'

/** `plugin` is the plugin's name, or a harness's ID for the components it loads on its own. */
const props = defineProps<{
  kind: ComponentKind
  marketplaceId: number
  plugin: string
  name: string
}>()

const details = ref<ComponentDetails>()
const error = ref<string>()
const tab = ref('preview')

// The router reuses this view from one component to the next.
watch(
  () => [props.kind, props.marketplaceId, props.plugin, props.name] as const,
  async ([kind, marketplaceId, plugin, name], _, onCleanup) => {
    let stale = false
    // A late answer for the previous component must not show under this one.
    onCleanup(() => (stale = true))
    details.value = undefined
    error.value = undefined
    tab.value = 'preview'
    try {
      const loaded = await getComponentDetails(marketplaceId, plugin, kind, name)
      if (!stale) details.value = loaded
    } catch (e) {
      if (!stale) error.value = String(e)
    }
  },
  { immediate: true },
)

const kindLabels: Record<ComponentKind, string> = {
  skill: 'Skill',
  agent: 'Agent',
  command: 'Command',
}

/** Skills and agents have a list of their own. Commands are listed on their marketplace's page. */
const back = computed((): { to: RouteLocationRaw; label: string } => {
  if (props.kind === 'skill') return { to: { name: 'skills' }, label: 'Skills' }
  if (props.kind === 'agent') return { to: { name: 'agents' }, label: 'Agents' }
  return {
    to: { name: 'marketplace-preview', params: { id: props.marketplaceId } },
    label: details.value?.marketplace.name ?? 'Marketplace',
  }
})

const standalone = computed(() => details.value?.plugin.source.kind === 'harness')
const frontmatter = computed(() => details.value?.file?.frontmatter ?? {})
const tags = computed(() => frontmatterTags(frontmatter.value))
const fields = computed(() => frontmatterFields(frontmatter.value))
</script>

<template>
  <div class="mx-auto flex w-full max-w-4xl flex-col gap-6 p-8">
    <Button as-child variant="ghost" size="sm" class="-ml-2 self-start">
      <RouterLink :to="back.to">
        <ArrowLeftIcon />
        {{ back.label }}
      </RouterLink>
    </Button>

    <p v-if="error" role="alert" class="text-destructive wrap-anywhere">{{ error }}</p>
    <p v-else-if="!details" role="status" class="text-muted-foreground">Loading…</p>

    <template v-else>
      <header class="flex flex-col gap-3">
        <div class="flex items-start justify-between gap-4">
          <div class="flex min-w-0 flex-col gap-1">
            <p class="text-muted-foreground text-xs font-medium tracking-wide uppercase">
              {{ kindLabels[kind] }}
            </p>
            <h1 class="font-mono text-2xl font-semibold break-all">{{ details.component.name }}</h1>
          </div>
          <PluginStatusBadge :status="details.status" />
        </div>
        <p v-if="details.component.description" class="text-muted-foreground">
          {{ details.component.description }}
        </p>
        <div v-if="tags.length" class="flex flex-wrap gap-1.5" aria-label="Tags">
          <Badge v-for="tag in tags" :key="tag" variant="secondary">{{ tag }}</Badge>
        </div>
        <LocationActions :path="details.component.path" :web-url="details.webUrl" />
      </header>

      <dl
        class="grid grid-cols-[max-content_minmax(0,1fr)] gap-x-6 gap-y-3 rounded-xl border p-4 text-sm"
      >
        <dt class="text-muted-foreground">Plugin</dt>
        <dd v-if="standalone">Standalone: {{ details.marketplace.name }} loads it on its own</dd>
        <dd v-else class="flex flex-wrap items-center gap-2">
          <RouterLink
            :to="pluginRoute(details.marketplace.id, details.plugin.name)"
            class="font-medium underline-offset-4 hover:underline"
          >
            {{ details.plugin.name }}
          </RouterLink>
          <Badge v-if="details.plugin.version" variant="secondary">
            {{ details.plugin.version }}
          </Badge>
        </dd>

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

        <dt class="text-muted-foreground">File</dt>
        <dd
          v-if="details.component.path"
          class="font-mono text-xs leading-5 break-all"
          :title="details.component.path"
        >
          {{ relativePath(details.component.path, details.marketplace.root) }}
        </dd>
        <dd v-else>Written inline in the marketplace manifest</dd>

        <template v-for="field in fields" :key="field.key">
          <dt class="text-muted-foreground font-mono text-xs leading-5">{{ field.key }}</dt>
          <dd v-if="field.kind === 'list'" class="flex flex-wrap gap-1.5">
            <Badge v-for="(item, index) in field.items" :key="index" variant="outline">
              {{ item }}
            </Badge>
          </dd>
          <dd v-else-if="field.kind === 'text'" class="whitespace-pre-wrap wrap-anywhere">
            {{ field.text }}
          </dd>
          <dd v-else>
            <pre class="bg-muted overflow-x-auto rounded-md p-2 font-mono text-xs">{{
              field.json
            }}</pre>
          </dd>
        </template>
      </dl>

      <p
        v-if="details.fileError"
        role="alert"
        class="bg-destructive/10 text-destructive rounded-xl p-4 text-sm wrap-anywhere"
      >
        Cannot read the file: {{ details.fileError }}
      </p>
      <Tabs v-else-if="details.file" v-model="tab">
        <TabsList>
          <TabsTrigger value="preview">Preview</TabsTrigger>
          <TabsTrigger value="source">Source</TabsTrigger>
        </TabsList>
        <TabsContent value="preview" class="rounded-xl border p-6">
          <MarkdownContent v-if="details.file.body.trim()" :source="details.file.body" />
          <p v-else class="text-muted-foreground text-sm">
            The file has no text after its frontmatter.
          </p>
        </TabsContent>
        <TabsContent value="source">
          <pre
            class="bg-muted/50 overflow-x-auto rounded-xl border p-4 font-mono text-xs leading-relaxed"
          ><code>{{ details.file.text }}</code></pre>
        </TabsContent>
      </Tabs>
    </template>
  </div>
</template>
