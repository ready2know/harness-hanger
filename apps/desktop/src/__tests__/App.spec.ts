import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { createMemoryHistory, createRouter } from 'vue-router'

import { getAppInfo } from '@/lib/api'
import { routes } from '@/router'
import App from '../App.vue'

vi.mock('@/lib/api', () => ({ getAppInfo: vi.fn<typeof getAppInfo>() }))

enableAutoUnmount(afterEach)

beforeEach(() => {
  localStorage.clear()
  vi.mocked(getAppInfo).mockResolvedValue({ version: '1.2.3', claudeConfigDir: null })
})

async function mountAt(path: string) {
  const router = createRouter({ history: createMemoryHistory(), routes })
  await router.push(path)
  const wrapper = mount(App, { global: { plugins: [router] } })
  await flushPromises()
  return wrapper
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
