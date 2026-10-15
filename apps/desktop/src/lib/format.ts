import type { MarketplaceSource, PluginPreview, PluginStatus } from '@/lib/api'

/** `1 skill`, `3 skills`. */
export function count(n: number, noun: string): string {
  return `${n} ${n === 1 ? noun : `${noun}s`}`
}

export const statusLabels: Record<PluginStatus, string> = {
  enabled: 'Enabled',
  disabled: 'Disabled',
  notInstalled: 'Not installed',
}

/** `2 skills · 1 agent`, or why there is nothing to count. */
export function pluginContents(plugin: PluginPreview): string {
  if (plugin.source.kind === 'external') return 'Installed from elsewhere; contents unknown'
  const parts = [
    plugin.skills.length && count(plugin.skills.length, 'skill'),
    plugin.agents.length && count(plugin.agents.length, 'agent'),
    plugin.commands.length && count(plugin.commands.length, 'command'),
  ].filter(Boolean)
  return parts.length ? parts.join(' · ') : 'No skills, agents, or commands'
}

export function formatSource(source: MarketplaceSource): string {
  return source.kind === 'github' ? `github.com/${source.owner}/${source.repo}` : source.path
}

/** `path` relative to `root` when it is inside it, else `path` unchanged. */
export function relativePath(path: string, root: string): string {
  const prefix = root.replace(/[\\/]+$/, '')
  const rest = path.slice(prefix.length)
  return path.startsWith(prefix) && /^[\\/]/.test(rest) ? rest.slice(1) : path
}
