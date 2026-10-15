import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { open as openFolderPicker } from '@tauri-apps/plugin-dialog'

import { addMarketplace, type MarketplaceSummary } from '@/lib/api'
import AddMarketplaceDialog from '../AddMarketplaceDialog.vue'

vi.mock('@/lib/api', () => ({ addMarketplace: vi.fn<typeof addMarketplace>() }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn<typeof openFolderPicker>() }))

enableAutoUnmount(afterEach)

const added = { id: 1 } as MarketplaceSummary

beforeEach(() => {
  vi.clearAllMocks()
})

async function mountDialog() {
  const wrapper = mount(AddMarketplaceDialog, { props: { open: false }, attachTo: document.body })
  await wrapper.setProps({ open: true })
  await flushPromises()
  return wrapper
}

/** The dialog is portalled to `<body>`. */
function dialog() {
  return document.body.querySelector<HTMLElement>('[data-slot="dialog-content"]')!
}

function button(label: string) {
  return [...dialog().querySelectorAll('button')].find((b) => b.textContent?.trim() === label)!
}

function textarea() {
  return dialog().querySelector('textarea')!
}

/** The textarea reports its value to the dialog on the next tick. */
async function type(text: string) {
  textarea().value = text
  textarea().dispatchEvent(new Event('input'))
  await flushPromises()
}

describe('AddMarketplaceDialog', () => {
  it('takes one source per line, without blanks, comments, or repeats', async () => {
    await mountDialog()

    await type('acme/tools\n\n# a comment\n  /src/market  \nacme/tools\n')

    expect(dialog().textContent).toContain('2 marketplaces')
    expect(button('Add 2')).toBeDefined()
  })

  it('adds one source and closes', async () => {
    vi.mocked(addMarketplace).mockResolvedValue(added)
    const wrapper = await mountDialog()
    await type('acme/tools\n# a comment')

    button('Add').click()
    await flushPromises()

    expect(addMarketplace).toHaveBeenCalledWith('acme/tools')
    expect(wrapper.emitted('added')).toEqual([[added]])
    expect(wrapper.emitted('update:open')).toEqual([[false]])
  })

  it('adds with Cmd+Enter', async () => {
    vi.mocked(addMarketplace).mockResolvedValue(added)
    await mountDialog()
    await type('acme/tools')

    textarea().dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', metaKey: true }))
    await flushPromises()

    expect(addMarketplace).toHaveBeenCalledWith('acme/tools')
  })

  it('shows why one source was not added, until the text changes', async () => {
    vi.mocked(addMarketplace).mockRejectedValue('git failed: the repository does not exist')
    const wrapper = await mountDialog()
    await type('acme/missing')

    button('Add').click()
    await flushPromises()

    expect(dialog().querySelector('[role="alert"]')!.textContent).toContain('does not exist')
    expect(wrapper.emitted('update:open')).toBeUndefined()
    await type('acme/tools')
    expect(dialog().querySelector('[role="alert"]')).toBeNull()
  })

  it('adds several sources in turn, and shows each outcome', async () => {
    vi.mocked(addMarketplace)
      .mockResolvedValueOnce(added)
      .mockRejectedValueOnce('/src/gone is not a marketplace: the directory does not exist')
    const wrapper = await mountDialog()
    await type('acme/tools\n/src/gone')

    button('Add 2').click()
    await flushPromises()

    expect(vi.mocked(addMarketplace).mock.calls).toEqual([['acme/tools'], ['/src/gone']])
    const rows = [...dialog().querySelectorAll('li')]
    expect(rows[0]!.querySelector('[aria-label="Added"]')).not.toBeNull()
    expect(rows[1]!.querySelector('[role="alert"]')!.textContent).toContain('does not exist')
    expect(dialog().textContent).toContain('1 added, 1 not')
    expect(wrapper.emitted('addedSeveral')).toHaveLength(1)
    expect(wrapper.emitted('added')).toBeUndefined()
    expect(button('Done')).toBeDefined()
  })

  it('appends the folders that were picked', async () => {
    vi.mocked(openFolderPicker).mockResolvedValue(['/src/one', '/src/two'])
    await mountDialog()
    await type('acme/tools')

    button('Browse…').click()
    await flushPromises()

    expect(textarea().value).toBe('acme/tools\n/src/one\n/src/two')
    expect(dialog().textContent).toContain('3 marketplaces')
  })

  it('stays open while adding several', async () => {
    let finish!: (summary: MarketplaceSummary) => void
    vi.mocked(addMarketplace).mockReturnValue(new Promise((resolve) => (finish = resolve)))
    const wrapper = await mountDialog()
    await type('acme/tools\nacme/more')

    button('Add 2').click()
    await flushPromises()
    dialog().dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    await flushPromises()

    expect(wrapper.emitted('update:open')).toBeUndefined()
    finish(added)
  })
})
