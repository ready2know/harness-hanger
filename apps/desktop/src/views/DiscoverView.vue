<script setup lang="ts">
import {
  CheckIcon,
  ChevronRightIcon,
  LoaderCircleIcon,
  PlusIcon,
  RefreshCwIcon,
  SearchIcon,
  StarIcon,
  TriangleAlertIcon,
} from '@lucide/vue'
import { computed, onMounted, ref } from 'vue'
import { RouterLink, useRouter } from 'vue-router'

import ExternalLink from '@/components/ExternalLink.vue'
import PagedList from '@/components/PagedList.vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  addMarketplace,
  discoverMarketplaces,
  type DiscoverProblem,
  type DiscoverResults,
  type DiscoveredMarketplace,
} from '@/lib/api'
import { count, formatDate, formatDateTime } from '@/lib/format'

const router = useRouter()

const query = ref('')
const results = ref<DiscoverResults>()
const error = ref<string>()
const loading = ref(false)
/** The `owner/repo` being added. One at a time, because adding opens the marketplace's page. */
const adding = ref<string>()
const addErrors = ref<Record<string, string>>({})

type SortKey = 'stars' | 'updated' | 'plugins'
const sort = ref<SortKey>('stars')
const sortLabels: Record<SortKey, string> = {
  stars: 'Most stars',
  updated: 'Recently updated',
  plugins: 'Most plugins',
}

// Each request is numbered, so that a search can start while another runs, and only the
// latest one shows.
let latest = 0

async function load(text: string, refresh = false) {
  const request = ++latest
  loading.value = true
  error.value = undefined
  try {
    const loaded = await discoverMarketplaces(text, refresh)
    // A later search replaces this one.
    if (request === latest) results.value = loaded
  } catch (e) {
    if (request === latest) error.value = String(e)
  } finally {
    if (request === latest) loading.value = false
  }
}

// Opening this page is what lets harness-hanger ask GitHub, for the curated list.
onMounted(() => load(''))

function search() {
  addErrors.value = {}
  load(query.value)
}

function refresh() {
  load(results.value?.query ?? '', true)
}

// The core gives the most starred first; a stable sort keeps that order among ties.
const shown = computed(() => {
  const found = [...(results.value?.results ?? [])]
  if (sort.value === 'updated') {
    found.sort((a, b) => (b.pushedAt ?? '').localeCompare(a.pushedAt ?? ''))
  } else if (sort.value === 'plugins') {
    found.sort((a, b) => b.pluginCount - a.pluginCount)
  }
  return found
})

function fullName(found: DiscoveredMarketplace): string {
  return `${found.owner}/${found.repo}`
}

async function add(found: DiscoveredMarketplace) {
  const name = fullName(found)
  adding.value = name
  delete addErrors.value[name]
  try {
    const added = await addMarketplace(name)
    await router.push({ name: 'marketplace-preview', params: { id: added.id } })
  } catch (e) {
    addErrors.value[name] = String(e)
  } finally {
    adding.value = undefined
  }
}

function describe(problem: DiscoverProblem): string {
  switch (problem.kind) {
    case 'offline':
      return `Cannot reach GitHub: ${problem.message}`
    case 'rateLimited':
      return problem.resetAt
        ? `GitHub's rate limit is reached. It resets at ${formatDateTime(problem.resetAt)}.`
        : "GitHub's rate limit is reached. Try again later."
    case 'unauthorized':
      return `GitHub refused the token: ${problem.message}. Sign in again on the Settings page, or check GH_TOKEN and GITHUB_TOKEN.`
    case 'github':
      return `GitHub answered ${problem.status}: ${problem.message}`
  }
}
</script>

