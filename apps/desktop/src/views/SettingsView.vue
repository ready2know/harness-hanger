<script setup lang="ts">
import { FolderIcon, FolderOpenIcon, LoaderCircleIcon, PlusIcon, TrashIcon } from '@lucide/vue'
import { open as openFolderPicker } from '@tauri-apps/plugin-dialog'
import { storeToRefs } from 'pinia'
import { onMounted, ref } from 'vue'
import { RouterLink } from 'vue-router'

import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { useProjectsStore } from '@/stores/projects'

const store = useProjectsStore()
const { projects, error: loadError } = storeToRefs(store)
onMounted(store.load)

const path = ref('')
const adding = ref(false)
const addError = ref<string>()
const removeErrors = ref<Record<number, string>>({})

async function browse() {
  try {
    const folder = await openFolderPicker({ directory: true, title: 'Choose a project folder' })
    if (folder) path.value = folder
  } catch (e) {
    addError.value = String(e)
  }
}

async function add() {
  const trimmed = path.value.trim()
  if (!trimmed || adding.value) return
  adding.value = true
  addError.value = undefined
  try {
    await store.add(trimmed)
    path.value = ''
  } catch (e) {
    addError.value = String(e)
  } finally {
    adding.value = false
  }
}

async function remove(id: number) {
  delete removeErrors.value[id]
  try {
    await store.remove(id)
  } catch (e) {
    removeErrors.value[id] = String(e)
  }
}
</script>

<template>
  <div class="mx-auto flex w-full max-w-4xl flex-col gap-6 p-8">
    <h1 class="text-2xl font-semibold">Settings</h1>

    <section class="flex flex-col gap-3" aria-labelledby="projects-heading">
      <div class="flex flex-col gap-1">
        <h2 id="projects-heading" class="text-lg font-semibold">Projects</h2>
        <p class="text-muted-foreground text-sm">
          Folders whose Claude Code plugins you want to see and change. They appear in the sidebar.
          Removing one here leaves the folder as it is.
        </p>
      </div>

      <form class="flex gap-2" @submit.prevent="add">
        <Input
          v-model="path"
          aria-label="Project folder"
          placeholder="/path/to/project"
          :disabled="adding"
        />
        <Button type="button" variant="outline" :disabled="adding" @click="browse">
          <FolderOpenIcon />
          Browse…
        </Button>
        <Button type="submit" :disabled="adding || !path.trim()">
          <LoaderCircleIcon v-if="adding" class="animate-spin" />
          <PlusIcon v-else />
          Add
        </Button>
      </form>
      <p v-if="addError" role="alert" class="text-destructive text-sm wrap-anywhere">
        {{ addError }}
      </p>

      <p v-if="loadError" role="alert" class="text-destructive wrap-anywhere">{{ loadError }}</p>
      <p v-else-if="!projects" role="status" class="text-muted-foreground">Loading…</p>
      <p v-else-if="projects.length === 0" class="text-muted-foreground text-sm">
        No projects yet.
      </p>
      <ul v-else class="divide-y rounded-xl border">
        <li v-for="project in projects" :key="project.id" class="flex items-center gap-3 px-4 py-3">
          <FolderIcon class="text-muted-foreground size-4 shrink-0" />
          <div class="flex min-w-0 flex-1 flex-col">
            <RouterLink
              :to="`/projects/${project.id}`"
              class="w-fit font-medium underline-offset-4 hover:underline"
            >
              {{ project.name }}
            </RouterLink>
            <span class="text-muted-foreground truncate font-mono text-xs" :title="project.path">
              {{ project.path }}
            </span>
            <span v-if="project.error" class="text-destructive text-xs">{{ project.error }}</span>
            <span
              v-if="removeErrors[project.id]"
              role="alert"
              class="text-destructive text-xs wrap-anywhere"
            >
              {{ removeErrors[project.id] }}
            </span>
          </div>
          <Button
            variant="ghost"
            size="icon-sm"
            :aria-label="`Remove ${project.name}`"
            @click="remove(project.id)"
          >
            <TrashIcon />
          </Button>
        </li>
      </ul>
    </section>
  </div>
</template>
