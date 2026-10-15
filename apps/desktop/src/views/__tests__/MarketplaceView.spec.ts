import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { open as openFolderPicker } from '@tauri-apps/plugin-dialog'
import { createMemoryHistory, createRouter } from 'vue-router'

import { addMarketplace, listMarketplaces, type MarketplaceSummary } from '@/lib/api'
import MarketplaceView from '../MarketplaceView.vue'

vi.mock('@/lib/api', () => ({
  listMarketplaces: vi.fn<typeof listMarketplaces>(),
  addMarketplace: vi.fn<typeof addMarketplace>(),
}))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn<typeof openFolderPicker>() }))

enableAutoUnmount(afterEach)

const official: MarketplaceSummary = {
  id: 1,
  source: { kind: 'github', owner: 'anthropics', repo: 'claude-plugins-official' },
  addedAt: '2026-10-15T12:00:00Z',
  root: '/data/marketplaces/github/anthropics/claude-plugins-official',
  name: 'claude-plugins-official',
  description: 'Official plugins.',
  pluginCount: 315,
  error: null,
}

const moved: MarketplaceSummary = {
  id: 2,
  source: { kind: 'local', path: '/src/moved' },
  addedAt: '2026-10-15T12:00:00Z',
  root: '/src/moved',
  name: null,
  description: null,
  pluginCount: 0,
  error: '/src/moved is not a marketplace: the directory does not exist',
}

function makeRouter() {
  return createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/marketplace', name: 'marketplace', component: MarketplaceView },
      { path: '/marketplace/:id', name: 'marketplace-preview', component: { render: () => null } },
    ],
  })
}

async function mountView() {
  const router = makeRouter()
  await router.push('/marketplace')
  const wrapper = mount(MarketplaceView, {
    global: { plugins: [router] },
    attachTo: document.body,
  })
  await flushPromises()
  return { wrapper, router }
}

/** The add dialog is portalled to `<body>`. */
function dialog() {
  return document.body.querySelector<HTMLElement>('[data-slot="dialog-content"]')
}

/** The input reports its value to the dialog on the next tick. */
async function setInput(value: string) {
  const input = dialog()!.querySelector('input')!
  input.value = value
  input.dispatchEvent(new Event('input'))
  await flushPromises()
}

beforeEach(() => {
  vi.mocked(listMarketplaces).mockResolvedValue([official, moved])
})

describe('MarketplaceView', () => {
  it('lists the marketplaces with their source and plugin count', async () => {
    const { wrapper } = await mountView()

    const cards = wrapper.findAll('li')
    expect(cards).toHaveLength(2)
    expect(cards[0]!.text()).toContain('claude-plugins-official')
    expect(cards[0]!.text()).toContain('315 plugins')
    expect(cards[0]!.text()).toContain('github.com/anthropics/claude-plugins-official')
    expect(cards[1]!.text()).toContain('Unreadable marketplace')
    expect(cards[1]!.text()).toContain('the directory does not exist')
  })

  it('invites you to add the first marketplace', async () => {
    vi.mocked(listMarketplaces).mockResolvedValue([])

    const { wrapper } = await mountView()

    expect(wrapper.text()).toContain('No marketplaces yet')
  })

  it('adds a marketplace and opens its preview', async () => {
    vi.mocked(listMarketplaces).mockResolvedValue([])
    vi.mocked(addMarketplace).mockResolvedValue({ ...official, id: 7 })
    const { wrapper, router } = await mountView()

    await wrapper.get('button').trigger('click')
    await flushPromises()
    await setInput('anthropics/claude-plugins-official')
    dialog()!.querySelector('form')!.dispatchEvent(new Event('submit'))
    await flushPromises()

    expect(addMarketplace).toHaveBeenCalledWith('anthropics/claude-plugins-official')
    expect(router.currentRoute.value.fullPath).toBe('/marketplace/7')
  })

  it('fills in the folder chosen with Browse', async () => {
    vi.mocked(listMarketplaces).mockResolvedValue([])
    vi.mocked(openFolderPicker).mockResolvedValue('/src/my-marketplace')
    const { wrapper } = await mountView()

    await wrapper.get('button').trigger('click')
    await flushPromises()
    const browse = [...dialog()!.querySelectorAll('button')].find((b) =>
      b.textContent?.includes('Browse'),
    )!
    browse.click()
    await flushPromises()

    expect(dialog()!.querySelector('input')!.value).toBe('/src/my-marketplace')
  })

  it('shows why a marketplace could not be added', async () => {
    vi.mocked(listMarketplaces).mockResolvedValue([])
    vi.mocked(addMarketplace).mockRejectedValue(
      'git failed: the repository does not exist, or it is private and git has no credentials for it',
    )
    const { wrapper, router } = await mountView()

    await wrapper.get('button').trigger('click')
    await flushPromises()
    await setInput('acme/missing')
    dialog()!.querySelector('form')!.dispatchEvent(new Event('submit'))
    await flushPromises()

    expect(dialog()!.querySelector('[role="alert"]')!.textContent).toContain(
      'the repository does not exist',
    )
    expect(router.currentRoute.value.fullPath).toBe('/marketplace')
  })

  it('stays open while a marketplace is being added', async () => {
    vi.mocked(listMarketplaces).mockResolvedValue([])
    let finish!: (marketplace: MarketplaceSummary) => void
    vi.mocked(addMarketplace).mockReturnValue(new Promise((resolve) => (finish = resolve)))
    const { wrapper, router } = await mountView()

    await wrapper.get('button').trigger('click')
    await flushPromises()
    await setInput('  acme/tools  ')
    dialog()!.querySelector('form')!.dispatchEvent(new Event('submit'))
    await flushPromises()
    dialog()!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    await flushPromises()

    expect(dialog()).not.toBeNull()
    expect(addMarketplace).toHaveBeenCalledWith('acme/tools')
    finish({ ...official, id: 9 })
    await flushPromises()
    expect(router.currentRoute.value.fullPath).toBe('/marketplace/9')
  })

  it('shows why the folder picker failed', async () => {
    vi.mocked(listMarketplaces).mockResolvedValue([])
    vi.mocked(openFolderPicker).mockRejectedValue('dialog.open not allowed')
    const { wrapper } = await mountView()

    await wrapper.get('button').trigger('click')
    await flushPromises()
    ;[...dialog()!.querySelectorAll('button')]
      .find((b) => b.textContent?.includes('Browse'))!
      .click()
    await flushPromises()

    expect(dialog()!.querySelector('[role="alert"]')!.textContent).toContain('not allowed')
  })
})
