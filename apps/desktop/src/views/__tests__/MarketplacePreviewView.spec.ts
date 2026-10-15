import { afterEach, describe, expect, it, vi } from 'vitest'
import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'

import {
  getMarketplaceDetails,
  removeMarketplace,
  type MarketplaceDetails,
  type PluginPreview,
} from '@/lib/api'
import MarketplacePreviewView from '../MarketplacePreviewView.vue'
import { stubRouter } from './stubRouter'

vi.mock('@/lib/api', () => ({
  getMarketplaceDetails: vi.fn<typeof getMarketplaceDetails>(),
  removeMarketplace: vi.fn<typeof removeMarketplace>(),
}))

enableAutoUnmount(afterEach)

function plugin(overrides: Partial<PluginPreview>): PluginPreview {
  return {
    name: 'plugin',
    description: null,
    version: null,
    author: null,
    category: null,
    keywords: [],
    homepage: null,
    repository: null,
    license: null,
    source: { kind: 'local', path: '/m/plugins/plugin' },
    skills: [],
    agents: [],
    commands: [],
    ...overrides,
  }
}

const details: MarketplaceDetails = {
  summary: {
    id: 3,
    source: { kind: 'github', owner: 'acme', repo: 'tools' },
    addedAt: '2026-10-15T12:00:00Z',
    root: '/data/marketplaces/github/acme/tools',
    name: 'acme-tools',
    description: 'Tools by Acme.',
    pluginCount: 2,
    error: null,
  },
  preview: {
    name: 'acme-tools',
    description: 'Tools by Acme.',
    owner: 'Acme',
    plugins: [
      plugin({
        name: 'review',
        version: '1.0.0',
        description: 'Code review helpers.',
        skills: [
          { name: 'review-pr', description: 'Reviews a pull request.', path: '/a/SKILL.md' },
          { name: 'lint', description: 'Runs linters.', path: '/b/SKILL.md' },
        ],
        agents: [{ name: 'security', description: 'Finds security bugs.', path: '/c.md' }],
      }),
      plugin({
        name: 'remote',
        source: { kind: 'external', source: 'github', location: 'acme/remote' },
      }),
    ],
    warnings: [
      { path: '/m/plugins/missing', message: 'plugin missing: the directory does not exist' },
    ],
  },
}

async function mountView() {
  const router = stubRouter()
  await router.push('/marketplace/3')
  const wrapper = mount(MarketplacePreviewView, {
    props: { id: 3 },
    global: { plugins: [router] },
    attachTo: document.body,
  })
  await flushPromises()
  return { wrapper, router }
}

function tabLabels(wrapper: Awaited<ReturnType<typeof mountView>>['wrapper']) {
  return wrapper.findAll('[role="tab"]').map((tab) => tab.text())
}

describe('MarketplacePreviewView', () => {
  it('shows the marketplace, counts its components, and lists its plugins', async () => {
    vi.mocked(getMarketplaceDetails).mockResolvedValue(details)

    const { wrapper } = await mountView()

    expect(getMarketplaceDetails).toHaveBeenCalledWith(3)
    expect(wrapper.get('h1').text()).toBe('acme-tools')
    expect(wrapper.text()).toContain('github.com/acme/tools')
    expect(wrapper.text()).toContain('by Acme')
    expect(tabLabels(wrapper)).toEqual(['Plugins 2', 'Skills 2', 'Agents 1', 'Commands 0'])
    expect(wrapper.text()).toContain('2 skills · 1 agent')
    expect(wrapper.text()).toContain('Installed from elsewhere; contents unknown')
    expect(wrapper.text()).toContain('1 warning')
  })

  it('filters every list by the search text', async () => {
    vi.mocked(getMarketplaceDetails).mockResolvedValue(details)
    const { wrapper } = await mountView()

    await wrapper.get('input[aria-label="Search"]').setValue('pull request')

    expect(tabLabels(wrapper)).toEqual(['Plugins 0', 'Skills 1', 'Agents 0', 'Commands 0'])
  })

  it('explains why a marketplace cannot be read', async () => {
    vi.mocked(getMarketplaceDetails).mockResolvedValue({
      summary: { ...details.summary, name: null, error: 'the directory does not exist' },
      preview: null,
    })

    const { wrapper } = await mountView()

    expect(wrapper.get('[role="alert"]').text()).toContain('the directory does not exist')
    expect(wrapper.find('[role="tab"]').exists()).toBe(false)
  })

  it('removes the marketplace after confirmation and goes back to the list', async () => {
    vi.mocked(getMarketplaceDetails).mockResolvedValue(details)
    vi.mocked(removeMarketplace).mockResolvedValue()
    const { wrapper, router } = await mountView()

    const removeButton = wrapper.findAll('button').find((b) => b.text() === 'Remove')!
    await removeButton.trigger('click')
    await flushPromises()
    const dialog = document.body.querySelector('[data-slot="dialog-content"]')!
    expect(dialog.textContent).toContain('deletes its clone')
    const confirm = [...dialog.querySelectorAll('button')].find(
      (b) => b.textContent?.trim() === 'Remove',
    )!
    confirm.click()
    await flushPromises()

    expect(removeMarketplace).toHaveBeenCalledWith(3)
    expect(router.currentRoute.value.name).toBe('marketplace')
  })

  it('ignores a late answer for the marketplace it showed before', async () => {
    let answerFirst!: (details: MarketplaceDetails) => void
    vi.mocked(getMarketplaceDetails)
      .mockReturnValueOnce(new Promise((resolve) => (answerFirst = resolve)))
      .mockResolvedValueOnce({
        ...details,
        summary: { ...details.summary, id: 4, name: 'second' },
      })
    const { wrapper } = await mountView()

    await wrapper.setProps({ id: 4 })
    await flushPromises()
    answerFirst(details)
    await flushPromises()

    expect(wrapper.get('h1').text()).toBe('second')
  })

  it('keeps the remove dialog open and shows why removing failed', async () => {
    vi.mocked(getMarketplaceDetails).mockResolvedValue(details)
    vi.mocked(removeMarketplace).mockRejectedValue('database is locked')
    const { wrapper, router } = await mountView()

    await wrapper
      .findAll('button')
      .find((b) => b.text() === 'Remove')!
      .trigger('click')
    await flushPromises()
    const dialog = document.body.querySelector('[data-slot="dialog-content"]')!
    ;[...dialog.querySelectorAll('button')].find((b) => b.textContent?.trim() === 'Remove')!.click()
    await flushPromises()

    expect(dialog.querySelector('[role="alert"]')!.textContent).toContain('database is locked')
    expect(router.currentRoute.value.name).toBe('marketplace-preview')
  })
})
