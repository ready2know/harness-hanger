import type { CatalogPlugin, MarketplaceSummary, PluginPreview } from '@/lib/api'

function plugin(overrides: Partial<PluginPreview>): PluginPreview {
  return {
    name: 'plugin',
    description: null,
    version: null,
    author: null,
    category: null,
    source: { kind: 'local', path: '/m/plugin' },
    skills: [],
    agents: [],
    commands: [],
    ...overrides,
  }
}

function component(name: string, description: string) {
  return { name, description, path: `/m/${name}.md` }
}

/** Three plugins in two marketplaces, one for each status in Claude Code. */
export const catalogPlugins: CatalogPlugin[] = [
  {
    marketplaceId: 1,
    marketplace: 'acme',
    status: 'enabled',
    plugin: plugin({
      name: 'review',
      version: '1.2.0',
      description: 'Code review helpers.',
      skills: [
        component('review-pr', 'Reviews a pull request.'),
        component('lint', 'Runs linters.'),
      ],
      agents: [component('security', 'Finds security bugs.')],
    }),
  },
  {
    marketplaceId: 2,
    marketplace: 'tools',
    status: 'notInstalled',
    plugin: plugin({
      name: 'deploy',
      skills: [component('deploy-app', 'Deploys the app.')],
    }),
  },
  {
    marketplaceId: 1,
    marketplace: 'acme',
    status: 'disabled',
    plugin: plugin({ name: 'docs', skills: [component('write-docs', 'Writes docs.')] }),
  },
  {
    marketplaceId: 2,
    marketplace: 'tools',
    status: 'notInstalled',
    plugin: plugin({
      name: 'remote',
      source: { kind: 'external', source: 'github', location: 'tools/remote' },
    }),
  },
]

/** A local marketplace whose directory was moved. */
export const movedMarketplace: MarketplaceSummary = {
  id: 3,
  source: { kind: 'local', path: '/src/moved' },
  addedAt: '2026-10-15T12:00:00Z',
  root: '/src/moved',
  name: null,
  description: null,
  pluginCount: 0,
  error: '/src/moved is not a marketplace: the directory does not exist',
}
