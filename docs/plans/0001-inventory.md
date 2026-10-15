# Plan 0001: Inventory (M1)

Goal: answer "what is installed, and where does it come from?" for Claude Code, in the CLI and in
the desktop app. This milestone is **read-only**: it never writes harness files.

Every later milestone builds on this inventory. Statistics join usage events to components, and
project management changes the files this scan reads.

## Sources to scan

The table below was observed on a developer machine on 2026-10-15. Before implementing each row,
confirm it against the current Claude Code documentation.

| Kind | User scope | Project scope | Provided by a plugin |
| --- | --- | --- | --- |
| Skill | `~/.claude/skills/<name>/SKILL.md` | `<project>/.claude/skills/<name>/SKILL.md` | `<plugin>/skills/<name>/SKILL.md` |
| Agent | `~/.claude/agents/*.md` | `<project>/.claude/agents/*.md` | `<plugin>/agents/*.md` |
| Command | `~/.claude/commands/*.md` | `<project>/.claude/commands/*.md` | `<plugin>/commands/*.md` |
| Hook | `hooks` in `~/.claude/settings.json` | `hooks` in `.claude/settings.json` and `.claude/settings.local.json` | `<plugin>/hooks/hooks.json` |
| MCP server | `mcpServers` in `~/.claude.json`; per-project entries under `projects["<path>"].mcpServers` | `<project>/.mcp.json` | `<plugin>/.mcp.json` |
| Plugin | `enabledPlugins` in `~/.claude/settings.json`; `~/.claude/plugins/installed_plugins.json` (has `scope`, `installPath`, `version`) | `enabledPlugins` in `.claude/settings.json` | n/a |
| Marketplace | `~/.claude/plugins/known_marketplaces.json`; `extraKnownMarketplaces` in settings | `extraKnownMarketplaces` in `.claude/settings.json` | n/a |

Notes:

- `$CLAUDE_CONFIG_DIR` replaces `~/.claude` when it is set. `hanger_core::paths` already handles this.
- A plugin's manifest is `<installPath>/.claude-plugin/plugin.json`. It can declare custom paths
  for commands, agents, skills, hooks, and MCP servers. Support the default directories first,
  then the custom paths.
- Managed (enterprise) settings exist as a separate scope. Model the scope in the types now, but
  scanning it can wait.
- Precedence between scopes, for example a project setting that overrides a user setting, matters
  for M3. In M1, list every definition with its scope. Do not resolve "the effective value" yet.

## Domain model (`hanger-core`)

```rust
pub enum ComponentKind { Skill, Agent, Command, Hook, McpServer, Plugin, Marketplace }

pub enum Scope {
    Managed,
    User,
    Project,
    /// Personal per-project settings that are not committed (`settings.local.json`, `~/.claude.json` project entries).
    Local,
}

pub struct Component {
    /// Stable ID: harness, kind, scope, provider, and name. Statistics and the UI key on it.
    pub id: ComponentId,
    pub kind: ComponentKind,
    pub name: String,
    pub description: Option<String>,
    pub scope: Scope,
    /// Set when a plugin provides the component.
    pub plugin: Option<PluginRef>,
    /// File where the component is defined.
    pub source: PathBuf,
    /// `None` when the harness has no on/off state for this kind.
    pub enabled: Option<bool>,
}

pub struct Inventory {
    pub components: Vec<Component>,
    /// Problems that did not stop the scan, such as malformed frontmatter or unreadable files.
    pub warnings: Vec<ScanWarning>,
}

pub trait HarnessAdapter {
    fn id(&self) -> &'static str;
    fn scan(&self, ctx: &ScanContext) -> Inventory;
}

/// All inputs are explicit, so tests can point the scan at fixture directories.
pub struct ScanContext {
    pub config_dir: PathBuf,          // ~/.claude
    pub user_state_file: PathBuf,     // ~/.claude.json
    pub project_root: Option<PathBuf>,
}
```

Rules:

