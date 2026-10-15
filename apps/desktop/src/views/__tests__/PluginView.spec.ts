import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { enableAutoUnmount, flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { openUrl, revealItemInDir } from '@tauri-apps/plugin-opener'

import { getPluginDetails, type PluginDetails } from '@/lib/api'
import PluginView from '../PluginView.vue'
import { plugin } from './catalogFixtures'
import { field } from './queries'
import { stubRouter } from './stubRouter'

vi.mock('@/lib/api', () => ({ getPluginDetails: vi.fn<typeof getPluginDetails>() }))
vi.mock('@tauri-apps/plugin-opener', () => ({
  openUrl: vi.fn<typeof openUrl>(),
  revealItemInDir: vi.fn<typeof revealItemInDir>(),
}))

enableAutoUnmount(afterEach)

beforeEach(() => vi.clearAllMocks())

const details: PluginDetails = {
  marketplace: {
    id: 1,
    source: { kind: 'local', path: '/src/acme' },
    addedAt: '2026-10-15T12:00:00Z',
    root: '/src/acme',
    name: 'acme',
    description: null,
    pluginCount: 1,
    error: null,
  },
  status: 'disabled',
  plugin: plugin({
    name: 'review',
    version: '1.2.0',
    description: 'Code review helpers.',
    author: 'Acme',
    category: 'development',
    keywords: ['review', 'quality'],
    homepage: 'https://example.com/review',
    repository: 'git@example.com:acme/review.git',
    license: 'MIT',
    source: { kind: 'local', path: '/src/acme/plugins/review' },
    skills: [{ name: 'review-pr', description: 'Reviews a pull request.', path: '/a/SKILL.md' }],
    agents: [{ name: 'security', description: 'Finds security bugs.', path: '/b.md' }],
  }),
  readme: '# Review\n\nHelps you review code.\n',
  webUrl: null,
}

async function mountView(loaded: PluginDetails = details) {
  vi.mocked(getPluginDetails).mockResolvedValue(loaded)
  const wrapper = mount(PluginView, {
    props: { marketplaceId: 1, name: 'review' },
    global: { plugins: [stubRouter()] },
  })
  await flushPromises()
  return wrapper
}

async function openTab(wrapper: VueWrapper, label: string) {
  await wrapper
    .findAll('[role="tab"]')
    .find((tab) => tab.text().startsWith(label))!
    .trigger('mousedown', { button: 0 })
}

describe('PluginView', () => {
  it('shows the plugin with its metadata and README', async () => {
    const wrapper = await mountView()

    expect(getPluginDetails).toHaveBeenCalledWith(1, 'review')
    expect(wrapper.get('h1').text()).toBe('review')
    expect(wrapper.text()).toContain('1.2.0')
    expect(wrapper.text()).toContain('Disabled')
    expect(
      wrapper
        .get('[aria-label="Tags"]')
        .findAll('[data-slot="badge"]')
        .map((badge) => badge.text()),
    ).toEqual(['review', 'quality'])
    expect(field(wrapper, 'Marketplace')).toBe('acme /src/acme')
    expect(field(wrapper, 'Folder')).toBe('plugins/review')
    expect(field(wrapper, 'Author')).toBe('Acme')
    expect(field(wrapper, 'Category')).toBe('development')
    expect(field(wrapper, 'License')).toBe('MIT')
    expect(wrapper.findAll('[role="tab"]').map((tab) => tab.text())).toEqual([
      'README',
      'Skills 1',
      'Agents 1',
      'Commands 0',
    ])
    expect(wrapper.get('.prose h1').text()).toBe('Review')
  })

  it('opens web addresses in the browser, and shows other addresses as text', async () => {
    const wrapper = await mountView()

    await wrapper.get('a[href="https://example.com/review"]').trigger('click')

    expect(openUrl).toHaveBeenCalledWith('https://example.com/review')
    expect(field(wrapper, 'Repository')).toBe('git@example.com:acme/review.git')
    expect(wrapper.find('a[href="git@example.com:acme/review.git"]').exists()).toBe(false)
  })

  it('links each component to its page', async () => {
    const wrapper = await mountView()

    await openTab(wrapper, 'Skills')
    expect(wrapper.get('li a').attributes('href')).toBe('/skills/1/review/review-pr')

    await openTab(wrapper, 'Agents')
    expect(wrapper.get('li a').attributes('href')).toBe('/agents/1/review/security')
  })

  it('shows the folder on this machine', async () => {
    const wrapper = await mountView()

    await wrapper
      .findAll('button')
      .find((b) => b.text().includes('Show in'))!
      .trigger('click')

    expect(revealItemInDir).toHaveBeenCalledWith('/src/acme/plugins/review')
  })

  it('opens on the skills without a README', async () => {
    const wrapper = await mountView({ ...details, readme: null })

    expect(wrapper.findAll('[role="tab"]').map((tab) => tab.text())).toEqual([
      'Skills 1',
      'Agents 1',
      'Commands 0',
    ])
    expect(wrapper.get('li a').attributes('href')).toBe('/skills/1/review/review-pr')
  })

  it('explains that the contents of a plugin from elsewhere are not known', async () => {
    const wrapper = await mountView({
      ...details,
      plugin: plugin({
        name: 'remote',
        source: { kind: 'external', source: 'github', location: 'acme/remote' },
      }),
      readme: null,
      webUrl: 'https://github.com/acme/remote',
    })

    expect(field(wrapper, 'Installed from')).toBe('github acme/remote')
    expect(wrapper.text()).toContain('not known yet')
    expect(wrapper.find('[role="tab"]').exists()).toBe(false)
    expect(wrapper.text()).not.toContain('Show in')

    await wrapper
      .findAll('button')
      .find((b) => b.text() === 'View on GitHub')!
      .trigger('click')
    expect(openUrl).toHaveBeenCalledWith('https://github.com/acme/remote')
  })
})
