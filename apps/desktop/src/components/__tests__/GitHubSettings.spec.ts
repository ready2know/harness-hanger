import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { enableAutoUnmount, flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { openUrl } from '@tauri-apps/plugin-opener'

import {
  cancelGhSignIn,
  checkGitHubToken,
  getGitHubStatus,
  pollBrowserSignIn,
  pollGhSignIn,
  saveGitHubToken,
  signOutOfGitHub,
  startBrowserSignIn,
  startGhSignIn,
  type AccountStatus,
} from '@/lib/api'
import GitHubSettings from '../GitHubSettings.vue'

vi.mock('@/lib/api', () => ({
  getGitHubStatus: vi.fn<typeof getGitHubStatus>(),
  saveGitHubToken: vi.fn<typeof saveGitHubToken>(),
  signOutOfGitHub: vi.fn<typeof signOutOfGitHub>(),
  checkGitHubToken: vi.fn<typeof checkGitHubToken>(),
  startBrowserSignIn: vi.fn<typeof startBrowserSignIn>(),
  pollBrowserSignIn: vi.fn<typeof pollBrowserSignIn>(),
  startGhSignIn: vi.fn<typeof startGhSignIn>(),
  pollGhSignIn: vi.fn<typeof pollGhSignIn>(),
  cancelGhSignIn: vi.fn<typeof cancelGhSignIn>(() => Promise.resolve()),
}))
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn<typeof openUrl>() }))

enableAutoUnmount(afterEach)

function status(overrides: Partial<AccountStatus> = {}): AccountStatus {
  return {
    source: null,
    login: null,
    saved: false,
    savedLogin: null,
    gh: { signedIn: false, login: null },
    browserSignIn: true,
    keychainError: null,
    ...overrides,
  }
}

const signedIn = status({ source: { kind: 'saved' }, login: 'octocat', saved: true })

const deviceCode = {
  deviceCode: 'dc',
  userCode: 'ABCD-1234',
  verificationUri: 'https://github.com/login/device',
  expiresIn: 899,
  interval: 5,
}

async function mountSection() {
  const wrapper = mount(GitHubSettings, { attachTo: document.body })
  await flushPromises()
  return wrapper
}

function buttons(wrapper: VueWrapper) {
  return wrapper.findAll('button').map((button) => button.text())
}

async function click(wrapper: VueWrapper, label: string) {
  await wrapper
    .findAll('button')
    .find((button) => button.text() === label)!
    .trigger('click')
  await flushPromises()
}

/** The green badge, and the line after it. */
function expectSignedIn(wrapper: VueWrapper, line: string) {
  const badge = wrapper.get('[data-testid="github-badge"]')
  expect(badge.text()).toBe('Signed in')
  expect(badge.classes()).toContain('bg-emerald-600')
  expect(badge.find('svg').exists()).toBe(true)
  expect(wrapper.get('[data-testid="github-status"]').text()).toBe(line)
}

/** The sign-in dialog is portalled to `<body>`. */
function dialog() {
  return document.body.querySelector<HTMLElement>('[data-slot="dialog-content"]')
}

function dialogButton(label: string) {
  return [...dialog()!.querySelectorAll('button')].find((b) => b.textContent?.trim() === label)!
}

beforeEach(() => {
  vi.clearAllMocks()
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
  vi.mocked(getGitHubStatus).mockResolvedValue(status())
})

afterEach(() => {
  vi.useRealTimers()
})

