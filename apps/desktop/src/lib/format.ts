import type {
  MarketplaceSource,
  MarketplaceSummary,
  PluginPreview,
  PluginStatus,
  Project,
} from '@/lib/api'

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
  switch (source.kind) {
    case 'github':
      return `github.com/${source.owner}/${source.repo}`
    case 'local':
      return source.path
    case 'harness':
      return `${source.harness} user directory`
  }
}

/** Where a catalog entry comes from: its source, or the directory of a harness's own components. */
export function formatLocation(summary: MarketplaceSummary): string {
  return summary.source.kind === 'harness' ? summary.root : formatSource(summary.source)
}

/** `path` relative to `root` when it is inside it, else `path` unchanged. */
export function relativePath(path: string, root: string): string {
  const prefix = root.replace(/[\\/]+$/, '')
  const rest = path.slice(prefix.length)
  return path.startsWith(prefix) && /^[\\/]/.test(rest) ? rest.slice(1) : path
}

/** Each project's name, with its parent folder when another project has the same name. */
export function projectLabels(projects: Project[]): Map<number, string> {
  const seen = new Map<string, number>()
  for (const project of projects) seen.set(project.name, (seen.get(project.name) ?? 0) + 1)
  return new Map(
    projects.map((project) => {
      if (seen.get(project.name)! < 2) return [project.id, project.name]
      const parts = project.path.split(/[\\/]/).filter(Boolean)
      const parent = parts[parts.length - 2]
      return [project.id, parent ? `${parent}/${project.name}` : project.path]
    }),
  )
}
