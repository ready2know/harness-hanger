import { createRouter, createWebHashHistory, type RouteRecordRaw } from 'vue-router'

import DashboardView from '@/views/DashboardView.vue'
import PlaceholderView from '@/views/PlaceholderView.vue'

export const routes: RouteRecordRaw[] = [
  { path: '/', name: 'dashboard', component: DashboardView },
  {
    path: '/marketplace',
    name: 'marketplace',
    component: PlaceholderView,
    props: { title: 'Marketplace' },
  },
  { path: '/skills', name: 'skills', component: PlaceholderView, props: { title: 'Skills' } },
  { path: '/agents', name: 'agents', component: PlaceholderView, props: { title: 'Agents' } },
  { path: '/plugins', name: 'plugins', component: PlaceholderView, props: { title: 'Plugins' } },
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
