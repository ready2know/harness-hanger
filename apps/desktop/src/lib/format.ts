import type { MarketplaceSource } from '@/lib/api'

/** `1 skill`, `3 skills`. */
export function count(n: number, noun: string): string {
  return `${n} ${n === 1 ? noun : `${noun}s`}`
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
