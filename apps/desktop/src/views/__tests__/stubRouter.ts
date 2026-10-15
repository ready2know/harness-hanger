import { createMemoryHistory, createRouter, type RouteRecordRaw } from 'vue-router'

import { routes } from '@/router'

const Stub = { render: () => null }

/** A router with the app's routes, each showing nothing, for a view that a test mounts itself. */
export function stubRouter() {
  return createRouter({
    history: createMemoryHistory(),
    routes: routes.map((route) => ({ ...route, component: Stub }) as RouteRecordRaw),
  })
}
