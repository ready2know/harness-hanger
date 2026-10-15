# Contributing to harness-hanger

Thanks for your interest. This guide covers setup, the daily workflow, and what a pull request needs.

## Prerequisites

- Rust stable (edition 2024) with `rustfmt` and `clippy`
- Node.js 22.18+ or 24.12+, and pnpm 9
- Tauri system dependencies for your OS: see [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)

## Setup

```sh
pnpm install   # installs JS dependencies and the git hooks (lefthook)
cargo build    # builds the core, the CLI, and the desktop app crate
```

## Common commands

| Task                           | Command                                              |
| ------------------------------ | ---------------------------------------------------- |
| Run the desktop app (dev mode) | `pnpm dev`                                           |
| Run the CLI                    | `cargo run -p hanger-cli -- <args>`                  |
| Rust: format, lint, test       | `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` |
| Frontend: lint, types, test    | `pnpm lint`, `pnpm type-check`, `pnpm test`          |
| Frontend: format               | `pnpm format`                                        |
| Production build of the app    | `pnpm build`                                         |

## Git hooks

[lefthook](https://lefthook.dev) runs checks for you:

- **pre-commit:** `rustfmt`, `oxlint`, `eslint`, `prettier` on staged files
- **pre-push:** `clippy`, Rust tests, frontend type check and tests
- **commit-msg:** commitlint

If the hooks are missing, run `pnpm exec lefthook install`.

## Commit messages

We use [Conventional Commits](https://www.conventionalcommits.org/):

```
feat(cli): add `ls --kind` filter
fix(core): ignore unreadable skill directories
docs: describe the inventory data model
```

Common types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `ci`, `build`, `perf`.
Common scopes: `core`, `cli`, `desktop`, `ui`, `ci`.

## Pull requests

1. Branch from `master`.
2. Keep each PR focused on one change. Add or update tests for behavior changes.
3. Make sure CI passes: formatting, lint, types, and tests on macOS, Linux, and Windows.
4. Describe what changed and why. Link the issue if there is one.

## Architecture rules

- All behavior lives in `crates/hanger-core`. The CLI and the desktop app are thin adapters.
- Harness files (for example `~/.claude/settings.json`) are the source of truth. Never drop keys
  that harness-hanger does not understand when writing them.
- Every CLI command supports `--json` with a stable, camelCase shape, so that agents can use it.

See [`docs/product.md`](docs/product.md) for the product scope and roadmap.

## License

By contributing, you agree that your contributions are dual-licensed under MIT and Apache-2.0,
without any additional terms or conditions.
