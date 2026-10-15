import type { VueWrapper } from '@vue/test-utils'

/** The text of the `<dd>` after the `<dt>` named `label`, with its parts separated by spaces. */
export function field(wrapper: VueWrapper, label: string): string | undefined {
  const term = wrapper.findAll('dt').find((dt) => dt.text() === label)
  const definition = term?.element.nextElementSibling
  if (!definition) return undefined
  const walker = document.createTreeWalker(definition, NodeFilter.SHOW_TEXT)
  const parts: string[] = []
  while (walker.nextNode()) parts.push(walker.currentNode.textContent?.trim() ?? '')
  return parts.filter(Boolean).join(' ')
}
