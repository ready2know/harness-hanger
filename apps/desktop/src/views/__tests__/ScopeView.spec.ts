import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { enableAutoUnmount, flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { createPinia } from 'pinia'

import {
  getScopeState,
  installPlugin,
  listCatalogPlugins,
  listProjects,
  setPluginEnabled,
  uninstallPlugin,
  type CatalogPlugin,
  type ScopedPlugin,
  type ScopeState,
} from '@/lib/api'
import ScopeView from '../ScopeView.vue'
import { catalogPlugins } from './catalogFixtures'

vi.mock('@/lib/api', () => ({
  getScopeState: vi.fn<typeof getScopeState>(),
  listCatalogPlugins: vi.fn<typeof listCatalogPlugins>(),
  listProjects: vi.fn<typeof listProjects>(),
  setPluginEnabled: vi.fn<typeof setPluginEnabled>(),
  installPlugin: vi.fn<typeof installPlugin>(),
  uninstallPlugin: vi.fn<typeof uninstallPlugin>(),
}))

enableAutoUnmount(afterEach)

function plugin(overrides: Partial<ScopedPlugin>): ScopedPlugin {
  return {
    id: 'x@m',
    name: 'x',
    marketplace: 'm',
    installedIn: 'user',
    enabled: true,
    decidedBy: 'user',
    version: null,
    description: null,
    ...overrides,
  }
}

const projectState: ScopeState = {
  project: '/src/my-app',
  plugins: [
    plugin({ id: 'review@acme', name: 'review', marketplace: 'acme' }),
    plugin({
      id: 'docs@acme',
      name: 'docs',
      marketplace: 'acme',
      enabled: false,
      decidedBy: 'local',
    }),
    plugin({
      id: 'lint@tools',
      name: 'lint',
      marketplace: 'tools',
      installedIn: 'project',
      decidedBy: 'project',
    }),
  ],
  standalone: {
    ...catalogPlugins[0]!.plugin,
    name: 'claude-code',
    source: { kind: 'harness', harness: 'claude-code' },
    skills: [
      { name: 'deploy', description: 'Deploys.', path: '/src/my-app/.claude/skills/deploy' },
    ],
    agents: [],
  },
  warnings: [],
}

beforeEach(() => {
  vi.clearAllMocks()
  vi.mocked(listProjects).mockResolvedValue([
    { id: 4, name: 'my-app', path: '/src/my-app', addedAt: '2026-10-15T12:00:00Z', error: null },
  ])
  vi.mocked(getScopeState).mockResolvedValue(projectState)
  // review@acme is the catalog's enabled plugin, so it must not be offered for install.
  vi.mocked(listCatalogPlugins).mockResolvedValue({ plugins: catalogPlugins, unreadable: [] })
})

async function mountView(projectId: number | null) {
  const wrapper = mount(ScopeView, {
    props: { projectId },
    global: { plugins: [createPinia()] },
    attachTo: document.body,
  })
  await flushPromises()
  return wrapper
}

function section(wrapper: VueWrapper, heading: string) {
  return wrapper.get(`section[aria-labelledby="${heading}"]`)
}

function row(wrapper: VueWrapper, name: string) {
  return section(wrapper, 'plugins-heading')
    .findAll('li')
    .find((item) => item.get('.font-medium').text() === name)!
}

describe('ScopeView', () => {
  it('shows the global setup, and switches its plugins for the user', async () => {
    vi.mocked(getScopeState).mockResolvedValue({
      ...projectState,
      project: null,
      plugins: [
        plugin({ id: 'review@acme', name: 'review' }),
        plugin({ id: 'docs@acme', name: 'docs', enabled: false, decidedBy: null }),
      ],
    })
    vi.mocked(setPluginEnabled).mockResolvedValue()

    const wrapper = await mountView(null)

    expect(getScopeState).toHaveBeenCalledWith(null)
    expect(wrapper.get('h1').text()).toBe('Global')
    const review = row(wrapper, 'review').get('[role="switch"]')
    expect(review.attributes('aria-checked')).toBe('true')
    expect(review.attributes('aria-label')).toBe('review on for your user')
    expect(row(wrapper, 'docs').get('[role="switch"]').attributes('aria-checked')).toBe('false')
    expect(row(wrapper, 'docs').text()).toContain('Off: no settings switch it on')
    // A plugin installed for the user is not the Global page's to uninstall.
    expect(row(wrapper, 'review').find('button[aria-label="Uninstall review"]').exists()).toBe(
      false,
    )
    expect(wrapper.find('section[aria-labelledby="install-heading"]').exists()).toBe(false)
    expect(listCatalogPlugins).not.toHaveBeenCalled()

    await row(wrapper, 'docs').get('[role="switch"]').trigger('click')
    await flushPromises()

    expect(setPluginEnabled).toHaveBeenCalledWith(null, 'docs@acme', true)
    expect(getScopeState).toHaveBeenCalledTimes(2)
    expect(getScopeState).toHaveBeenLastCalledWith(null)
  })

  it('shows the global plugins 5 at a time, and stays on the page after a switch', async () => {
    const names = Array.from({ length: 15 }, (_, index) => `p${String(index).padStart(2, '0')}`)
    // Each reload gives a new list, as Claude Code's files are read again.
    vi.mocked(getScopeState).mockImplementation(async () => ({
      ...projectState,
      project: null,
      plugins: names.map((name) => plugin({ id: `${name}@acme`, name })),
    }))
    vi.mocked(setPluginEnabled).mockResolvedValue()
    const wrapper = await mountView(null)
    const plugins = section(wrapper, 'plugins-heading')
    const shown = () => plugins.findAll('li').map((item) => item.get('.font-medium').text())
    expect(shown()).toEqual(names.slice(0, 5))

    await plugins
      .findAll('[data-slot="pagination-item"]')
      .find((item) => item.text() === '2')!
      .trigger('click')
    expect(shown()).toEqual(names.slice(5, 10))

    await row(wrapper, 'p07').get('[role="switch"]').trigger('click')
    await flushPromises()

    expect(setPluginEnabled).toHaveBeenCalledWith(null, 'p07@acme', false)
    expect(getScopeState).toHaveBeenCalledTimes(2)
    expect(shown()).toEqual(names.slice(5, 10))
  })

  it("shows a project's plugins, where each comes from, and what decides it", async () => {
    const wrapper = await mountView(4)

    expect(getScopeState).toHaveBeenCalledWith(4)
    expect(wrapper.get('h1').text()).toBe('my-app')
    expect(row(wrapper, 'review').text()).toContain('Global')
    expect(row(wrapper, 'docs').text()).toContain('switched off here only for you')
    expect(row(wrapper, 'lint').text()).toContain(
      'Installed for everyone who works on this project',
    )
    // Only a plugin installed in the project can be uninstalled from it.
    expect(row(wrapper, 'review').find('button[aria-label="Uninstall review"]').exists()).toBe(
      false,
    )
    expect(row(wrapper, 'lint').find('button[aria-label="Uninstall lint"]').exists()).toBe(true)
    expect(section(wrapper, 'standalone-heading').text()).toContain('deploy')
  })

  it('switches a plugin through Claude Code, then shows the state it left', async () => {
    vi.mocked(setPluginEnabled).mockResolvedValue()
    const wrapper = await mountView(4)

    await row(wrapper, 'review').get('[role="switch"]').trigger('click')
    await flushPromises()

    expect(setPluginEnabled).toHaveBeenCalledWith(4, 'review@acme', false)
    expect(getScopeState).toHaveBeenCalledTimes(2)
  })

  it('shows why a change failed', async () => {
    vi.mocked(setPluginEnabled).mockRejectedValue('Claude Code: cannot find the claude command')
    const wrapper = await mountView(4)

    await row(wrapper, 'review').get('[role="switch"]').trigger('click')
    await flushPromises()

    expect(row(wrapper, 'review').get('[role="alert"]').text()).toContain('claude command')
  })

  it('offers only catalog plugins that are not installed globally or here', async () => {
    vi.mocked(installPlugin).mockResolvedValue()
    const extra: CatalogPlugin = {
      ...catalogPlugins[1]!,
      plugin: { ...catalogPlugins[1]!.plugin, name: 'lint' },
    }
    vi.mocked(listCatalogPlugins).mockResolvedValue({
      plugins: [...catalogPlugins, extra],
      unreadable: [],
    })
    const wrapper = await mountView(4)

    const install = section(wrapper, 'install-heading')
    const offered = install.findAll('li').map((item) => item.get('.font-medium').text())
    // review is enabled and docs disabled globally; lint@tools is installed in the project.
    expect(offered).toEqual(['deploy', 'remote'])

    await install.findAll('li')[0]!.get('button').trigger('click')
    await flushPromises()
    expect(installPlugin).toHaveBeenCalledWith(4, 2, 'deploy')
  })

  it('asks before uninstalling a plugin that everyone in the project shares', async () => {
    vi.mocked(uninstallPlugin).mockResolvedValue()
    const wrapper = await mountView(4)

    await row(wrapper, 'lint').get('button[aria-label="Uninstall lint"]').trigger('click')
    await flushPromises()
    const dialog = document.body.querySelector('[data-slot="dialog-content"]')!
    expect(dialog.textContent).toContain('removes it from the shared')
    expect(uninstallPlugin).not.toHaveBeenCalled()
    ;[...dialog.querySelectorAll('button')]
      .find((b) => b.textContent?.trim() === 'Uninstall')!
      .click()
    await flushPromises()

    expect(uninstallPlugin).toHaveBeenCalledWith(4, 'lint@tools')
  })

  it("keeps a change's error with its project when another one is opened", async () => {
    let fail!: (reason: string) => void
    vi.mocked(setPluginEnabled).mockReturnValue(new Promise((_, reject) => (fail = reject)))
    const wrapper = await mountView(4)

    await row(wrapper, 'review').get('[role="switch"]').trigger('click')
    await wrapper.setProps({ projectId: 5 })
    await flushPromises()
    fail('Claude Code: still on')
    await flushPromises()

    // review@acme is global, so project 5 has a row with the same ID.
    expect(row(wrapper, 'review').find('[role="alert"]').exists()).toBe(false)
    expect(row(wrapper, 'review').get('[role="switch"]').attributes('disabled')).toBeUndefined()
  })
})