<template>
  <div class="mx-auto flex w-full max-w-4xl flex-col gap-6 p-8">
    <header class="flex flex-col gap-1">
      <h1 class="text-2xl font-semibold">Discover</h1>
      <p class="text-muted-foreground text-sm">
        Find public plugin marketplaces on GitHub and add them to your catalog. harness-hanger asks
        GitHub only from this page, and sends only what you search for.
      </p>
    </header>

    <div class="flex flex-col gap-2">
      <form role="search" class="flex gap-2" @submit.prevent="search">
        <div class="relative flex-1">
          <SearchIcon
            class="text-muted-foreground pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2"
          />
          <Input
            v-model="query"
            type="search"
            placeholder="Marketplaces and plugins, such as code review"
            aria-label="Search GitHub for marketplaces"
            class="pl-8"
          />
        </div>
        <Button type="submit">
          <LoaderCircleIcon v-if="loading" class="animate-spin" />
          Search
        </Button>
      </form>
      <p
        v-if="results?.signedIn"
        class="text-muted-foreground flex items-center gap-1.5 text-xs"
        data-testid="search-mode"
      >
        <CheckIcon class="size-3.5 shrink-0 text-emerald-600 dark:text-emerald-500" />
        <span>
          Signed in<template v-if="results.account">
            as <strong class="text-foreground">{{ results.account }}</strong></template
          >, so a search looks through the <code>.claude-plugin/marketplace.json</code> of every
          repository on GitHub.
        </span>
      </p>
      <p v-else-if="results" class="text-muted-foreground text-xs" data-testid="search-mode">
        Not signed in to GitHub, so a search finds only repositories with the
        <code>claude-code-marketplace</code> topic. To search every marketplace,
        <RouterLink :to="{ name: 'settings' }" class="text-foreground underline underline-offset-4">
          sign in on the Settings page</RouterLink
        >.
      </p>
    </div>

    <p v-if="error" role="alert" class="text-destructive wrap-anywhere">{{ error }}</p>
    <p v-else-if="!results" role="status" class="text-muted-foreground">Asking GitHub…</p>

    <template v-else>
      <div
        v-if="results.problem"
        role="alert"
        class="flex gap-3 rounded-xl border border-amber-500/40 bg-amber-500/10 p-4 text-sm"
      >
        <TriangleAlertIcon class="mt-0.5 size-4 shrink-0 text-amber-600 dark:text-amber-400" />
        <div class="flex min-w-0 flex-col gap-1">
          <p class="font-medium wrap-anywhere">{{ describe(results.problem) }}</p>
          <p class="text-muted-foreground">
            {{
              results.results.length
                ? 'Showing what earlier searches found.'
                : 'Nothing found earlier matches. The rest of harness-hanger works without GitHub.'
            }}
          </p>
        </div>
      </div>

      <div class="text-muted-foreground flex items-start justify-between gap-4 text-sm">
        <p class="min-w-0">
          <template v-if="results.method === 'curated'">
            Well-known marketplaces. Search to find more on GitHub.
          </template>
          <template v-else-if="results.totalCount !== null && results.method === 'code'">
            GitHub found {{ count(results.totalCount, 'manifest') }} that match. The first 50 are
            here, with the marketplaces from earlier searches that match.
          </template>
          <template v-else-if="results.totalCount !== null">
            GitHub found {{ results.totalCount.toLocaleString() }}
            {{ results.totalCount === 1 ? 'repository' : 'repositories' }} with the topic that
            match.
          </template>
          <template v-if="results.searchedAt">
            Searched {{ formatDateTime(results.searchedAt) }}.
          </template>
        </p>
        <Button variant="ghost" size="sm" class="shrink-0" :disabled="loading" @click="refresh">
          <RefreshCwIcon />
          Refresh
        </Button>
      </div>

      <p
        v-if="results.results.length === 0 && !results.problem"
        class="text-muted-foreground py-6 text-center"
      >
        No marketplaces found.
      </p>
      <template v-else-if="results.results.length">
        <div class="flex items-center justify-between gap-3">
          <span class="text-sm font-medium">
            {{ count(results.results.length, 'marketplace') }}
          </span>
          <Select v-model="sort">
            <SelectTrigger size="sm" aria-label="Sort by">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem v-for="(label, key) in sortLabels" :key="key" :value="key">
                {{ label }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>

        <PagedList :key="`${results.query}|${sort}`" v-slot="{ items: visible }" :items="shown">
          <ul class="divide-y rounded-xl border">
            <li
              v-for="found in visible"
              :key="fullName(found)"
              class="flex flex-col gap-2 px-4 py-3"
            >
              <div class="flex items-start justify-between gap-3">
                <div class="flex min-w-0 flex-col gap-1">
                  <div class="flex flex-wrap items-center gap-2">
                    <ExternalLink :href="found.url" class="font-medium">
                      {{ fullName(found) }}
                    </ExternalLink>
                    <Badge v-if="found.curated" variant="secondary">Curated</Badge>
                    <Badge v-if="found.archived" variant="outline">Archived</Badge>
                  </div>
                  <p v-if="found.description" class="text-muted-foreground line-clamp-2 text-sm">
                    {{ found.description }}
                  </p>
                </div>
                <Button
                  v-if="found.catalogId !== null"
                  as-child
                  variant="outline"
                  size="sm"
                  class="shrink-0"
                >
                  <RouterLink
                    :to="{ name: 'marketplace-preview', params: { id: found.catalogId } }"
                  >
                    Open
                  </RouterLink>
                </Button>
                <Button
                  v-else
                  size="sm"
                  class="shrink-0"
                  :disabled="adding !== undefined"
                  @click="add(found)"
                >
                  <LoaderCircleIcon v-if="adding === fullName(found)" class="animate-spin" />
                  <PlusIcon v-else />
                  {{ adding === fullName(found) ? 'Adding…' : 'Add' }}
                </Button>
              </div>

              <p class="text-muted-foreground flex flex-wrap items-center gap-x-2 gap-y-1 text-xs">
                <span class="flex items-center gap-1" :title="count(found.stars, 'star')">
                  <StarIcon class="size-3" />
                  {{ found.stars.toLocaleString() }}
                </span>
                <span aria-hidden="true">·</span>
                <span>{{ count(found.pluginCount, 'plugin') }}</span>
                <template v-if="found.pushedAt">
                  <span aria-hidden="true">·</span>
                  <span>Updated {{ formatDate(found.pushedAt) }}</span>
                </template>
                <template v-if="found.license">
                  <span aria-hidden="true">·</span>
                  <span>{{ found.license }}</span>
                </template>
                <template v-if="found.copies.length">
                  <span aria-hidden="true">·</span>
                  <span :title="found.copies.join('\n')">
                    {{ found.copies.length }}
                    {{ found.copies.length === 1 ? 'copy' : 'copies' }} left out
                  </span>
                </template>
              </p>

              <p v-if="found.matchingPlugins.length" class="line-clamp-2 text-xs">
                <span class="text-muted-foreground">Matching plugins:</span>
                {{ found.matchingPlugins.join(', ') }}
              </p>

              <Collapsible v-if="found.plugins.length" class="group/plugins">
                <CollapsibleTrigger
                  class="text-muted-foreground hover:text-foreground flex cursor-pointer items-center gap-1 text-xs"
                >
                  <ChevronRightIcon
                    class="size-3 transition-transform group-data-[state=open]/plugins:rotate-90"
                  />
                  Plugins
                </CollapsibleTrigger>
                <CollapsibleContent>
                  <ul class="mt-2 flex flex-col gap-1.5 border-l pl-3">
                    <li
                      v-for="(plugin, index) in found.plugins"
                      :key="`${index}/${plugin.name}`"
                      class="flex flex-col text-sm"
                    >
                      <span class="font-medium">{{ plugin.name }}</span>
                      <span v-if="plugin.description" class="text-muted-foreground line-clamp-2">
                        {{ plugin.description }}
                      </span>
                    </li>
                  </ul>
                </CollapsibleContent>
              </Collapsible>

              <p
                v-if="addErrors[fullName(found)]"
                role="alert"
                class="text-destructive text-sm wrap-anywhere"
              >
                {{ addErrors[fullName(found)] }}
              </p>
            </li>
          </ul>
        </PagedList>
      </template>
    </template>
  </div>
</template>
