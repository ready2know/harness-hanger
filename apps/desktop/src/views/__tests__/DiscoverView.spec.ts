import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { enableAutoUnmount, flushPromises, mount, type VueWrapper } from '@vue/test-utils'

import {
  addMarketplace,
  discoverMarketplaces,
  type DiscoverResults,
  type DiscoveredMarketplace,
  type MarketplaceSummary,
} from '@/lib/api'
import DiscoverView from '../DiscoverView.vue'
import { stubRouter } from './stubRouter'

vi.mock('@/lib/api', () => ({
  discoverMarketplaces: vi.fn<typeof discoverMarketplaces>(),
  addMarketplace: vi.fn<typeof addMarketplace>(),
}))

enableAutoUnmount(afterEach)

function found(overrides: Partial<DiscoveredMarketplace>): DiscoveredMarketplace {
  return {
    owner: 'acme',
    repo: 'tools',
    url: 'https://github.com/acme/tools',
    description: 'Tools by Acme',
    stars: 1234,
    pushedAt: '2026-10-01T12:00:00Z',
    license: 'MIT',
    archived: false,
    name: 'acme-tools',
    pluginCount: 2,
    plugins: [
      {
        name: 'reviewer',
        description: 'Reviews code',
        version: null,
        category: null,
        keywords: [],
      },
      { name: 'formatter', description: null, version: '1.0.0', category: null, keywords: [] },
    ],
    fetchedAt: '2026-10-16T12:00:00Z',
    curated: false,
    catalogId: null,
    matchingPlugins: [],
    copies: [],
    ...overrides,
  }
}

const official = found({
  owner: 'anthropics',
  repo: 'claude-plugins-official',
  url: 'https://github.com/anthropics/claude-plugins-official',
  description: 'Official plugins.',
  stars: 39819,
  pushedAt: '2026-10-16T07:00:00Z',
  pluginCount: 315,
  curated: true,
})

function results(overrides: Partial<DiscoverResults>): DiscoverResults {
  return {
    query: null,
    method: 'curated',
    signedIn: true,
    account: null,
    results: [official],
    totalCount: null,
    searchedAt: null,
    fromCache: false,
    problem: null,
    rateLimit: null,
    ...overrides,
  }
}

async function mountView() {
  const router = stubRouter()
  await router.push('/discover')
  const wrapper = mount(DiscoverView, { global: { plugins: [router] }, attachTo: document.body })
  await flushPromises()
  return { wrapper, router }
}

async function searchFor(wrapper: VueWrapper, text: string) {
  await wrapper.get('input[type="search"]').setValue(text)
  await wrapper.get('form').trigger('submit')
  await flushPromises()
}

function rows(wrapper: VueWrapper) {
  return wrapper.findAll('ul.divide-y > li')
}

beforeEach(() => {
  vi.clearAllMocks()
  localStorage.clear()
  vi.mocked(discoverMarketplaces).mockResolvedValue(results({}))
})

