import { createRouter, createWebHashHistory, type RouteRecordRaw } from 'vue-router'

import CatalogComponentsView from '@/views/CatalogComponentsView.vue'
import DashboardView from '@/views/DashboardView.vue'
import MarketplacePreviewView from '@/views/MarketplacePreviewView.vue'
import MarketplaceView from '@/views/MarketplaceView.vue'
import PlaceholderView from '@/views/PlaceholderView.vue'
import PluginsView from '@/views/PluginsView.vue'

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
    path: '/settings',
    name: 'settings',
    component: PlaceholderView,
    props: { title: 'Settings' },
  },
]

// Hash history: the desktop app is served from a custom protocol with no server-side fallback.
const router = createRouter({ history: createWebHashHistory(), routes })

export default router
