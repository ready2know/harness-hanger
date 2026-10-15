import { afterEach, beforeAll, beforeEach, describe, expect, it } from 'vitest'
import { enableAutoUnmount, flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { h } from 'vue'

import type { PageSizeSetting } from '@/lib/pageSizes'
import PagedList from '../PagedList.vue'

enableAutoUnmount(afterEach)

// The select scrolls its chosen option into view, which jsdom cannot do.
beforeAll(() => {
  Element.prototype.scrollIntoView = () => {}
})

beforeEach(() => localStorage.clear())

const numbers = (count: number) => Array.from({ length: count }, (_, index) => index + 1)

function mountList(items: number[], setting?: PageSizeSetting) {
  return mount(PagedList<number>, {
    props: { items, setting },
    slots: {
      default: ({ items: shown }: { items: number[] }) =>
        h(
          'ul',
          shown.map((item) => h('li', item)),
        ),
    },
    attachTo: document.body,
  })
}

function shown(wrapper: VueWrapper) {
  return wrapper.findAll('li').map((item) => Number(item.text()))
}

function range(wrapper: VueWrapper) {
  return wrapper.get('[aria-live="polite"]').text()
}

async function choosePageSize(wrapper: VueWrapper, label: string) {
  await wrapper.get('[data-slot="select-trigger"]').trigger('keydown', { key: 'Enter' })
  await flushPromises()
  const option = [...document.body.querySelectorAll('[role="option"]')].find(
    (item) => item.textContent?.trim() === label,
  )!
  option.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }))
  await flushPromises()
}

async function turnTo(wrapper: VueWrapper, page: string) {
  await wrapper
    .findAll('[data-slot="pagination-item"]')
    .find((item) => item.text() === page)!
    .trigger('click')
}

describe('PagedList', () => {
  it('shows 10 items at first, and turns pages', async () => {
    const wrapper = mountList(numbers(60))

    expect(shown(wrapper)).toEqual(numbers(10))
    expect(range(wrapper)).toBe('1–10 of 60')

    await turnTo(wrapper, '3')
    expect(shown(wrapper)).toEqual([21, 22, 23, 24, 25, 26, 27, 28, 29, 30])
    expect(range(wrapper)).toBe('21–30 of 60')

    await wrapper.get('[data-slot="pagination-previous"]').trigger('click')
    expect(range(wrapper)).toBe('11–20 of 60')
  })

  it('offers 5, 10, 25, 50, 100, and all items per page, and remembers the choice', async () => {
    const wrapper = mountList(numbers(60))

    await choosePageSize(wrapper, '50')

    expect(shown(wrapper)).toEqual(numbers(50))
    expect(wrapper.get('[data-slot="select-trigger"]').attributes('aria-expanded')).toBe('false')
    expect(localStorage.getItem('list-page-size')).toBe('50')
    expect(mountList(numbers(60)).findAll('li')).toHaveLength(50)
  })

  it('shows every item, without pages, for All', async () => {
    const wrapper = mountList(numbers(60))

    await choosePageSize(wrapper, 'All')

    expect(shown(wrapper)).toEqual(numbers(60))
    expect(range(wrapper)).toBe('1–60 of 60')
    expect(wrapper.find('[data-slot="pagination"]').exists()).toBe(false)
  })

  it('keeps the first item of the page when the page size changes', async () => {
    const wrapper = mountList(numbers(60))
    await turnTo(wrapper, '6')
    expect(range(wrapper)).toBe('51–60 of 60')

    await choosePageSize(wrapper, '25')

    expect(range(wrapper)).toBe('51–60 of 60')
  })

  it('moves back to the last page when the list gets shorter', async () => {
    const wrapper = mountList(numbers(60))
    await turnTo(wrapper, '3')

    await wrapper.setProps({ items: numbers(15) })

    expect(range(wrapper)).toBe('11–15 of 15')
  })

  it('has no controls for a list that fits on the smallest page', () => {
    const wrapper = mountList(numbers(5))

    expect(shown(wrapper)).toEqual(numbers(5))
    expect(wrapper.find('[data-slot="select-trigger"]').exists()).toBe(false)
  })

  it('has controls for a list longer than the smallest page, so that it can be split', async () => {
    const wrapper = mountList(numbers(8))
    expect(range(wrapper)).toBe('1–8 of 8')

    await choosePageSize(wrapper, '5')

    expect(shown(wrapper)).toEqual(numbers(5))
    expect(range(wrapper)).toBe('1–5 of 8')
  })

  it('ignores a page size that it does not offer', () => {
    localStorage.setItem('list-page-size', '7')

    expect(mountList(numbers(60)).findAll('li')).toHaveLength(10)
  })

  it('keeps the page size of lists with their own setting apart', async () => {
    const setting: PageSizeSetting = { key: 'other-page-size', initial: '5' }
    const wrapper = mountList(numbers(60), setting)
    expect(range(wrapper)).toBe('1–5 of 60')

    await choosePageSize(wrapper, '25')

    expect(localStorage.getItem('other-page-size')).toBe('25')
    expect(localStorage.getItem('list-page-size')).toBeNull()
    expect(mountList(numbers(60)).findAll('li')).toHaveLength(10)
  })
})
