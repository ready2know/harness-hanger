# harness-hanger

[![CI](https://github.com/ready2know/harness-hanger/actions/workflows/ci.yml/badge.svg)](https://github.com/ready2know/harness-hanger/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)

A manager for AI coding-harness components: skills, agents, slash commands, hooks, MCP servers,
plugins, and the marketplaces they come from.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/project-dark.png">
  <img src="docs/screenshots/project-light.png" alt="The page of a project: each Claude Code plugin that applies there, where it is installed, and a switch to turn it on or off">
</picture>

> **Status: early development.** There is no release yet. You can build the desktop app and the
> CLI from source and use them with Claude Code. The CLI and the data format can still change.

## What works today

- **Marketplace catalog.** Add marketplaces from GitHub (a URL or `owner/repo`) or from a local
  folder, or import the marketplaces that Claude Code already knows. The catalog belongs to
  harness-hanger: adding a marketplace does not change Claude Code's files.
- **Browse everything in the catalog.** The Skills, Agents, and Plugins pages show whether Claude
  Code has each one enabled for your user. Each plugin, skill, agent, and command has its own
  page with its metadata, its text, and links to its file and to GitHub.
- **Global setup and projects.** Add a project folder to see which Claude Code plugins apply
  there and where each one comes from. Switch a plugin on or off, install it into a project, or
  uninstall it. On the Global page, switch a plugin on or off for your user.
- **CLI.** The same features from the terminal. Every command has `--json` output for scripts and
  agents.

Claude Code is the first supported harness. Cursor, Hermes, Codex, and pi are planned next.

## Screenshots

### Global setup

The Global page shows what Claude Code loads for your user in every project. A switch turns a
plugin on or off for your user. A project page, as in the screenshot at the top, does the same
for one folder.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/global-dark.png">
  <img src="docs/screenshots/global-light.png" alt="The Global page: the plugins that Claude Code loads for the user, each with a switch">
</picture>

### Marketplaces

Open a marketplace to see its plugins, skills, agents, and commands before you install anything.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/marketplace-preview-dark.png">
  <img src="docs/screenshots/marketplace-preview-light.png" alt="The page of a marketplace: its plugins, with tabs for its skills, agents, and commands">
</picture>

Import the marketplaces and the standalone skills, agents, and commands that Claude Code already
has. Claude Code does not change.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/import-dark.png">
  <img src="docs/screenshots/import-light.png" alt="The import dialog: the marketplaces that Claude Code knows, and which of them are already in the catalog">
</picture>

### Plugins, skills, agents, and commands

Filter the catalog by status: enabled, disabled, or not installed.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/skills-dark.png">
  <img src="docs/screenshots/skills-light.png" alt="The Skills page, filtered to the skills that Claude Code has enabled">
</picture>

Open a plugin to read its README and see what it contains.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/plugin-dark.png">
  <img src="docs/screenshots/plugin-light.png" alt="The page of a plugin: its marketplace, folder, author, and category, and its README">
</picture>

Open a skill, an agent, or a command to read its file, as a preview or as source.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/skill-dark.png">
  <img src="docs/screenshots/skill-light.png" alt="The page of a skill: its plugin, marketplace, and file, and the text of its SKILL.md">
</picture>

## Planned

- **Usage statistics.** A Claude Code hook records which skills, agents, commands, and MCP tools
  run, and whether the agent or you started each one. You see what you use and what is dead
  weight.
- **More of the setup.** MCP servers, permissions, instructions (CLAUDE.md, AGENTS.md, and rule
  files), and switches for skills, agents, and commands outside plugins.
- **Backlog.** Track the components you plan to build: idea, draft, published.
- **More harnesses.** Install from a marketplace into Cursor, Hermes, Codex, and pi, globally or
  for one project.
- **Collections.** Group components into a set, apply it to a project, or share it as a file.
- **Tester.** Run an agent with a prompt in a folder you choose, and check that it uses a skill or
  a subagent.
- **Workflows.** Describe a sequence of steps that use skills, agents, and plugins. Run it with
  one call, or share it as a file.

The full roadmap and the decisions behind it are in [`docs/product.md`](docs/product.md).

## Build from source

You need Rust, Node.js, pnpm, and the Tauri system dependencies. See
[CONTRIBUTING.md](CONTRIBUTING.md#prerequisites) for the versions.

```sh
pnpm install                              # JS dependencies and git hooks
pnpm dev                                  # run the desktop app
cargo install --path crates/hanger-cli    # install the CLI as `harness-hanger`
```

harness-hanger also runs two other tools: `git`, to clone marketplaces from GitHub, and Claude
Code's `claude` command, to switch, install, and uninstall plugins.

## CLI

```sh
harness-hanger import claude-code --all                # import what Claude Code already has
harness-hanger marketplace add anthropics/claude-code  # add one or more marketplaces
harness-hanger marketplace show 4                      # preview a marketplace
harness-hanger project add ~/src/acme-web              # add a project folder
harness-hanger project install 1 hookify@claude-code-plugins
harness-hanger project disable 1 frontend-design@claude-plugins-official
harness-hanger project show 1
```

```text
Project ~/src/acme-web
  on   agent-sdk-dev@claude-code-plugins  installed for the project, set by project settings
  on   code-review@claude-plugins-official  installed for the user, set by user settings
  on   commit-commands@claude-plugins-official  installed for the user, set by user settings
  on   document-skills@anthropic-agent-skills  installed for the user, set by user settings
  on   feature-dev@claude-plugins-official  installed for the user, set by user settings
  off  frontend-design@claude-plugins-official  installed for the user, set by your own project settings
  on   hookify@claude-code-plugins  installed for the project, set by project settings
  off  pr-review-toolkit@claude-plugins-official  installed for the user, set by user settings
  on   superpowers@superpowers-marketplace  installed for the user, set by user settings
Standalone:
  skills: deploy-preview
```

| Command | What it does |
| --- | --- |
| `info` | Show the version and where the Claude Code configuration is read from. |
| `marketplace add <source>...` | Add marketplaces from GitHub or from local folders. |
| `marketplace ls`, `show <id>`, `rm <id>` | List, preview, or remove the marketplaces in the catalog. |
| `marketplace plugins` | List the plugins of every marketplace, and whether Claude Code has each one enabled. |
| `import [harness] [marketplace]... [--standalone] [--all]` | Import from a harness. With nothing named, list what can be imported. |
| `project ls`, `add <folder>`, `rm <id>` | Manage the project folders. Removing a project does not touch the folder. |
| `project show [id]` | Show what applies in a project, or globally when the ID is left out. |
| `project enable`, `disable <id> <plugin@marketplace>` | Switch a plugin on or off in a project. |
| `project install`, `uninstall <id> <plugin@marketplace>` | Install or uninstall a plugin in a project. |

Add `--json` to any command for machine-readable output.

## How harness-hanger treats your setup

- **Claude Code's files are the source of truth.** harness-hanger reads them directly and does not
  keep its own copy of what is installed.
- **Changes go through Claude Code's CLI.** harness-hanger runs `claude plugin …` to switch,
  install, and uninstall plugins. It does not edit Claude Code's files itself.
- **Each change goes to the narrowest place it can.** A global plugin that you switch off in a
  project is switched off only for you, in `.claude/settings.local.json`. A plugin from a GitHub
  marketplace that you install into a project goes into `.claude/settings.json`, for everyone who
  works on the project. A plugin from a local-folder marketplace goes into your own
  `.claude/settings.local.json`, because the folder exists only on your machine.
- **Your data stays on your machine.** There is no telemetry. harness-hanger keeps its own data
  (the catalog, the project list, and clones of GitHub marketplaces) in `harness-hanger` in the
  platform's local data directory, or in `$HARNESS_HANGER_DATA_DIR` when it is set. It reads Claude
  Code's configuration from `~/.claude`, or from `$CLAUDE_CONFIG_DIR` when it is set.

## Repository layout

```
crates/
  hanger-core/        Core library: discovery, harness adapters, storage
  hanger-cli/         CLI (binary: harness-hanger)
apps/
  desktop/            Desktop app: Vue 3 + TypeScript frontend
    src-tauri/        Tauri 2 shell over hanger-core
docs/
  product.md          Product brief, decisions, roadmap
  plans/              Plans for each milestone
  screenshots/        The screenshots in this README
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the setup, the checks, and the commit convention.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
