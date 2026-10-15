import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'

import {
  getImportCandidates,
  importFromHarness,
  type Discovery,
  type MarketplaceSummary,
} from '@/lib/api'
import ImportDialog from '../ImportDialog.vue'

vi.mock('@/lib/api', () => ({
  getImportCandidates: vi.fn<typeof getImportCandidates>(),
  importFromHarness: vi.fn<typeof importFromHarness>(),
}))

enableAutoUnmount(afterEach)

const claudeCode: Discovery = {
  harness: 'claude-code',
  name: 'Claude Code',
  configDir: '/home/me/.claude',
  marketplaces: [
    {
      name: 'acme',
      source: { kind: 'github', owner: 'acme', repo: 'tools' },
      problem: null,
      installedPlugins: ['review', 'lint'],
      inCatalog: false,
    },
    {
      name: 'ai-zen',
      source: { kind: 'local', path: '/src/ai-zen' },
      problem: null,
      installedPlugins: [],
      inCatalog: true,
    },
    {
      name: 'gone',
      source: null,
      problem: 'its directory does not exist',
      installedPlugins: ['old'],
      inCatalog: false,
    },
  ],
  standalone: {
    source: { kind: 'harness', harness: 'claude-code' },
    path: '/home/me/.claude',
    skills: 5,
    agents: 2,
    commands: 0,
    inCatalog: false,
  },
  unmatchedPlugins: ['linear@anthropic-plugin-directory'],
}

const imported = { id: 1 } as MarketplaceSummary

beforeEach(() => {
  vi.mocked(getImportCandidates).mockResolvedValue([claudeCode])
  vi.mocked(importFromHarness).mockReset()
})

async function mountDialog() {
  const wrapper = mount(ImportDialog, { props: { open: false }, attachTo: document.body })
  await wrapper.setProps({ open: true })
  await flushPromises()
  return wrapper
}

/** The dialog is portalled to `<body>`. */
function dialog() {
  return document.body.querySelector<HTMLElement>('[data-slot="dialog-content"]')!
}

function row(title: string) {
  return [...dialog().querySelectorAll('li')].find((item) =>
    item.querySelector('.font-medium')?.textContent?.includes(title),
  )!
}

function checkbox(title: string) {
  return row(title).querySelector<HTMLButtonElement>('[role="checkbox"]')!
}

function button(label: string) {
  return [...dialog().querySelectorAll('button')].find((b) => b.textContent?.trim() === label)!
}

describe('ImportDialog', () => {
  it('picks everything new, and says why the rest cannot be picked', async () => {
    await mountDialog()

    expect(checkbox('Standalone').getAttribute('aria-checked')).toBe('true')
    expect(row('Standalone').textContent).toContain('5 skills · 2 agents')
    expect(checkbox('acme').getAttribute('aria-checked')).toBe('true')
    expect(row('acme').textContent).toContain('github.com/acme/tools')
    expect(row('acme').textContent).toContain('Installed plugins: review, lint')
    expect(checkbox('ai-zen').disabled).toBe(true)
    expect(row('ai-zen').textContent).toContain('In catalog')
    expect(checkbox('gone').disabled).toBe(true)
    expect(row('gone').textContent).toContain('its directory does not exist')
    expect(dialog().textContent).toContain('linear@anthropic-plugin-directory')
    expect(button('Import 2')).toBeDefined()
  })

  it('imports what is picked, one by one, and shows each outcome', async () => {
    vi.mocked(importFromHarness)
      .mockResolvedValueOnce(imported)
      .mockRejectedValueOnce('git failed: no network')
    const wrapper = await mountDialog()

    button('Import 2').click()
    await flushPromises()

    expect(vi.mocked(importFromHarness).mock.calls).toEqual([
      ['claude-code', { kind: 'standalone' }],
      ['claude-code', { kind: 'marketplace', name: 'acme' }],
    ])
    expect(row('Standalone').querySelector('[aria-label="Imported"]')).not.toBeNull()
    expect(row('acme').querySelector('[role="alert"]')!.textContent).toContain('no network')
    expect(wrapper.emitted('imported')).toHaveLength(1)
    expect(button('Done')).toBeDefined()
  })

  it('leaves out what is unticked', async () => {
    vi.mocked(importFromHarness).mockResolvedValue(imported)
    await mountDialog()

    checkbox('Standalone').click()
    await flushPromises()
    button('Import 1').click()
    await flushPromises()

    expect(vi.mocked(importFromHarness).mock.calls).toEqual([
      ['claude-code', { kind: 'marketplace', name: 'acme' }],
    ])
  })

  it('stays open while importing', async () => {
    let finish!: (summary: MarketplaceSummary) => void
    vi.mocked(importFromHarness).mockReturnValue(new Promise((resolve) => (finish = resolve)))
    const wrapper = await mountDialog()

    button('Import 2').click()
    await flushPromises()
    dialog().dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    await flushPromises()

    expect(wrapper.emitted('update:open')).toBeUndefined()
    finish(imported)
  })

  it('ignores a slow load from an earlier opening', async () => {
    let finishFirst!: (discoveries: Discovery[]) => void
    vi.mocked(getImportCandidates)
      .mockReturnValueOnce(new Promise((resolve) => (finishFirst = resolve)))
      .mockResolvedValueOnce([claudeCode])
    const wrapper = mount(ImportDialog, { props: { open: false }, attachTo: document.body })
    await wrapper.setProps({ open: true })
    await wrapper.setProps({ open: false })
    await wrapper.setProps({ open: true })
    await flushPromises()

    checkbox('Standalone').click()
    await flushPromises()
    finishFirst([claudeCode])
    await flushPromises()

    expect(checkbox('Standalone').getAttribute('aria-checked')).toBe('false')
  })

  it('says when a harness is not on this machine', async () => {
    vi.mocked(getImportCandidates).mockResolvedValue([
      { ...claudeCode, configDir: null, marketplaces: [], standalone: null, unmatchedPlugins: [] },
    ])

    await mountDialog()

    expect(dialog().textContent).toContain('Not found on this machine')
    expect(button('Import 0').disabled).toBe(true)
  })
})
