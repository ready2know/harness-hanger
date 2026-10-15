import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { createMemoryHistory, createRouter } from 'vue-router'

import { getAppInfo, getMarketplaceDetails, listCatalogPlugins, listMarketplaces } from '@/lib/api'
import { routes } from '@/router'
import App from '../App.vue'

vi.mock('@/lib/api', () => ({
  getAppInfo: vi.fn<typeof getAppInfo>(),
  listMarketplaces: vi.fn<typeof listMarketplaces>(),
  listCatalogPlugins: vi.fn<typeof listCatalogPlugins>(),
  // Never settles: these tests only look at the layout around the page.
  getMarketplaceDetails: vi.fn<typeof getMarketplaceDetails>(() => new Promise(() => {})),
}))

enableAutoUnmount(afterEach)

beforeEach(() => {
  localStorage.clear()
  vi.mocked(getAppInfo).mockResolvedValue({ version: '1.2.3', claudeConfigDir: null })
  vi.mocked(listMarketplaces).mockResolvedValue([])
  vi.mocked(listCatalogPlugins).mockResolvedValue({ plugins: [], unreadable: [] })
})

async function mountAt(path: string) {
  const router = createRouter({ history: createMemoryHistory(), routes })
  await router.push(path)
  // Attached, so that events reach document listeners and portalled menus and dialogs.
  const wrapper = mount(App, { global: { plugins: [router] }, attachTo: document.body })
  await flushPromises()
  return wrapper
}

/** Menus and dialogs are portalled to `<body>`, outside the wrapper. */
function bodyElement(selector: string) {
  return document.body.querySelector<HTMLElement>(selector)
}

describe('App sidebar', () => {
  it('lists the sections and keeps Settings apart in the footer', async () => {
    const wrapper = await mountAt('/')

    const content = wrapper.get('[data-sidebar="content"]').text()
    for (const title of ['Dashboard', 'Marketplace', 'Skills', 'Agents', 'Plugins']) {
      expect(content).toContain(title)
    }
    expect(content).not.toContain('Settings')
    expect(wrapper.get('[data-sidebar="footer"]').text()).toContain('Settings')
  })

  it('marks the current section as active', async () => {
    const wrapper = await mountAt('/skills')

    const active = wrapper.findAll('[data-sidebar="menu-button"][data-active]')
    expect(active.map((button) => button.text())).toEqual(['Skills'])
    expect(wrapper.get('h1').text()).toBe('Skills')
  })

  it('keeps a section active on the pages below it', async () => {
    const wrapper = await mountAt('/marketplace/3')

    const active = wrapper.findAll('[data-sidebar="menu-button"][data-active]')
    expect(active.map((button) => button.text())).toEqual(['Marketplace'])
  })

  it('collapses to icons with the toggle button and remembers it', async () => {
    const wrapper = await mountAt('/')
    const sidebar = wrapper.get('[data-slot="sidebar"]')
    expect(sidebar.attributes('data-state')).toBe('expanded')

    await wrapper.get('[data-sidebar="trigger"]').trigger('click')

    expect(sidebar.attributes('data-state')).toBe('collapsed')
    expect(sidebar.attributes('data-collapsible')).toBe('icon')
    expect(localStorage.getItem('sidebar-open')).toBe('false')
  })

  it('starts collapsed when it was collapsed before', async () => {
    localStorage.setItem('sidebar-open', 'false')

    const wrapper = await mountAt('/')

    expect(wrapper.get('[data-slot="sidebar"]').attributes('data-state')).toBe('collapsed')
  })
})

describe('App context menu', () => {
  async function rightClick(element: Element) {
    const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true })
    element.dispatchEvent(event)
    await flushPromises()
    return event
  }

  it('replaces the system menu with the app menu', async () => {
    const wrapper = await mountAt('/')

    const event = await rightClick(wrapper.get('h1').element)

    expect(event.defaultPrevented).toBe(true)
    expect(bodyElement('[data-slot="context-menu-content"]')?.textContent).toContain('About')
  })

  it('suppresses the system menu outside the app tree too', async () => {
    await mountAt('/')

    const event = await rightClick(document.body)

    expect(event.defaultPrevented).toBe(true)
  })

  it('shows the app version in the About dialog', async () => {
    const wrapper = await mountAt('/')
    await rightClick(wrapper.get('h1').element)

    bodyElement('[data-slot="context-menu-item"]')?.click()
    await flushPromises()

    expect(bodyElement('[data-slot="dialog-content"]')?.textContent).toContain('Version 1.2.3')
  })
})
