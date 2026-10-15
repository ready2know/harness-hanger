import { describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'

import { getAppInfo } from '@/lib/api'
import DashboardView from '../DashboardView.vue'

vi.mock('@/lib/api', () => ({ getAppInfo: vi.fn<typeof getAppInfo>() }))

describe('DashboardView', () => {
  it('shows the version and Claude Code config directory', async () => {
    vi.mocked(getAppInfo).mockResolvedValue({
      version: '1.2.3',
      claudeConfigDir: '/home/u/.claude',
    })

    const wrapper = mount(DashboardView)
    await flushPromises()

    expect(wrapper.text()).toContain('1.2.3')
    expect(wrapper.text()).toContain('/home/u/.claude')
  })

  it('shows the error when the backend call fails', async () => {
    vi.mocked(getAppInfo).mockRejectedValue(new Error('not running inside Tauri'))

    const wrapper = mount(DashboardView)
    await flushPromises()

    expect(wrapper.text()).toContain('not running inside Tauri')
  })
})
