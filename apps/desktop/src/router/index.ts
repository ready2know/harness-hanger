import { createRouter, createWebHashHistory } from 'vue-router'

import HomeView from '@/views/HomeView.vue'

// Hash history: the desktop app is served from a custom protocol with no server-side fallback.
const router = createRouter({
  history: createWebHashHistory(),
  routes: [{ path: '/', name: 'home', component: HomeView }],
})

export default router
