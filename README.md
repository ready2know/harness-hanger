# harness-hanger

A manager for AI coding-harness components: skills, agents, slash commands, hooks, MCP servers,
plugins, and the marketplaces they come from.

> **Status: early development.** The skeleton is in place, and the first feature (inventory) is
> planned. Nothing is ready to use yet.

## What it will do

- **Global dashboard.** See every component installed for your user, edit it, and keep a backlog
  of components you plan to build.
- **Project scope.** Add any folder as a project to see which components apply there and where
  each one comes from. Add, remove, enable, or disable them for that project only.
- **Usage statistics.** A harness hook records which skills, agents, commands, and MCP tools run,
  and whether the agent or you started them, so you can see what you use and what is dead weight.
  All data stays on your machine.
- **Marketplaces.** Browse marketplaces and install from them into one or more agents, globally
  or for one project.
- **Collections.** Group components into a set, apply it to a project, or share it as a file.
- **Tester.** Run an agent with a prompt in a folder you choose, and check that it uses a skill or
  subagent.
- **Workflows.** Describe a sequence of steps that use skills, agents, and plugins. Run it with
  one call, or share it as a file.
- **Desktop app and CLI.** Both use one Rust core. The CLI has `--json` output for scripts and
  agents.

Claude Code is supported first. The core is built around harness adapters. Cursor, Hermes, Codex,
and pi are planned next.

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
```

## Development

```sh
pnpm install                          # JS dependencies and git hooks
pnpm dev                              # run the desktop app
cargo run -p hanger-cli -- info       # run the CLI
cargo run -p hanger-cli -- marketplace add owner/repo   # add a marketplace to the catalog
```

The CLI and the desktop app keep their data in one directory: `harness-hanger` in the platform's
local data directory, or `$HARNESS_HANGER_DATA_DIR` when it is set.

See [CONTRIBUTING.md](CONTRIBUTING.md) for prerequisites, checks, and the commit convention.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
