import { defineStore } from 'pinia'
import { ref } from 'vue'

import { addProject, listProjects, removeProject, type Project } from '@/lib/api'

/** The project folders, shared by the sidebar, the Settings page, and the project pages. */
export const useProjectsStore = defineStore('projects', () => {
  const projects = ref<Project[]>()
  const error = ref<string>()

  async function load() {
    try {
      projects.value = await listProjects()
      error.value = undefined
    } catch (e) {
      error.value = String(e)
    }
  }

  async function add(path: string): Promise<Project> {
    const project = await addProject(path)
    await load()
    return project
  }

  async function remove(id: number) {
    await removeProject(id)
    await load()
  }

  return { projects, error, load, add, remove }
})
