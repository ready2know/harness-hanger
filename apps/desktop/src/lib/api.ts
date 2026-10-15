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
  | { kind: 'github'; owner: string; repo: string }
  | { kind: 'local'; path: string }
  /** A harness's own skills, agents, and commands, read in place. `harness` is its ID. */
  | { kind: 'harness'; harness: string }

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
  | { kind: 'local'; path: string }
  | { kind: 'external'; source: string; location: string | null }
  /** Not a plugin: the components that a harness loads for the user on their own. */
  | { kind: 'harness'; harness: string }

/** Mirrors `hanger_core::marketplace::PluginPreview`. */
export interface PluginPreview {
  name: string
  description: string | null
  version: string | null
  author: string | null
  category: string | null
  /** The manifest's `keywords` and the marketplace entry's `tags`. */
  keywords: string[]
  homepage: string | null
  /** The plugin's own repository, which can differ from the marketplace's. */
  repository: string | null
  license: string | null
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

/** Mirrors `hanger_core::claude_code::PluginStatus`: the plugin's state for the user, in every project. */
export type PluginStatus = 'enabled' | 'disabled' | 'notInstalled'

/** Mirrors `hanger_core::marketplace::CatalogPlugin`. */
export interface CatalogPlugin {
  marketplaceId: number
  /** Claude Code identifies the plugin as `plugin@marketplace`. */
  marketplace: string
  status: PluginStatus
  plugin: PluginPreview
}

/** Mirrors `hanger_core::marketplace::CatalogPlugins`. */
export interface CatalogPlugins {
  plugins: CatalogPlugin[]
  /** Marketplaces left out because they cannot be read. Each one's `error` says why. */
  unreadable: MarketplaceSummary[]
}

/** Every plugin in the catalog's marketplaces, with its status in Claude Code. */
export function listCatalogPlugins(): Promise<CatalogPlugins> {
  return invoke<CatalogPlugins>('list_catalog_plugins')
}

/** Mirrors `hanger_core::marketplace::PluginDetails`. */
export interface PluginDetails {
  marketplace: MarketplaceSummary
  status: PluginStatus
  plugin: PluginPreview
  /** The README in the plugin's directory, when there is one that can be read. */
  readme: string | null
  /** The plugin in a browser: its directory on GitHub, or the repository it is installed from. */
  webUrl: string | null
}

/** `plugin` is its name in the catalog marketplace `marketplace`. */
export function getPluginDetails(marketplace: number, plugin: string): Promise<PluginDetails> {
  return invoke<PluginDetails>('plugin_details', { marketplace, plugin })
}

/** Mirrors `hanger_core::marketplace::ComponentKind`. */
export type ComponentKind = 'skill' | 'agent' | 'command'

/** Mirrors `hanger_core::marketplace::ComponentFile`. */
export interface ComponentFile {
  /** The whole file. */
  text: string
  /** The top-level frontmatter fields, as YAML reads them. */
  frontmatter: Record<string, unknown>
  /** The Markdown after the frontmatter. */
  body: string
}

/** Mirrors `hanger_core::marketplace::ComponentDetails`. */
export interface ComponentDetails {
  marketplace: MarketplaceSummary
  /** The status of the plugin that provides it. */
  status: PluginStatus
  plugin: PluginPreview
  kind: ComponentKind
  component: Component
  /** `null` for a command written inline in a manifest, or when `fileError` is set. */
  file: ComponentFile | null
  /** Why the file cannot be read. */
  fileError: string | null
  /** The file on GitHub, for a marketplace cloned from there. */
  webUrl: string | null
}

/** A skill, an agent, or a command of `plugin` in the catalog marketplace `marketplace`. */
export function getComponentDetails(
  marketplace: number,
  plugin: string,
  kind: ComponentKind,
  name: string,
): Promise<ComponentDetails> {
  return invoke<ComponentDetails>('component_details', { marketplace, plugin, kind, name })
}

/** Mirrors `hanger_core::harness::FoundMarketplace`. */
export interface FoundMarketplace {
  name: string
  /** `null` when the catalog cannot use its kind of source; `problem` says why. */
  source: MarketplaceSource | null
  problem: string | null
  installedPlugins: string[]
  inCatalog: boolean
}

/** Mirrors `hanger_core::harness::FoundStandalone`. */
export interface FoundStandalone {
  source: MarketplaceSource
  path: string
  skills: number
  agents: number
  commands: number
  inCatalog: boolean
}

/** Mirrors `hanger_core::harness::Discovery`: what one harness has to import. */
export interface Discovery {
  harness: string
  name: string
  /** `null` when the harness is not on this machine. */
  configDir: string | null
  marketplaces: FoundMarketplace[]
  standalone: FoundStandalone | null
  /** `plugin@marketplace` IDs whose marketplace the harness does not describe. */
  unmatchedPlugins: string[]
}

/** Mirrors `hanger_core::harness::ImportItem`. */
export type ImportItem = { kind: 'marketplace'; name: string } | { kind: 'standalone' }

export function getImportCandidates(): Promise<Discovery[]> {
  return invoke<Discovery[]>('import_candidates')
}

/** Imports one item from a harness. A marketplace's clone can take a while. */
export function importFromHarness(harness: string, item: ImportItem): Promise<MarketplaceSummary> {
  return invoke<MarketplaceSummary>('import_from_harness', { harness, item })
}

/** Mirrors `hanger_core::projects::Project`. */
export interface Project {
  id: number
  /** The folder's name. */
  name: string
  path: string
  addedAt: string
  /** Why the folder cannot be used, for example because it was moved. */
  error: string | null
}

export function listProjects(): Promise<Project[]> {
  return invoke<Project[]>('list_projects')
}

/** `path` is a folder path, which can start with `~`. */
export function addProject(path: string): Promise<Project> {
  return invoke<Project>('add_project', { path })
}

export function removeProject(id: number): Promise<void> {
  return invoke<void>('remove_project', { id })
}

/**
 * Mirrors `hanger_core::harness::claude_code::Scope`: where a plugin is installed, and which
 * settings switch it. `user` applies everywhere, `project` is shared through the project's
 * `.claude/settings.json`, and `local` is the person's own `.claude/settings.local.json`.
 */
export type Scope = 'user' | 'project' | 'local'

/** Mirrors `hanger_core::harness::claude_code::ScopedPlugin`. */
export interface ScopedPlugin {
  /** `plugin@marketplace`. */
  id: string
  name: string
  marketplace: string
  installedIn: Scope
  /** Whether Claude Code loads it here. */
  enabled: boolean
  /** The settings that decided `enabled`. `null` when none mention it, which leaves it off. */
  decidedBy: Scope | null
  version: string | null
  description: string | null
}

/** Mirrors `hanger_core::harness::claude_code::ScopeState`. */
export interface ScopeState {
  /** The project's folder, or `null` for the global setup. */
  project: string | null
  plugins: ScopedPlugin[]
  standalone: PluginPreview | null
  warnings: ScanWarning[]
}

/** What applies in a project, or globally when `project` is `null`. */
export function getScopeState(project: number | null): Promise<ScopeState> {
  return invoke<ScopeState>('scope_state', { project })
}

/**
 * Switches `plugin` (`plugin@marketplace`) on or off through Claude Code: in a project, or for the
 * user, in every project whose own settings do not decide it, when `project` is `null`.
 */
export function setPluginEnabled(
  project: number | null,
  plugin: string,
  enabled: boolean,
): Promise<void> {
  return invoke<void>('set_plugin_enabled', { project, plugin, enabled })
}

/** Installs `plugin` from the catalog marketplace `marketplace` into a project, for everyone. */
export function installPlugin(project: number, marketplace: number, plugin: string): Promise<void> {
  return invoke<void>('install_plugin', { project, marketplace, plugin })
}

/** Uninstalls a plugin that is installed in a project. */
export function uninstallPlugin(project: number, plugin: string): Promise<void> {
  return invoke<void>('uninstall_plugin', { project, plugin })
}

export function removeMarketplace(id: number): Promise<void> {
  return invoke<void>('remove_marketplace', { id })
}