describe('DiscoverView', () => {
  it('shows the curated marketplaces when it opens', async () => {
    const { wrapper } = await mountView()

    expect(discoverMarketplaces).toHaveBeenCalledWith('', false)
    expect(wrapper.text()).toContain('Well-known marketplaces')
    const [row] = rows(wrapper)
    expect(row!.text()).toContain('anthropics/claude-plugins-official')
    expect(row!.text()).toContain('Curated')
    expect(row!.text()).toContain('Official plugins.')
    expect(row!.text()).toContain('39,819')
    expect(row!.text()).toContain('315 plugins')
    expect(row!.text()).toContain('MIT')
  })

  it('searches GitHub and shows the plugins that match', async () => {
    const { wrapper } = await mountView()
    vi.mocked(discoverMarketplaces).mockResolvedValue(
      results({
        query: 'review',
        method: 'code',
        totalCount: 120,
        searchedAt: '2026-10-16T12:00:00Z',
        results: [found({ matchingPlugins: ['reviewer'] })],
      }),
    )

    await searchFor(wrapper, 'review')

    expect(discoverMarketplaces).toHaveBeenLastCalledWith('review', false)
    expect(wrapper.text()).toContain('GitHub found 120 manifests that match')
    expect(rows(wrapper)[0]!.text()).toContain('Matching plugins: reviewer')
  })

  it('says how many repositories with the topic match without a GitHub token', async () => {
    const { wrapper } = await mountView()
    vi.mocked(discoverMarketplaces).mockResolvedValue(
      results({ query: 'review', method: 'topic', signedIn: false, totalCount: 34 }),
    )

    await searchFor(wrapper, 'review')

    expect(wrapper.text()).toContain('GitHub found 34 repositories with the topic that match')
  })

  it('searches while the curated list is still loading, and shows only the search', async () => {
    let finishCurated!: (loaded: DiscoverResults) => void
    vi.mocked(discoverMarketplaces).mockReturnValueOnce(
      new Promise((resolve) => (finishCurated = resolve)),
    )
    const { wrapper } = await mountView()
    vi.mocked(discoverMarketplaces).mockResolvedValue(
      results({ query: 'deploy', method: 'code', results: [found({ repo: 'deployer' })] }),
    )

    await searchFor(wrapper, 'deploy')
    finishCurated(results({}))
    await flushPromises()

    expect(discoverMarketplaces).toHaveBeenLastCalledWith('deploy', false)
    expect(rows(wrapper).map((row) => row.find('a').text())).toEqual(['acme/deployer'])
  })

  it('says which account a search runs as, and that it reads every manifest', async () => {
    vi.mocked(discoverMarketplaces).mockResolvedValue(results({ account: 'ready2know' }))

    const { wrapper } = await mountView()

    const mode = wrapper.get('[data-testid="search-mode"]').text()
    expect(mode).toContain('Signed in as ready2know')
    expect(mode).toContain('.claude-plugin/marketplace.json of every repository on GitHub')
    expect(wrapper.find('a[href="/settings"]').exists()).toBe(false)
  })

  it('explains what a search finds without a GitHub token', async () => {
    vi.mocked(discoverMarketplaces).mockResolvedValue(results({ signedIn: false }))

    const { wrapper } = await mountView()

    expect(wrapper.text()).toContain('Not signed in to GitHub')
    expect(wrapper.get('a[href="/settings"]').text()).toBe('sign in on the Settings page')
  })

  it('says why GitHub cannot be reached, and keeps what was found before', async () => {
    vi.mocked(discoverMarketplaces).mockResolvedValue(
      results({ problem: { kind: 'offline', message: 'connection refused' } }),
    )

    const { wrapper } = await mountView()

    const alert = wrapper.get('[role="alert"]').text()
    expect(alert).toContain('Cannot reach GitHub: connection refused')
    expect(alert).toContain('Showing what earlier searches found')
    expect(rows(wrapper)).toHaveLength(1)
  })

  it('shows when a reached rate limit resets', async () => {
    vi.mocked(discoverMarketplaces).mockResolvedValue(
      results({
        results: [],
        problem: { kind: 'rateLimited', resetAt: '2026-10-16T12:06:40Z' },
      }),
    )

    const { wrapper } = await mountView()

    const alert = wrapper.get('[role="alert"]').text()
    expect(alert).toContain("GitHub's rate limit is reached. It resets at")
    expect(alert).toContain('2026')
    expect(wrapper.text()).not.toContain('No marketplaces found')
  })

  it('adds a marketplace in one action and opens it', async () => {
    vi.mocked(addMarketplace).mockResolvedValue({ id: 7 } as MarketplaceSummary)
    const { wrapper, router } = await mountView()

    await rows(wrapper)[0]!
      .findAll('button')
      .find((button) => button.text() === 'Add')!
      .trigger('click')
    await flushPromises()

    expect(addMarketplace).toHaveBeenCalledWith('anthropics/claude-plugins-official')
    expect(router.currentRoute.value.fullPath).toBe('/marketplace/7')
  })

  it('shows why a marketplace could not be added', async () => {
    vi.mocked(addMarketplace).mockRejectedValue('git failed: the repository does not exist')
    const { wrapper, router } = await mountView()

    await rows(wrapper)[0]!
      .findAll('button')
      .find((button) => button.text() === 'Add')!
      .trigger('click')
    await flushPromises()

    expect(rows(wrapper)[0]!.get('[role="alert"]').text()).toContain('does not exist')
    expect(router.currentRoute.value.fullPath).toBe('/discover')
  })

  it('opens a marketplace that the catalog has instead of adding it', async () => {
    vi.mocked(discoverMarketplaces).mockResolvedValue(
      results({ results: [{ ...official, catalogId: 3 }] }),
    )

    const { wrapper } = await mountView()

    const open = rows(wrapper)[0]!.get('a[href="/marketplace/3"]')
    expect(open.text()).toBe('Open')
    expect(
      rows(wrapper)[0]!
        .findAll('button')
        .some((b) => b.text() === 'Add'),
    ).toBe(false)
  })

  it('asks GitHub again on Refresh', async () => {
    const { wrapper } = await mountView()

    await wrapper
      .findAll('button')
      .find((button) => button.text() === 'Refresh')!
      .trigger('click')
    await flushPromises()

    expect(discoverMarketplaces).toHaveBeenLastCalledWith('', true)
  })

  it('counts the copies it left out', async () => {
    vi.mocked(discoverMarketplaces).mockResolvedValue(
      results({ results: [found({ copies: ['copier/tools', 'other/tools'] })] }),
    )

    const { wrapper } = await mountView()

    expect(rows(wrapper)[0]!.text()).toContain('2 copies left out')
  })
})
