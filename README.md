# harness-hanger

A manager for AI coding-harness components: skills, agents, slash commands, hooks, MCP servers,
plugins, and the marketplaces they come from.

> **Status: early development.** The skeleton is in place, and the first feature (inventory) is
> planned. Nothing is ready to use yet.

## What it will do

- **Global dashboard.** See every component installed for your user, edit it, and keep a backlog
  of components you plan to build.
- **Project scope.** Open a project to see which components apply there and where each one comes
  from. Add, remove, enable, or disable them for that project only.
- **Usage statistics.** A harness hook records which skills, agents, commands, and MCP tools run,
  so you can see what you use and what is dead weight. All data stays on your machine.
- **Desktop app and CLI.** Both use one Rust core. The CLI has `--json` output for scripts and
  agents.

Claude Code is supported first. The core is built around harness adapters, so other harnesses can
be added later.

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
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for prerequisites, checks, and the commit convention.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
