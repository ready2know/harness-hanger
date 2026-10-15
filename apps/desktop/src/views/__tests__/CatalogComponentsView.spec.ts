import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { enableAutoUnmount, flushPromises, mount, type VueWrapper } from '@vue/test-utils'

import { listCatalogPlugins } from '@/lib/api'
import CatalogComponentsView from '../CatalogComponentsView.vue'
import { catalogPlugins, movedMarketplace } from './catalogFixtures'
import { stubRouter } from './stubRouter'

vi.mock('@/lib/api', () => ({ listCatalogPlugins: vi.fn<typeof listCatalogPlugins>() }))

enableAutoUnmount(afterEach)

beforeEach(() => {
  vi.mocked(listCatalogPlugins).mockResolvedValue({ plugins: catalogPlugins, unreadable: [] })
})

async function mountView(kind: 'skills' | 'agents') {
  const router = stubRouter()
  const wrapper = mount(CatalogComponentsView, {
    props: { kind },
    global: { plugins: [router] },
  })
  await flushPromises()
  return wrapper
}

function rows(wrapper: VueWrapper) {
  return wrapper.findAll('li').map((row) => ({
    name: row.get('.font-mono').text(),
    status: row.get('[data-slot="badge"]').text(),
  }))
}

function filters(wrapper: VueWrapper) {
  return wrapper.findAll('[data-slot="toggle-group-item"]').map((item) => item.text())
}

describe('CatalogComponentsView', () => {
  it("lists every plugin's skills, sorted, each with its plugin's status", async () => {
    const wrapper = await mountView('skills')

    expect(wrapper.get('h1').text()).toBe('Skills')
    expect(rows(wrapper)).toEqual([
      { name: 'deploy-app', status: 'Not installed' },
      { name: 'lint', status: 'Enabled' },
      { name: 'review-pr', status: 'Enabled' },
      { name: 'write-docs', status: 'Disabled' },
    ])
    const links = wrapper.findAll('li')[0]!.findAll('a')
    expect(links.map((link) => link.attributes('href'))).toEqual([
      '/skills/2/deploy/deploy-app',
      '/plugins/2/deploy',
      '/marketplace/2',
    ])
  })

  it('filters by status and counts each status', async () => {
    const wrapper = await mountView('skills')
    expect(filters(wrapper)).toEqual(['All 4', 'Enabled 2', 'Disabled 1', 'Not installed 1'])

    await wrapper.findAll('[data-slot="toggle-group-item"]')[1]!.trigger('click')

    expect(rows(wrapper).map((row) => row.name)).toEqual(['lint', 'review-pr'])
  })

  it('searches names, descriptions, plugins, and marketplaces', async () => {
    const wrapper = await mountView('skills')

    await wrapper.get('input[aria-label="Search"]').setValue('tools')

    expect(rows(wrapper).map((row) => row.name)).toEqual(['deploy-app'])
    expect(filters(wrapper)).toEqual(['All 1', 'Enabled 0', 'Disabled 0', 'Not installed 1'])
  })

  it('lists agents, and starts over when switching from skills', async () => {
    const wrapper = await mountView('skills')
    await wrapper.get('input[aria-label="Search"]').setValue('deploy')

    await wrapper.setProps({ kind: 'agents' })

    expect(wrapper.get('h1').text()).toBe('Agents')
    expect(rows(wrapper)).toEqual([{ name: 'security', status: 'Enabled' }])
    expect(wrapper.get<HTMLInputElement>('input[aria-label="Search"]').element.value).toBe('')
  })

  it('shows the skills a page at a time, and starts again at the first page on a new search', async () => {
    const skills = Array.from({ length: 30 }, (_, index) => ({
      name: `skill-${String(index).padStart(2, '0')}`,
      description: 'A skill.',
      path: `/m/skill-${index}/SKILL.md`,
    }))
    const review = catalogPlugins[0]!
    vi.mocked(listCatalogPlugins).mockResolvedValue({
      plugins: [{ ...review, plugin: { ...review.plugin, skills } }],
      unreadable: [],
    })
    const wrapper = await mountView('skills')
    expect(rows(wrapper)).toHaveLength(10)

    await wrapper
      .findAll('[data-slot="pagination-item"]')
      .find((item) => item.text() === '3')!
      .trigger('click')
    expect(rows(wrapper).map((row) => row.name)).toEqual(
      skills.slice(20).map((skill) => skill.name),
    )

    await wrapper.get('input[aria-label="Search"]').setValue('skill-')
    expect(rows(wrapper)[0]!.name).toBe('skill-00')
  })

  it('points to the Marketplaces page when there is nothing to list', async () => {
    vi.mocked(listCatalogPlugins).mockResolvedValue({ plugins: [], unreadable: [] })

    const wrapper = await mountView('agents')

    expect(wrapper.text()).toContain('No agents yet')
    expect(wrapper.get('a').attributes('href')).toBe('/marketplace')
  })

  it('says when the plugins include none, instead of asking to add marketplaces', async () => {
    vi.mocked(listCatalogPlugins).mockResolvedValue({
      plugins: catalogPlugins.filter((entry) => entry.plugin.agents.length === 0),
      unreadable: [],
    })

    const wrapper = await mountView('agents')

    expect(wrapper.text()).toContain('The plugins in your marketplaces include no agents')
    expect(wrapper.text()).not.toContain('No agents yet')
  })

  it('names the marketplaces that could not be read', async () => {
    vi.mocked(listCatalogPlugins).mockResolvedValue({
      plugins: catalogPlugins,
      unreadable: [movedMarketplace],
    })

    const wrapper = await mountView('skills')

    const notice = wrapper.get('[role="alert"]')
    expect(notice.text()).toContain(
      'Some skills are missing, because 1 marketplace could not be read',
    )
    expect(notice.text()).toContain('the directory does not exist')
    expect(notice.get('a').attributes('href')).toBe('/marketplace/3')
    expect(wrapper.findAll('li .font-mono')).toHaveLength(4)
  })

  it("labels a harness's own skills as standalone", async () => {
    vi.mocked(listCatalogPlugins).mockResolvedValue({
      plugins: [
        {
          marketplaceId: 9,
          marketplace: 'Claude Code',
          status: 'enabled',
          plugin: {
            ...catalogPlugins[0]!.plugin,
            name: 'claude-code',
            source: { kind: 'harness', harness: 'claude-code' },
          },
        },
      ],
      unreadable: [],
    })

    const wrapper = await mountView('skills')

    expect(wrapper.get('li').text()).toContain('Standalone in Claude Code')
    expect(wrapper.get('li').text()).not.toContain('claude-code ·')
    // The page of a harness's own skill goes through the harness's ID.
    expect(wrapper.get('li a').attributes('href')).toBe('/skills/9/claude-code/lint')
  })

  it('shows why the catalog could not be read', async () => {
    vi.mocked(listCatalogPlugins).mockRejectedValue('settings.json: invalid JSON')

    const wrapper = await mountView('skills')

    expect(wrapper.get('[role="alert"]').text()).toContain('invalid JSON')
  })
})
