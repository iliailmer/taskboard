# AGENTS.md

Guidance for coding agents that work in this repository.

## Project

`taskboard` is a CLI/TUI task manager. The binary is `tsk`. Tasks are kept in a
plain-text `.tasklist` file. Writes are atomic and locked, so several processes
(people, agents, the `nvim-kanban` plugin) can use one file at the same time.

Supported: Linux and macOS. Minimum Rust: 1.90.

## Direction

This is a personal tool first. Keep it small and simple. Other tools also use it
(`nvim-kanban`, agents, a planned MCP server), so the CLI commands, flags, and
`--json` output are a contract. Do not break them without asking.

## Commands

```bash
cargo build
cargo test
cargo fmt
cargo clippy
cargo run -- --help
```

## Modules

- `src/main.rs`: finds the tasklist and sends each command to `Mngr`.
- `src/cli.rs`: clap definitions.
- `src/task.rs`: `Task` (id, status, title, description, date) and `Status`.
- `src/manager.rs`: `Mngr`. Reads, writes, locks, and prints tasks.
- `src/tui.rs`: ratatui interface.

## Rules for changes

- **File format.** `#max_id=N` header, then one task per line with five
  tab-separated fields. Keep old files readable: four-field rows (no title) and
  files without the header must still load. Lines that start with `#` are
  comments.
- **IDs.** Never reuse an ID. `max_id` is the larger of the header and the
  highest task ID.
- **Writes.** Every mutation takes the lock (`acquire_write_lock`), reads the
  file once, and writes with `atomic_write` (temp file, then rename). The lock
  file is `$TMPDIR/tsk-<hash>.lock`, not next to the tasklist.
- **JSON.** With `--json`, stdout is only valid JSON (`[]` when empty). Human
  messages print only without `--json`. `nvim-kanban` depends on this output.
- **Tests.** Integration tests in `tests/integration_tests.rs` run the binary.
  Write few tests. Cover core behavior, not small details.

## Picking work

- Do the task you are given. Stay inside it. If it needs more, say so and ask.
- When asked what is next, suggest work from the GitHub milestones (lowest
  version first) and priority labels (`P0` is highest), and from `.tasklist`.
  Wait for a yes.
- When told to work through a milestone, take its open issues in priority order.
- Mark local tasks with `tsk` (`tsk update <id> --status ip`, then `done`).
  `.tasklist` is not tracked in git.

## Workflow

- Edit files only. The owner reviews, commits, pushes, and opens PRs. Run git
  commands only when asked.
- Work goes on a branch (`feat/...`, `fix/...`) and a PR, never on `main`.
- Update `README.md` when a command or flag changes.
- Write docs, commit messages, and PR text in short, simple English.

## Release

1. Bump `version` in `Cargo.toml` in a PR and merge it.
2. Tag the merged commit on `main` (squash merges change the hash), then push:
   `git tag vX.Y.Z <main-commit> && git push origin vX.Y.Z`.
3. `release.yml` (cargo-dist) builds binaries, creates the GitHub release, and
   publishes to crates.io. Do not run `cargo publish` by hand.
