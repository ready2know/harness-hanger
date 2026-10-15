import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { enableAutoUnmount, flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { openUrl, revealItemInDir } from '@tauri-apps/plugin-opener'

import { getComponentDetails, type ComponentDetails, type ComponentKind } from '@/lib/api'
import ComponentView from '../ComponentView.vue'
import { plugin } from './catalogFixtures'
import { field } from './queries'
import { stubRouter } from './stubRouter'

vi.mock('@/lib/api', () => ({ getComponentDetails: vi.fn<typeof getComponentDetails>() }))
vi.mock('@tauri-apps/plugin-opener', () => ({
  openUrl: vi.fn<typeof openUrl>(),
  revealItemInDir: vi.fn<typeof revealItemInDir>(),
}))

enableAutoUnmount(afterEach)

beforeEach(() => vi.clearAllMocks())

const body = `# Review

See [the guide](https://example.com/guide) and [the checklist](./checklist.md).

<script>alert(1)</script>
`

const details: ComponentDetails = {
  marketplace: {
    id: 1,
    source: { kind: 'github', owner: 'acme', repo: 'tools' },
    addedAt: '2026-10-15T12:00:00Z',
    root: '/data/acme/tools',
    name: 'acme',
    description: null,
    pluginCount: 1,
    error: null,
  },
  status: 'enabled',
  plugin: plugin({ name: 'review', version: '1.2.0' }),
  kind: 'skill',
  component: {
    name: 'review-pr',
    description: 'Reviews a pull request.',
    path: '/data/acme/tools/plugins/review/skills/review-pr/SKILL.md',
  },
  file: {
    text: `---\nname: review-pr\n---\n${body}`,
    frontmatter: {
      name: 'review-pr',
      description: 'Reviews a pull request.',
      tags: ['review', 'git'],
      'allowed-tools': ['Read', 'Grep'],
      model: 'opus',
    },
    body,
  },
  fileError: null,
  webUrl: 'https://github.com/acme/tools/blob/abc/plugins/review/skills/review-pr/SKILL.md',
}

async function mountView(loaded: ComponentDetails = details, kind: ComponentKind = 'skill') {
  vi.mocked(getComponentDetails).mockResolvedValue(loaded)
  const wrapper = mount(ComponentView, {
    props: { kind, marketplaceId: 1, plugin: 'review', name: 'review-pr' },
    global: { plugins: [stubRouter()] },
  })
  await flushPromises()
  return wrapper
}

function button(wrapper: VueWrapper, text: string) {
  return wrapper.findAll('button').find((b) => b.text().includes(text))!
}

describe('ComponentView', () => {
  it('shows the component with its plugin, marketplace, file, and frontmatter', async () => {
    const wrapper = await mountView()

    expect(getComponentDetails).toHaveBeenCalledWith(1, 'review', 'skill', 'review-pr')
    expect(wrapper.get('h1').text()).toBe('review-pr')
    expect(wrapper.text()).toContain('Reviews a pull request.')
    expect(wrapper.text()).toContain('Enabled')
    expect(
      wrapper
        .get('[aria-label="Tags"]')
        .findAll('[data-slot="badge"]')
        .map((badge) => badge.text()),
    ).toEqual(['review', 'git'])
    expect(field(wrapper, 'Plugin')).toBe('review 1.2.0')
    expect(field(wrapper, 'Marketplace')).toBe('acme github.com/acme/tools')
    expect(field(wrapper, 'File')).toBe('plugins/review/skills/review-pr/SKILL.md')
    expect(field(wrapper, 'allowed-tools')).toBe('Read Grep')
    expect(field(wrapper, 'model')).toBe('opus')
    const hrefs = wrapper.findAll('a').map((link) => link.attributes('href'))
    expect(hrefs).toEqual(
      expect.arrayContaining(['/skills', '/plugins/1/review', '/marketplace/1']),
    )
  })

  it('renders the Markdown, and shows raw HTML as text', async () => {
    const wrapper = await mountView()

    const preview = wrapper.get('.prose')
    expect(preview.get('h1').text()).toBe('Review')
    expect(preview.find('script').exists()).toBe(false)
    expect(preview.text()).toContain('<script>alert(1)</script>')
  })

  it('opens web links in the browser, and follows no other link', async () => {
    const wrapper = await mountView()
    const links = wrapper.get('.prose').findAll('a')

    await links.find((link) => link.text() === 'the checklist')!.trigger('click')
    expect(openUrl).not.toHaveBeenCalled()

    await links.find((link) => link.text() === 'the guide')!.trigger('click')
    expect(openUrl).toHaveBeenCalledWith('https://example.com/guide')
  })

  it('shows the whole file as its source', async () => {
    const wrapper = await mountView()

    await wrapper
      .findAll('[role="tab"]')
      .find((tab) => tab.text() === 'Source')!
      .trigger('mousedown', { button: 0 })

    expect(wrapper.get('pre').text()).toContain('---\nname: review-pr\n---\n# Review')
  })

  it('shows the file in its folder and on GitHub', async () => {
    const wrapper = await mountView()

    await button(wrapper, 'Show in').trigger('click')
    expect(revealItemInDir).toHaveBeenCalledWith(details.component.path)

    await button(wrapper, 'View on GitHub').trigger('click')
    expect(openUrl).toHaveBeenCalledWith(details.webUrl)
  })

  it('says why the file cannot be read', async () => {
    const wrapper = await mountView({
      ...details,
      file: null,
      fileError: 'SKILL.md: it links outside the marketplace, so it is not read',
    })

    expect(wrapper.get('[role="alert"]').text()).toContain('links outside the marketplace')
    expect(wrapper.find('.prose').exists()).toBe(false)
  })

  it('names the harness that loads a standalone component, which has no plugin page', async () => {
    const wrapper = await mountView({
      ...details,
      marketplace: {
        ...details.marketplace,
        source: { kind: 'harness', harness: 'claude-code' },
        name: 'Claude Code',
      },
      plugin: plugin({ name: 'claude-code', source: { kind: 'harness', harness: 'claude-code' } }),
      webUrl: null,
    })

    expect(field(wrapper, 'Plugin')).toBe('Standalone: Claude Code loads it on its own')
    expect(wrapper.findAll('a').map((link) => link.attributes('href'))).not.toContain(
      '/plugins/1/claude-code',
    )
    expect(wrapper.text()).not.toContain('View on GitHub')
  })

  it('goes back to the marketplace from a command, which has no list of its own', async () => {
    const wrapper = await mountView(
      { ...details, kind: 'command', component: { ...details.component, path: null }, file: null },
      'command',
    )

    expect(wrapper.get('a').attributes('href')).toBe('/marketplace/1')
    expect(field(wrapper, 'File')).toBe('Written inline in the marketplace manifest')
  })

  it('shows why the component could not be loaded', async () => {
    vi.mocked(getComponentDetails).mockRejectedValue('review has no skill named review-pr')
    const wrapper = mount(ComponentView, {
      props: { kind: 'skill', marketplaceId: 1, plugin: 'review', name: 'review-pr' },
      global: { plugins: [stubRouter()] },
    })
    await flushPromises()

    expect(wrapper.get('[role="alert"]').text()).toContain('no skill named review-pr')
  })
})
