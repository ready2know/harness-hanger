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
5. **Marketplaces.** Add marketplaces from GitHub or from a local clone, and preview the plugins,
   skills, agents, and commands that each one contains. Install from them into one or more agents
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
| Own storage | SQLite | Only for data the harness does not keep: usage events, backlog items, notes, the marketplace catalog, and the project list. |
| Usage statistics | A harness hook | `harness-hanger hook record` is installed as a Claude Code hook and writes events to SQLite. Statistics start from the moment the hook is installed. There is no import of old transcripts. Skills, subagents, slash commands, and MCP tools are all tracked from the start, and each event records whether the agent or a human started the component (decided on 2026-10-15, AIM-8). |
| "Planning" | A backlog of components | Not a declarative plan/apply model. A manifest and lock file may come later as an option. |
| Marketplace catalog | harness-hanger's own list, separate from Claude Code | Decided on 2026-10-15, so that you can look into a marketplace without registering it with Claude Code. Adding one does not change Claude Code's files. GitHub sources are shallow-cloned with the system `git` into the harness-hanger data directory. Local clones are read in place. Installing a plugin into Claude Code is a later step. |
| Importing from harnesses | One adapter per harness, read only | Decided on 2026-10-15. Each harness implements `HarnessAdapter`, which lists the marketplaces it knows, the plugins installed from them, and its own skills, agents, and commands. Claude Code is the first adapter. Importing adds catalog entries and never changes the harness. A GitHub marketplace is cloned from the copy that the harness keeps, so it needs no network, and its `origin` then points at GitHub. The harness's own components are a catalog entry that is read in place, like a local directory. |
| Changing the harness | Through its own CLI | Decided on 2026-10-15. harness-hanger switches, installs, and uninstalls Claude Code plugins with `claude plugin … --json`, run in the project folder; it does not edit Claude Code's files itself. Switching off a global plugin in a project writes the person's own `.claude/settings.local.json`, so it reaches no one else. A plugin from a GitHub marketplace is installed in the project scope, shared through `.claude/settings.json`, which also declares the marketplace for everyone. A plugin from a local-folder marketplace is installed only for the person, in `.claude/settings.local.json`, because the folder exists only on their machine. When Claude Code knows a different marketplace by the same name, installing is refused. `claude` gets `CLAUDE_CONFIG_DIR` only when harness-hanger was given a config directory explicitly, because Claude Code also moves its global `.claude.json` into a directory set that way. A plugin installed globally is not installed again in a project; it can only be switched off there. The Global page switches a plugin for the user with `claude plugin enable` or `disable --scope user` (changed on 2026-10-16; before, it only showed the global setup); a project whose own settings switch the plugin keeps its choice. Skills outside plugins have no switch yet: Claude Code turns them off only through `skillOverrides` in a settings file, which no `claude` command writes, so harness-hanger would have to edit Claude Code's files itself (decided on 2026-10-16). |
| Discover | GitHub search, with a local index | Decided on 2026-10-16 (AIM-18). Discover searches GitHub; it is not a hosted registry. With a GitHub token, GitHub's code search finds every repository with `.claude-plugin/marketplace.json` at its root, because code search needs a token. Without one, it searches repositories with the `claude-code-marketplace` topic, which finds fewer. Only `marketplace.json` is read, through GraphQL with a token and from raw.githubusercontent.com without one; the full preview comes after Add, from the usual shallow clone. Nothing goes to GitHub until the Discover page opens, which fetches the curated list, or a search runs, and GitHub gets only the query and the names of the repositories it returned. Searches are kept for an hour and the curated details for a day, in SQLite, and every manifest found stays in a local index that later searches also match by plugin names and descriptions, offline too. Results show the most stars first; the app also sorts by recent activity or plugin count. Forks, and copies with the same manifest name and plugins, are left out, keeping the curated one, else the one in the catalog, else the most starred. The curated list lives in the source code, so a change ships with a release. GitHub only for now; other hosts and repositories with skills but no marketplace (AIM-27, AIM-32) are not covered. After Add, the marketplace's page opens; the security audit (AIM-13) will show there. |
| GitHub account | A token from the environment, the system keychain, or `gh` | Decided on 2026-10-16 (AIM-18). The token comes from `GH_TOKEN` or `GITHUB_TOKEN`, which win as they do for `gh`; else from the system keychain (Keychain on macOS, Credential Manager on Windows, the Secret Service on Linux); else from the login of the `gh` command. Settings → GitHub, and `harness-hanger github login`, save one in the keychain in three ways: signing in in the browser through GitHub's device flow, with harness-hanger's own OAuth App, which asks for no scopes; running `gh auth login --web`, which signs `gh` itself in and does not change how git talks to GitHub; or pasting a personal access token, which is checked with GitHub before it is saved. The OAuth App's client ID is public, because the device flow needs no secret, so it is in the source; `HARNESS_HANGER_GITHUB_CLIENT_ID` replaces it when building, such as for a fork. Reading which account is in use asks GitHub nothing; only signing in, saving a token, and Check do. |
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
| M1: Inventory | Read-only scan of Claude Code components in user, project, and plugin scopes. `harness-hanger ls` and `show`, and an inventory view in the app. Plan: [`plans/0001-inventory.md`](plans/0001-inventory.md). A CLI contract for every command: stable error codes in the `--json` output, and the same `--dry-run` and `--yes` rules for the commands that write. | AIM-28 |
| M2: Usage statistics | `hook record`, `hook install` and `hook uninstall`, the SQLite schema, a stats CLI, and a stats view. Covers skills, subagents, slash commands, and MCP tools, with an agent vs. human split. A context budget: the estimated tokens that each component adds to every session, and the cost per run. Token counts for MCP tools come with the MCP servers page in M3. harness-hanger's own Claude Code plugin, which installs the same hooks with `/plugin install` and adds a skill that teaches agents the CLI. Diagnostics: rotating logs with secrets removed, and an export for bug reports. | AIM-8, AIM-11, AIM-19, AIM-30 |
| M3: Project scope management | Started: project folders in Settings, a Global page, and a page per project that shows the Claude Code plugins that apply there, with their standalone skills, agents, and commands, and that switches, installs, and uninstalls plugins through `claude plugin`. Remaining: the same for skills, agents, and commands outside plugins. An MCP servers page: the added, enabled, and available servers for each project and globally, a health check with each server's tools, and actions to add, remove, enable, and disable servers. A permissions page that lists, checks, and cleans up the allow, ask, and deny rules in every scope. Instructions (CLAUDE.md, AGENTS.md, and rule files) as a component kind, with the load order for each folder. An activity log of every change that harness-hanger makes, with undo. | AIM-6, AIM-14, AIM-22, AIM-23, AIM-29 |
| M4: Global dashboard and backlog | Edit user-level components. Backlog CRUD. A doctor that checks components for errors and conflicts, in the app, the CLI, and CI. A cleanup of unused plugins, stale caches, and leftover files, with a preview. Authoring: scaffold components from templates, and publish them as your own marketplace. | AIM-12, AIM-16, AIM-17 |
| M5: Marketplaces | Started: the catalog (add from GitHub or a local clone, list, remove), a preview of each marketplace's plugins, skills, agents, and commands, Skills, Agents, and Plugins pages that show whether Claude Code has each plugin enabled for the user, a page for each plugin, skill, agent, and command with its metadata, its text, and links to its file and to GitHub, and a Discover page that finds public marketplaces on GitHub without cloning them and adds one in a single action. Remaining: update clones, with a review of what changed before an update is applied; show hooks and MCP servers; install into Claude Code, Cursor, Hermes, Codex, and pi, globally or for one project; install safety: show what runs before an install, check that Claude Code fetched the commit that the user reviewed, and harden git and file reads; custom agents, defined by a name and their skills folders; installing a skill from a git repository, a folder, or an archive; progress and cancel for clones; a security audit of installed components; skill versions with history, diff, pin, and restore; update checks on a schedule that never apply an update on their own; more Discover sources: skills on skills.sh, and skills that websites publish in a well-known index; and moving an existing setup (MCP servers, skills, subagents, commands, and instructions) from one agent to another. | AIM-5, AIM-13, AIM-15, AIM-18, AIM-24, AIM-25, AIM-26, AIM-27, AIM-31, AIM-32, AIM-33 |
| M6: Collections | Named sets of components. Apply to a project as a one-time copy. Export to a file and import with a preview. | AIM-7 |
| M7: Tester | A Test tab on skill and subagent pages: folder, agent, and prompt, run in an embedded terminal, and a check that the component was used. Trigger evals: sets of prompts that check whether a skill or subagent triggers when it should, and not otherwise. An MCP inspector to call a server's tools, read its prompts and resources, and see the protocol log. | AIM-9, AIM-20, AIM-21 |
| M8: Workflows | Ordered steps that use skills, agents, and plugins. Run with one call, share as a file. Shared or per-step agent sessions. | AIM-10 |
| Later | harness-hanger's own MCP server (`harness-hanger mcp`), an optional manifest and lock file, release automation. | |

## Out of scope for now

- Hosting a marketplace or a registry, also for collections and workflows. Sharing is by file.
- Syncing components between machines.
- Scheduled runs of workflows.
- Telemetry sent anywhere. All data stays on the user's machine.
