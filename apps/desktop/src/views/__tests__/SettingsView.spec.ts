import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { open as openFolderPicker } from '@tauri-apps/plugin-dialog'
import { createPinia } from 'pinia'
import { createMemoryHistory, createRouter } from 'vue-router'

import { addProject, getGitHubStatus, listProjects, removeProject, type Project } from '@/lib/api'
import SettingsView from '../SettingsView.vue'

vi.mock('@/lib/api', () => ({
  listProjects: vi.fn<typeof listProjects>(),
  addProject: vi.fn<typeof addProject>(),
  removeProject: vi.fn<typeof removeProject>(),
  // GitHubSettings has tests of its own.
  getGitHubStatus: vi.fn<typeof getGitHubStatus>(() => new Promise(() => {})),
}))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn<typeof openFolderPicker>() }))

enableAutoUnmount(afterEach)

const app: Project = {
  id: 1,
  name: 'my-app',
  path: '/src/my-app',
  addedAt: '2026-10-15T12:00:00Z',
  error: null,
}
const moved: Project = {
  ...app,
  id: 2,
  name: 'old',
  path: '/src/old',
  error: 'the folder does not exist',
}

beforeEach(() => {
  vi.mocked(listProjects).mockResolvedValue([app, moved])
})

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [{ path: '/:any(.*)', component: { render: () => null } }],
  })
  const wrapper = mount(SettingsView, { global: { plugins: [router, createPinia()] } })
  await flushPromises()
  return wrapper
}

describe('SettingsView', () => {
  it('lists the projects and says which folder is gone', async () => {
    const wrapper = await mountView()

    const rows = wrapper.findAll('li')
    expect(rows.map((row) => row.get('a').text())).toEqual(['my-app', 'old'])
    expect(rows[0]!.get('a').attributes('href')).toBe('/projects/1')
    expect(rows[1]!.text()).toContain('the folder does not exist')
  })

  it('adds the folder that was typed or chosen', async () => {
    vi.mocked(openFolderPicker).mockResolvedValue('/src/new-app')
    vi.mocked(addProject).mockResolvedValue({ ...app, id: 3, name: 'new-app' })
    const wrapper = await mountView()

    await wrapper
      .findAll('button')
      .find((b) => b.text() === 'Browse…')!
      .trigger('click')
    await flushPromises()
    expect(wrapper.get<HTMLInputElement>('input').element.value).toBe('/src/new-app')
    vi.mocked(listProjects).mockResolvedValue([app, moved, { ...app, id: 3, name: 'new-app' }])
    await wrapper.get('form').trigger('submit')
    await flushPromises()

    expect(addProject).toHaveBeenCalledWith('/src/new-app')
    expect(wrapper.findAll('li')).toHaveLength(3)
    expect(wrapper.get<HTMLInputElement>('input').element.value).toBe('')
  })

  it('shows why a folder could not be added', async () => {
    vi.mocked(addProject).mockRejectedValue('/src/nope: the folder does not exist')
    const wrapper = await mountView()

    await wrapper.get('input').setValue('/src/nope')
    await wrapper.get('form').trigger('submit')
    await flushPromises()

    expect(wrapper.get('[role="alert"]').text()).toContain('the folder does not exist')
  })

  it('removes a project from the list', async () => {
    vi.mocked(removeProject).mockResolvedValue()
    const wrapper = await mountView()

    vi.mocked(listProjects).mockResolvedValue([moved])
    await wrapper.get('button[aria-label="Remove my-app"]').trigger('click')
    await flushPromises()

    expect(removeProject).toHaveBeenCalledWith(1)
    expect(wrapper.findAll('li')).toHaveLength(1)
  })
})
