import { createRouter, createWebHashHistory, type RouteRecordRaw } from 'vue-router'

import type { ComponentKind } from '@/lib/api'
import CatalogComponentsView from '@/views/CatalogComponentsView.vue'
import ComponentView from '@/views/ComponentView.vue'
import DashboardView from '@/views/DashboardView.vue'
import MarketplacePreviewView from '@/views/MarketplacePreviewView.vue'
import MarketplaceView from '@/views/MarketplaceView.vue'
import PlaceholderView from '@/views/PlaceholderView.vue'
import PluginsView from '@/views/PluginsView.vue'
import PluginView from '@/views/PluginView.vue'
import ScopeView from '@/views/ScopeView.vue'
import SettingsView from '@/views/SettingsView.vue'

/** The page of a skill, an agent, or a command, below its kind's section, such as `/skills`. */
function componentPage(kind: ComponentKind, section: string): RouteRecordRaw {
  return {
    path: `/${section}/:marketplace(\\d+)/:plugin/:name`,
    name: kind,
    component: ComponentView,
    props: (route) => ({
      kind,
      marketplaceId: Number(route.params.marketplace),
      plugin: String(route.params.plugin),
      name: String(route.params.name),
    }),
  }
}

export const routes: RouteRecordRaw[] = [
  { path: '/', name: 'dashboard', component: DashboardView },
  { path: '/marketplace', name: 'marketplace', component: MarketplaceView },
  {
    path: '/marketplace/:id(\\d+)',
    name: 'marketplace-preview',
    component: MarketplacePreviewView,
    props: (route) => ({ id: Number(route.params.id) }),
  },
  { path: '/skills', name: 'skills', component: CatalogComponentsView, props: { kind: 'skills' } },
  componentPage('skill', 'skills'),
  { path: '/agents', name: 'agents', component: CatalogComponentsView, props: { kind: 'agents' } },
  componentPage('agent', 'agents'),
  // Commands have no list of their own yet; their marketplace's page lists them.
  componentPage('command', 'commands'),
  { path: '/plugins', name: 'plugins', component: PluginsView },
  {
    path: '/plugins/:marketplace(\\d+)/:plugin',
    name: 'plugin',
    component: PluginView,
    props: (route) => ({
      marketplaceId: Number(route.params.marketplace),
      name: String(route.params.plugin),
    }),
  },
  {
    path: '/collections',
    name: 'collections',
    component: PlaceholderView,
    props: {
      title: 'All collections',
      description:
        'Named sets of skills, agents, and plugins. Apply one to a project in one step, or share it as a file.',
    },
  },
  {
    path: '/collections/new',
    name: 'collection-new',
    component: PlaceholderView,
    props: {
      title: 'Add collection',
      description: 'Pick skills, agents, and plugins from your catalog and name the set.',
    },
  },
  { path: '/global', name: 'global', component: ScopeView, props: { projectId: null } },
  {
    path: '/projects/:id(\\d+)',
    name: 'project',
    component: ScopeView,
    props: (route) => ({ projectId: Number(route.params.id) }),
  },
  { path: '/settings', name: 'settings', component: SettingsView },
]

// Hash history: the desktop app is served from a custom protocol with no server-side fallback.
const router = createRouter({ history: createWebHashHistory(), routes })

export default router
