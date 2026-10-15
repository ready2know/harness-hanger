import { describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'

import { getAppInfo } from '@/lib/api'
import HomeView from '../HomeView.vue'

vi.mock('@/lib/api', () => ({ getAppInfo: vi.fn<typeof getAppInfo>() }))

describe('HomeView', () => {
  it('shows the version and Claude Code config directory', async () => {
    vi.mocked(getAppInfo).mockResolvedValue({
      version: '1.2.3',
      claudeConfigDir: '/home/u/.claude',
    })

    const wrapper = mount(HomeView)
    await flushPromises()

    expect(wrapper.text()).toContain('1.2.3')
    expect(wrapper.text()).toContain('/home/u/.claude')
  })

  it('shows the error when the backend call fails', async () => {
    vi.mocked(getAppInfo).mockRejectedValue(new Error('not running inside Tauri'))

    const wrapper = mount(HomeView)
    await flushPromises()

    expect(wrapper.text()).toContain('not running inside Tauri')
  })
})
