import { afterEach, describe, expect, it, vi } from 'vitest'
import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'

import { listCatalogPlugins } from '@/lib/api'
import PluginsView from '../PluginsView.vue'
import { catalogPlugins, movedMarketplace } from './catalogFixtures'
import { stubRouter } from './stubRouter'

vi.mock('@/lib/api', () => ({ listCatalogPlugins: vi.fn<typeof listCatalogPlugins>() }))

enableAutoUnmount(afterEach)

async function mountView() {
  const router = stubRouter()
  const wrapper = mount(PluginsView, { global: { plugins: [router] } })
  await flushPromises()
  return wrapper
}

describe('PluginsView', () => {
  it('lists the plugins with their status, marketplace, and contents', async () => {
    vi.mocked(listCatalogPlugins).mockResolvedValue({ plugins: catalogPlugins, unreadable: [] })

    const wrapper = await mountView()

    const rows = wrapper.findAll('li')
    expect(rows.map((row) => row.get('.font-medium').text())).toEqual([
      'deploy',
      'docs',
      'remote',
      'review',
    ])
    const review = rows[3]!
    expect(review.text()).toContain('1.2.0')
    expect(review.text()).toContain('Enabled')
    expect(review.text()).toContain('2 skills · 1 agent')
    expect(review.findAll('a').map((link) => link.attributes('href'))).toEqual([
      '/plugins/1/review',
      '/marketplace/1',
    ])
    expect(rows[2]!.text()).toContain('Installed from elsewhere; contents unknown')
    expect(wrapper.findAll('[data-slot="toggle-group-item"]').map((item) => item.text())).toEqual([
      'All 4',
      'Enabled 1',
      'Disabled 1',
      'Not installed 2',
    ])
  })

  it('points to the Marketplaces page when there is nothing to list', async () => {
    vi.mocked(listCatalogPlugins).mockResolvedValue({ plugins: [], unreadable: [] })

    const wrapper = await mountView()

    expect(wrapper.text()).toContain('No plugins yet')
  })

  it('reports an unreadable marketplace instead of claiming that nothing was added', async () => {
    vi.mocked(listCatalogPlugins).mockResolvedValue({ plugins: [], unreadable: [movedMarketplace] })

    const wrapper = await mountView()

    expect(wrapper.get('[role="alert"]').text()).toContain('/src/moved')
    expect(wrapper.text()).not.toContain('No plugins yet')
  })

  it("leaves out a harness's own components, which are not a plugin", async () => {
    vi.mocked(listCatalogPlugins).mockResolvedValue({
      plugins: [
        ...catalogPlugins,
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

    const wrapper = await mountView()

    expect(wrapper.findAll('li')).toHaveLength(4)
    expect(wrapper.text()).not.toContain('claude-code')
  })
})