- **A bad file never fails the scan.** It becomes a `ScanWarning` with the path and the reason.
- **No environment reads inside the scanners.** Only `ScanContext` construction reads `$HOME` and
  `$CLAUDE_CONFIG_DIR`.
- **Never log or return secret values.** MCP server `env` and `headers` entries can contain tokens.
  The inventory exposes only key names.
- Serialize with camelCase. The same JSON is the CLI `--json` output and the Tauri payload.

## Dependencies to add

| Need | Candidate | Status on 2026-10-15 |
| --- | --- | --- |
| YAML frontmatter in skills, agents, and commands | `serde-saphyr` | 2.0.0, released 2026-10-13. `serde_yaml` is deprecated. Confirm that it is maintained before adding it. |
| JSON settings | `serde_json` | Already used in tests. |
| TypeScript types generated from Rust | `ts-rs` | 12.0.1. Add a test that exports the bindings, and fail CI when they drift. |
| Snapshot tests for the JSON output | `insta` | 1.49.0 |
| File dialog to pick a project in the app | `tauri-plugin-dialog` | Tauri 2 plugin. Add a matching capability permission. |

## CLI

```
harness-hanger ls [--kind <kind>]... [--scope <scope>]... [--project <path>] [--json]
harness-hanger show <component-id> [--json]
```

- `--project` defaults to the current directory if it contains `.claude/` or `.mcp.json`.
  Otherwise only user-level and plugin components are listed.
- `--json` prints `{ "components": [...], "warnings": [...] }`. The shape is a public contract:
  change it only in an additive way.
- Exit codes: `0` for success (even with warnings), `1` for an error, `2` for a usage error (clap).
- Text output is a table: kind, name, scope, source (plugin or file), and enabled.

## Desktop app

- Tauri command: `scan_inventory(project: Option<String>) -> Inventory`. It runs the scan on a
  blocking thread.
- An "Inventory" route with a table (shadcn-vue data table on TanStack Table), filters by kind and
  scope, and text search.
- A project picker that uses the dialog plugin. The last 10 projects are kept in a Pinia store and
  persisted.
- A detail panel that shows the source path, the frontmatter fields, and the plugin.

## Delivery steps

Each step is one PR with tests.

1. **Model and fixtures.** Add the domain types, `HarnessAdapter`, `ScanContext`, and fixture
   trees under `crates/hanger-core/tests/fixtures/` (a fake `~/.claude`, a fake `~/.claude.json`,
   and a fake project).
2. **File-based components.** Scan skills, agents, and commands at user and project scope, and
   parse their frontmatter.
3. **Settings-based components.** Hooks, `enabledPlugins`, and marketplaces from all settings files.
4. **Plugins.** Read `installed_plugins.json`, the plugin manifests, and the components each plugin
   provides.
5. **MCP servers.** Read `~/.claude.json` (user and per-project entries), `.mcp.json`, and
   plugin-provided servers. Redact secrets.
6. **CLI.** `ls` and `show`, text and `--json` output, and `insta` snapshots against the fixtures.
7. **Desktop.** The `scan_inventory` command, `ts-rs` bindings, the inventory view, the project
   picker, and component tests.

## Done when

- `harness-hanger ls --json` on a real machine lists every skill, agent, command, hook, MCP server,
  plugin, and marketplace that `/plugin`, `/agents`, and `/mcp` show in Claude Code.
- A malformed file produces a warning and the rest of the scan still succeeds.
- The desktop app shows the same data for user scope and for a project you pick.
- CI passes on macOS, Linux, and Windows, with fixture paths that work on Windows.

## Open questions

- How to show "disabled" for kinds that have no on/off switch in Claude Code, such as skills and
  agents. Permission rules like `Skill(name)` in `deny` are one option. Decide in M3. M1 reports
  `enabled: None`.
- Windows location of `~/.claude.json` when `$CLAUDE_CONFIG_DIR` is set. Confirm before step 5.