describe('GitHubSettings', () => {
  it('says that it is not signed in, and offers each way to sign in', async () => {
    const wrapper = await mountSection()

    expect(wrapper.get('[data-testid="github-badge"]').text()).toBe('Not signed in')
    expect(buttons(wrapper)).toEqual(['Sign in with GitHub', 'Sign in with gh', 'Paste a token'])
  })

  it('offers only what this machine and build can do', async () => {
    vi.mocked(getGitHubStatus).mockResolvedValue(status({ browserSignIn: false, gh: null }))

    const wrapper = await mountSection()

    expect(buttons(wrapper)).toEqual(['Paste a token'])
  })

  it('saves a pasted token once GitHub accepts it', async () => {
    vi.mocked(saveGitHubToken).mockResolvedValue(signedIn)
    const wrapper = await mountSection()

    await click(wrapper, 'Paste a token')
    await wrapper.get('input[type="password"]').setValue('  ghp_secret ')
    await wrapper.get('form').trigger('submit')
    await flushPromises()

    expect(saveGitHubToken).toHaveBeenCalledWith('ghp_secret')
    expectSignedIn(wrapper, 'As octocat. The token is saved in the system keychain.')
    expect(wrapper.find('form').exists()).toBe(false)
    expect(buttons(wrapper)).toContain('Sign out')
  })

  it('shows why GitHub refused a token', async () => {
    vi.mocked(saveGitHubToken).mockRejectedValue('GitHub: GitHub refused the token')
    const wrapper = await mountSection()

    await click(wrapper, 'Paste a token')
    await wrapper.get('input[type="password"]').setValue('wrong')
    await wrapper.get('form').trigger('submit')
    await flushPromises()

    expect(wrapper.get('form [role="alert"]').text()).toContain('refused the token')
  })

  it('signs in in the browser', async () => {
    vi.mocked(startBrowserSignIn).mockResolvedValue(deviceCode)
    vi.mocked(pollBrowserSignIn)
      .mockResolvedValueOnce({ state: 'pending', interval: 10 })
      .mockResolvedValueOnce({ state: 'done', status: signedIn })
    Object.assign(navigator, {
      clipboard: { writeText: vi.fn<(text: string) => Promise<void>>(() => Promise.resolve()) },
    })
    const wrapper = await mountSection()

    await click(wrapper, 'Sign in with GitHub')
    expect(dialog()!.querySelector('[data-testid="user-code"]')!.textContent).toContain('ABCD-1234')
    dialogButton('Copy code and open GitHub').click()
    await flushPromises()
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith('ABCD-1234')
    expect(openUrl).toHaveBeenCalledWith('https://github.com/login/device')

    await vi.advanceTimersByTimeAsync(5000)
    expect(pollBrowserSignIn).toHaveBeenLastCalledWith('dc', 5)
    // GitHub asked to wait longer.
    await vi.advanceTimersByTimeAsync(5000)
    expect(pollBrowserSignIn).toHaveBeenCalledTimes(1)
    await vi.advanceTimersByTimeAsync(5000)
    await flushPromises()

    expect(pollBrowserSignIn).toHaveBeenLastCalledWith('dc', 10)
    expect(dialog()).toBeNull()
    expectSignedIn(wrapper, 'As octocat. The token is saved in the system keychain.')
  })

  it('shows why a sign-in in the browser stopped', async () => {
    vi.mocked(startBrowserSignIn).mockResolvedValue(deviceCode)
    vi.mocked(pollBrowserSignIn).mockRejectedValue('GitHub: the code expired before it was entered')
    const wrapper = await mountSection()

    await click(wrapper, 'Sign in with GitHub')
    await vi.advanceTimersByTimeAsync(5000)
    await flushPromises()

    expect(dialog()!.querySelector('[role="alert"]')!.textContent).toContain('the code expired')
    dialogButton('Close').click()
    await flushPromises()
    expect(dialog()).toBeNull()
  })

  it('signs in through gh', async () => {
    vi.mocked(startGhSignIn).mockResolvedValue({
      userCode: 'WXYZ-9876',
      verificationUri: 'https://github.com/login/device',
    })
    vi.mocked(pollGhSignIn)
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce(
        status({
          source: { kind: 'gh' },
          login: 'octocat',
          gh: { signedIn: true, login: 'octocat' },
        }),
      )
    const wrapper = await mountSection()

    await click(wrapper, 'Sign in with gh')
    expect(dialog()!.textContent).toContain('WXYZ-9876')
    await vi.advanceTimersByTimeAsync(2000)
    await flushPromises()

    expect(dialog()).toBeNull()
    expectSignedIn(wrapper, 'Through the gh command as octocat.')
    expect(cancelGhSignIn).not.toHaveBeenCalled()
  })

  it('stops gh when the sign-in is cancelled', async () => {
    vi.mocked(startGhSignIn).mockResolvedValue({
      userCode: 'WXYZ-9876',
      verificationUri: 'https://github.com/login/device',
    })
    vi.mocked(pollGhSignIn).mockResolvedValue(null)
    const wrapper = await mountSection()

    await click(wrapper, 'Sign in with gh')
    dialogButton('Cancel').click()
    await flushPromises()
    await vi.advanceTimersByTimeAsync(5000)

    expect(cancelGhSignIn).toHaveBeenCalledTimes(1)
    expect(pollGhSignIn).not.toHaveBeenCalled()
    expect(dialog()).toBeNull()
  })

  it('says which variable overrides the saved token, and checks it', async () => {
    vi.mocked(getGitHubStatus).mockResolvedValue(
      status({ source: { kind: 'env', variable: 'GH_TOKEN' }, saved: true, savedLogin: 'octocat' }),
    )
    vi.mocked(checkGitHubToken).mockResolvedValue('robot')
    const wrapper = await mountSection()

    const text = wrapper.get('[data-testid="github-status"]').text()
    expect(text).toContain('GH_TOKEN')
    expect(text).toContain('It overrides the token saved here.')
    await click(wrapper, 'Check')

    expect(wrapper.text()).toContain('GitHub accepts the token in use. It belongs to robot.')
  })

  it('signs out by forgetting the saved token', async () => {
    vi.mocked(getGitHubStatus).mockResolvedValue(signedIn)
    vi.mocked(signOutOfGitHub).mockResolvedValue(status())
    const wrapper = await mountSection()

    await click(wrapper, 'Sign out')

    expect(signOutOfGitHub).toHaveBeenCalled()
    expect(wrapper.get('[data-testid="github-badge"]').text()).toBe('Not signed in')
    expect(wrapper.text()).toContain('The saved token is forgotten.')
  })

  it('says why the keychain cannot be used', async () => {
    vi.mocked(getGitHubStatus).mockResolvedValue(
      status({ keychainError: 'no Secret Service is running' }),
    )

    const wrapper = await mountSection()

    expect(wrapper.get('[role="alert"]').text()).toContain('no Secret Service is running')
  })
})
