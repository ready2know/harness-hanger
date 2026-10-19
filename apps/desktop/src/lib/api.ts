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

/** Mirrors `hanger_core::marketplace::discover::DiscoveredPlugin`: a plugin as a manifest lists it. */
export interface DiscoveredPlugin {
  name: string
  description: string | null
  version: string | null
  category: string | null
  /** The entry's `keywords` and `tags`. */
  keywords: string[]
}

/** Mirrors `hanger_core::marketplace::discover::DiscoveredMarketplace`: a public marketplace on GitHub. */
export interface DiscoveredMarketplace {
  owner: string
  repo: string
  /** The repository on GitHub. */
  url: string
  /** The repository's description, else the manifest's. */
  description: string | null
  stars: number
  /** The last push, as an RFC 3339 timestamp. */
  pushedAt: string | null
  /** An SPDX ID, such as `MIT`, or the license's name. */
  license: string | null
  archived: boolean
  /** The marketplace's name in its manifest. */
  name: string
  pluginCount: number
  plugins: DiscoveredPlugin[]
  /** When GitHub was asked about it, as an RFC 3339 timestamp. */
  fetchedAt: string
  curated: boolean
  /** Its ID in the catalog, when the catalog has it. */
  catalogId: number | null
  /** The plugins whose name, description, category, or keywords hold every word of the search. */
  matchingPlugins: string[]
  /** Other repositories with the same manifest name and plugins, as `owner/repo`, left out. */
  copies: string[]
}

/** Mirrors `hanger_core::marketplace::discover::Problem`: why GitHub could not be asked. */
export type DiscoverProblem =
  | { kind: 'offline'; message: string }
  /** `resetAt` is when the limit resets, as an RFC 3339 timestamp. */
  | { kind: 'rateLimited'; resetAt: string | null }
  | { kind: 'unauthorized'; message: string }
  | { kind: 'github'; status: number; message: string }

/** Mirrors `hanger_core::marketplace::discover::RateLimit`. */
export interface RateLimit {
  resource: string
  limit: number
  remaining: number
  resetAt: string
}

/**
 * Mirrors `hanger_core::marketplace::discover::SearchMethod`. `code` searches every manifest on
 * GitHub and needs a token; `topic` searches repositories with the `claude-code-marketplace`
 * topic, without one.
 */
export type SearchMethod = 'code' | 'topic' | 'curated'

/** Mirrors `hanger_core::marketplace::discover::DiscoverResults`. */
export interface DiscoverResults {
  /** The search, or `null` for the curated list. */
  query: string | null
  method: SearchMethod
  /** Whether GitHub is asked with a token. */
  signedIn: boolean
  /** The GitHub account that GitHub is asked as, when it is known without asking GitHub. */
  account: string | null
  /** The most starred first. */
  results: DiscoveredMarketplace[]
  /** How many repositories GitHub matched; it returns the first 50. */
  totalCount: number | null
  /** When GitHub was searched, as an RFC 3339 timestamp. */
  searchedAt: string | null
  /** Whether the GitHub results come from an earlier search instead of a new request. */
  fromCache: boolean
  /** Why GitHub could not be asked. The results then hold only what was found before. */
  problem: DiscoverProblem | null
  rateLimit: RateLimit | null
}

/**
 * Public marketplaces on GitHub that match `query`, or the curated ones when it is empty. GitHub
 * is asked unless the same search ran within the hour, or always with `refresh`.
 */
export function discoverMarketplaces(query: string, refresh = false): Promise<DiscoverResults> {
  return invoke<DiscoverResults>('discover_marketplaces', { query, refresh })
}

/** Mirrors `hanger_core::github::TokenSource`: where the GitHub token in use comes from. */
export type TokenSource =
  /** `GH_TOKEN` or `GITHUB_TOKEN`, which wins over the others. */
  | { kind: 'env'; variable: string }
  /** Saved in the system keychain, by signing in or pasting a token. */
  | { kind: 'saved' }
  /** The login of the `gh` command. */
  | { kind: 'gh' }

/** Mirrors `hanger_core::github::GhStatus`. */
export interface GhStatus {
  signedIn: boolean
  login: string | null
}

/** Mirrors `hanger_core::github::AccountStatus`: which GitHub account harness-hanger uses. */
export interface AccountStatus {
  /** `null` when there is no token. */
  source: TokenSource | null
  /** The account of the token in use, when harness-hanger knows it without asking GitHub. */
  login: string | null
  /** Whether a token is saved in the keychain, also when another one is in use. */
  saved: boolean
  savedLogin: string | null
  /** `null` when the `gh` command is not installed. */
  gh: GhStatus | null
  /** Whether this build can sign in in the browser. */
  browserSignIn: boolean
  /** Why the system keychain cannot be read, if it cannot. */
  keychainError: string | null
}

/** A code that the person enters at `verificationUri` to finish signing in. */
export interface SignInPrompt {
  userCode: string
  verificationUri: string
}

/** Mirrors `hanger_core::github::DeviceCode`: a sign-in in the browser that has started. */
export interface DeviceCode extends SignInPrompt {
  /** What the sign-in is asked about with. Not for showing. */
  deviceCode: string
  /** Seconds until the code expires. */
  expiresIn: number
  /** Seconds to wait between two questions. */
  interval: number
}

/** Mirrors `hanger_core::github::DevicePoll`. */
export type DevicePoll =
  { state: 'pending'; interval: number } | { state: 'done'; status: AccountStatus }

/** Which GitHub account harness-hanger uses. Asks GitHub nothing. */
export function getGitHubStatus(): Promise<AccountStatus> {
  return invoke<AccountStatus>('github_status')
}

/** Checks `token` with GitHub and saves it in the system keychain. */
export function saveGitHubToken(token: string): Promise<AccountStatus> {
  return invoke<AccountStatus>('github_save_token', { token })
}

/** Forgets the saved token. One from the environment or from `gh` stays in use. */
export function signOutOfGitHub(): Promise<AccountStatus> {
  return invoke<AccountStatus>('github_sign_out')
}

/** Asks GitHub which account the token in use belongs to, and returns its login. */
export function checkGitHubToken(): Promise<string> {
  return invoke<string>('github_check')
}

export function startBrowserSignIn(): Promise<DeviceCode> {
  return invoke<DeviceCode>('github_start_browser_sign_in')
}

/** Asks whether the person entered the code yet. The token is saved once they did. */
export function pollBrowserSignIn(deviceCode: string, interval: number): Promise<DevicePoll> {
  return invoke<DevicePoll>('github_poll_browser_sign_in', { deviceCode, interval })
}

/** Starts `gh auth login` in the browser, and returns the code that `gh` printed. */
export function startGhSignIn(): Promise<SignInPrompt> {
  return invoke<SignInPrompt>('github_start_gh_sign_in')
}

/** The account once `gh` finished signing in, or `null` while it waits. */
export function pollGhSignIn(): Promise<AccountStatus | null> {
  return invoke<AccountStatus | null>('github_poll_gh_sign_in')
}

export function cancelGhSignIn(): Promise<void> {
  return invoke<void>('github_cancel_gh_sign_in')
}

export function removeMarketplace(id: number): Promise<void> {
  return invoke<void>('remove_marketplace', { id })
}
