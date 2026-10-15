# harness-hanger: product brief

Status: agreed during the project kickoff interview on 2026-10-15. Update this file when a decision
changes, and record the reason.

## Problem

People who work with AI coding harnesses (Claude Code first) collect many components over time:
skills, subagents, slash commands, hooks, MCP servers, plugins, and the marketplaces they come
from. These components are spread across user-level and project-level files, plugin caches, and
settings files. There is no single place to answer simple questions:

- What is installed, and where does it come from?
- What applies in *this* project, and how do I add, remove, or disable something here?
- Which skills do I actually use, and which ones are dead weight?
- What components do I want to build next?

## Product

harness-hanger is a manager for harness components, with two surfaces over one Rust core:

- **Desktop app** (Tauri + Vue) for browsing, editing, and statistics.
- **CLI** (`harness-hanger`) for scripts and for agents. Every command supports `--json`.

### Capabilities

1. **Global dashboard.** All components installed for the user, across all sources. View, edit,
   and plan them.
2. **Backlog.** Track components you plan to build: idea, then draft, then published, with notes.
3. **Project scope.** Open a project and see which components apply there and where each one comes
   from. Add new ones, and remove or disable others, for that project only.
4. **Usage statistics.** Record which skills, agents, commands, and MCP tools run. Show what is
   used, how often, where, and what is never used.
5. **Marketplaces.** Browse known marketplaces and install from them.

## Audience

Open-source project. Users are developers who use AI coding harnesses daily and have more than a
handful of components. Contributors need clear docs, a predictable workflow, and CI on all three
desktop platforms.

## Decisions

| Topic | Decision | Notes |
| --- | --- | --- |
| Harnesses | Claude Code only for the first releases | The core has a `HarnessAdapter` boundary, so other harnesses (Codex CLI and others) can be added later. |
| Source of truth | The harness's own files | harness-hanger reads and writes `settings.json`, `.claude/`, plugin directories, and so on directly. It never keeps a shadow copy of installed state. |
| Own storage | SQLite | Only for data the harness does not keep: usage events, backlog items, notes. |
| Usage statistics | A harness hook | `harness-hanger hook record` is installed as a Claude Code hook and writes events to SQLite. Statistics start from the moment the hook is installed. There is no import of old transcripts. |
| "Planning" | A backlog of components | Not a declarative plan/apply model. A manifest and lock file may come later as an option. |
| Agent interface | CLI with `--json` first | An MCP server (`harness-hanger mcp`) comes in a later milestone. |
| Stack | Rust core and CLI; Tauri 2 desktop app with Vue 3, TypeScript, Vite, Pinia, Tailwind 4, shadcn-vue | One Cargo workspace and one pnpm workspace. |
| CLI binary name | `harness-hanger` | |
| Platforms | macOS, Linux, Windows | All three run in CI from day one. |
| License | MIT OR Apache-2.0 | |
| Process | Conventional Commits, lefthook git hooks, OSS community files | |

## Usage tracking design notes

- Claude Code hooks call `harness-hanger hook record` with the hook's JSON payload on stdin.
  Candidate events: `PostToolUse` for the `Skill` tool, subagent tools, and `mcp__*` tools;
  `UserPromptSubmit` for slash commands; `SessionStart` for session context.
- The command must be fast and must never break a session: it always exits 0, logs its own
  errors, and does not block on locks for long.
- Events store the session ID, the project path, the component, and a timestamp. They never store
  prompt or tool content.

## Roadmap

| Milestone | Scope |
| --- | --- |
| M0: Skeleton | Workspace, tooling, CI, docs. `harness-hanger info`, and a desktop window that shows the same data. |
| M1: Inventory | Read-only scan of Claude Code components in user, project, and plugin scopes. `harness-hanger ls` and `show`, and an inventory view in the app. Plan: [`plans/0001-inventory.md`](plans/0001-inventory.md). |
| M2: Usage statistics | `hook record`, `hook install` and `hook uninstall`, the SQLite schema, a stats CLI, and a stats view. |
| M3: Project scope management | Add, remove, enable, and disable components per project. Safe writes to settings files. |
| M4: Global dashboard and backlog | Edit user-level components. Backlog CRUD. |
| M5: Marketplaces | List known marketplaces, browse their plugins, install and update plugins. |
| Later | MCP server, more harness adapters, an optional manifest and lock file, release automation. |

## Out of scope for now

- Hosting a marketplace or a registry.
- Syncing components between machines.
- Telemetry sent anywhere. All data stays on the user's machine.
