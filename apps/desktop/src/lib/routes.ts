import type { RouteLocationRaw } from 'vue-router'

import type { ComponentKind } from '@/lib/api'

/**
 * The page of a plugin. A harness's own skills, agents, and commands use their harness's ID as
 * the plugin name.
 */
export function pluginRoute(marketplaceId: number, plugin: string): RouteLocationRaw {
  return { name: 'plugin', params: { marketplace: marketplaceId, plugin } }
}

/** The page of a skill, an agent, or a command. Each kind's route is named after it. */
export function componentRoute(
  kind: ComponentKind,
  marketplaceId: number,
  plugin: string,
  name: string,
): RouteLocationRaw {
  return { name: kind, params: { marketplace: marketplaceId, plugin, name } }
}
