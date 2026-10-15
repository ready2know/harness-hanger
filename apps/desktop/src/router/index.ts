import { createRouter, createWebHashHistory, type RouteRecordRaw } from 'vue-router'

import CatalogComponentsView from '@/views/CatalogComponentsView.vue'
import DashboardView from '@/views/DashboardView.vue'
import MarketplacePreviewView from '@/views/MarketplacePreviewView.vue'
import MarketplaceView from '@/views/MarketplaceView.vue'
import PlaceholderView from '@/views/PlaceholderView.vue'
import PluginsView from '@/views/PluginsView.vue'
import ScopeView from '@/views/ScopeView.vue'
import SettingsView from '@/views/SettingsView.vue'

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
  { path: '/agents', name: 'agents', component: CatalogComponentsView, props: { kind: 'agents' } },
  { path: '/plugins', name: 'plugins', component: PluginsView },
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
