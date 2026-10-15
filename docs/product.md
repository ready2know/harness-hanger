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
- Does the agent pick up a skill on its own, or only when I ask for it by name?
- How do I give my setup, or a sequence of steps, to someone else?
- What components do I want to build next?

## Product

harness-hanger is a manager for harness components, with two surfaces over one Rust core:

- **Desktop app** (Tauri + Vue) for browsing, editing, and statistics.
- **CLI** (`harness-hanger`) for scripts and for agents. Every command supports `--json`.

### Capabilities

1. **Global dashboard.** All components installed for the user, across all sources. View, edit,
   and plan them.
2. **Backlog.** Track components you plan to build: idea, then draft, then published, with notes.
3. **Project scope.** Add any folder as a project, and see which components apply there and where
   each one comes from. Add new ones, and remove or disable others, for that project only.
4. **Usage statistics.** Record which skills, agents, commands, and MCP tools run, and whether the
   agent or a human started each one. Show what is used, how often, where, and what is never used.
5. **Marketplaces.** Browse known marketplaces and install from them into one or more agents
   (Claude Code, Cursor, Hermes, Codex, pi), globally or for one project.
6. **Collections.** Group components into a named set. Apply the set to a project in one action, or
   export it to a file and import it on another machine.
7. **Tester.** On a skill or subagent page, pick a folder, an agent, and a prompt, run the agent in
   an embedded terminal, and see whether it used the component.
8. **Workflows.** Describe an ordered sequence of steps that use skills, agents, and plugins. Run
   it with one call, or share it as a file.

## Audience

Open-source project. Users are developers who use AI coding harnesses daily and have more than a
handful of components. Contributors need clear docs, a predictable workflow, and CI on all three
desktop platforms.

## Decisions

| Topic | Decision | Notes |
| --- | --- | --- |
| Harnesses | Claude Code first, then Cursor, Hermes, Codex, and pi | Changed on 2026-10-15. The earlier decision was "Claude Code only for the first releases". Installing marketplace components into several agents is now a planned feature (AIM-5). Each agent gets its own `HarnessAdapter`. Claude Code stays the first and most complete adapter. |
| Source of truth | The harness's own files | harness-hanger reads and writes `settings.json`, `.claude/`, plugin directories, and so on directly. It never keeps a shadow copy of installed state. |
| Own storage | SQLite | Only for data the harness does not keep: usage events, backlog items, notes, and the project list. |
| Usage statistics | A harness hook | `harness-hanger hook record` is installed as a Claude Code hook and writes events to SQLite. Statistics start from the moment the hook is installed. There is no import of old transcripts. Skills, subagents, slash commands, and MCP tools are all tracked from the start, and each event records whether the agent or a human started the component (decided on 2026-10-15, AIM-8). |
| "Planning" | A backlog of components | Not a declarative plan/apply model. A manifest and lock file may come later as an option. |
| Applying a collection | A one-time copy into the project | Decided on 2026-10-15 (AIM-7). The project does not stay linked to the collection, so a later change to the collection does not change the project. |
| Tester output | An embedded terminal | Decided on 2026-10-15 (AIM-9). A PTY in the Rust core and xterm.js in the window. It works the same for every agent. A structured view of messages and tool calls is not planned yet. |
| Workflow sessions | A setting on each workflow | Decided on 2026-10-15 (AIM-10). The user chooses between one shared agent session for all steps and a new session for each step. A new session for each step is the default. |
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
- Each event records who started the component (`invoked_by`). It is `agent` when the model calls
  a tool: `Skill`, the subagent tool, or an `mcp__*` tool. It is `user` when the prompt starts with
  `/name`. Confirm the exact tool names against the current Claude Code docs, because they changed
  between versions.
- Events store the session ID, the project path, the component, `invoked_by`, and a timestamp.
  They never store prompt or tool content. From `UserPromptSubmit`, only the leading `/name` is
  kept.

## Roadmap

Features are tracked as issues in Linear, in the Harness Hanger project (team `AIM`). The
"Issues" column below uses those IDs. The order of M6 to M8 is not decided yet.

| Milestone | Scope | Issues |
| --- | --- | --- |
| M0: Skeleton | Workspace, tooling, CI, docs. `harness-hanger info`, and a desktop window that shows the same data. | |
| M1: Inventory | Read-only scan of Claude Code components in user, project, and plugin scopes. `harness-hanger ls` and `show`, and an inventory view in the app. Plan: [`plans/0001-inventory.md`](plans/0001-inventory.md). | |
| M2: Usage statistics | `hook record`, `hook install` and `hook uninstall`, the SQLite schema, a stats CLI, and a stats view. Covers skills, subagents, slash commands, and MCP tools, with an agent vs. human split. | AIM-8 |
| M3: Project scope management | Add any folder as a project and see the harness it uses (read-only). Then add, remove, enable, and disable components per project. Safe writes to settings files. | AIM-6 |
| M4: Global dashboard and backlog | Edit user-level components. Backlog CRUD. | |
| M5: Marketplaces | List known marketplaces, browse their plugins, and install and update them in Claude Code, Cursor, Hermes, Codex, and pi, globally or for one project. | AIM-5 |
| M6: Collections | Named sets of components. Apply to a project as a one-time copy. Export to a file and import with a preview. | AIM-7 |
| M7: Tester | A Test tab on skill and subagent pages: folder, agent, and prompt, run in an embedded terminal, and a check that the component was used. | AIM-9 |
| M8: Workflows | Ordered steps that use skills, agents, and plugins. Run with one call, share as a file. Shared or per-step agent sessions. | AIM-10 |
| Later | MCP server, an optional manifest and lock file, release automation. | |

## Out of scope for now

- Hosting a marketplace or a registry, also for collections and workflows. Sharing is by file.
- Syncing components between machines.
- Scheduled runs of workflows.
- Telemetry sent anywhere. All data stays on the user's machine.
