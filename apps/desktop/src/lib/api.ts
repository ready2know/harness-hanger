import { invoke } from '@tauri-apps/api/core'

/** Mirrors `hanger_core::AppInfo`. */
export interface AppInfo {
  version: string
  claudeConfigDir: string | null
}

export function getAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>('app_info')
}

/** Mirrors `hanger_core::marketplace::MarketplaceSource`. */
export type MarketplaceSource =
  { kind: 'github'; owner: string; repo: string } | { kind: 'local'; path: string }

/** Mirrors `hanger_core::marketplace::MarketplaceSummary`. */
export interface MarketplaceSummary {
  id: number
  source: MarketplaceSource
  addedAt: string
  root: string
  name: string | null
  description: string | null
  pluginCount: number
  error: string | null
}

/** Mirrors `hanger_core::marketplace::Component`: a skill, an agent, or a command. */
export interface Component {
  name: string
  description: string | null
  path: string | null
}

/** Mirrors `hanger_core::marketplace::PluginSource`. */
export type PluginSource =
  { kind: 'local'; path: string } | { kind: 'external'; source: string; location: string | null }

/** Mirrors `hanger_core::marketplace::PluginPreview`. */
export interface PluginPreview {
  name: string
  description: string | null
  version: string | null
  author: string | null
  category: string | null
  source: PluginSource
  skills: Component[]
  agents: Component[]
  commands: Component[]
}

/** Mirrors `hanger_core::marketplace::ScanWarning`. */
export interface ScanWarning {
  path: string
  message: string
}

/** Mirrors `hanger_core::marketplace::MarketplacePreview`. */
export interface MarketplacePreview {
  name: string
  description: string | null
  owner: string | null
  plugins: PluginPreview[]
  warnings: ScanWarning[]
}

/** Mirrors `hanger_core::marketplace::MarketplaceDetails`. */
export interface MarketplaceDetails {
  summary: MarketplaceSummary
  /** `null` when the manifest cannot be read. `summary.error` says why. */
  preview: MarketplacePreview | null
}

export function listMarketplaces(): Promise<MarketplaceSummary[]> {
  return invoke<MarketplaceSummary[]>('list_marketplaces')
}

/** `source` is a GitHub URL, `owner/repo`, or a local directory path. A GitHub source is cloned. */
export function addMarketplace(source: string): Promise<MarketplaceSummary> {
  return invoke<MarketplaceSummary>('add_marketplace', { source })
}

export function getMarketplaceDetails(id: number): Promise<MarketplaceDetails> {
  return invoke<MarketplaceDetails>('marketplace_details', { id })
}

export function removeMarketplace(id: number): Promise<void> {
  return invoke<void>('remove_marketplace', { id })
}
