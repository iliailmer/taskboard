# taskboard

[![CI](https://github.com/iliailmer/board-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/iliailmer/board-rs/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/taskboard.svg)](https://crates.io/crates/taskboard)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![codecov](https://codecov.io/gh/iliailmer/board-rs/branch/main/graph/badge.svg)](https://codecov.io/gh/iliailmer/board-rs)

> > [!WARNING]
> > This project is still in early development and mostly considered a learning experiment.

A fast, reliable command-line task manager written in Rust with atomic file operations and file locking.

## Features

- Fast O(1) task addition with metadata caching
- Atomic file operations prevent data corruption
- File locking prevents race conditions
- Kanban board view with terminal width auto-detection
- Interactive TUI mode
- Simple CLI with command aliases

## Installation

**Platform Support:** Linux and macOS (Windows is expected to work but is untested)

### From crates.io

```bash
cargo install taskboard
```

### From Source

```bash
cargo install --path .
```

## Usage

### CLI Mode

```bash
# View tasks (default)
tsk

# Add a task
tsk add "Task description"
tsk a "Task description"       # short alias
tsk add -d "Task description"  # flag form also works

# Update task status
tsk update --id 1 --status in_progress
tsk u --id 1 --status ip  # with aliases

# Delete task
tsk delete --id 1
tsk rm --id 1  # short alias

# Kanban view
tsk --kanban
tsk show --kanban
```

### Status Aliases

Use shorter status values:

- `ip` = in_progress
- `d` = done
- `ns` = not_started

### Interactive TUI

Launch the interactive text-based interface:

```bash
tsk tui
```

**TUI Controls:**

- `↑/k` and `↓/j` - Navigate tasks
- `1/2/3` - Change status (Not Started/In Progress/Done)
- `n` - Add new task
- `e` - Edit selected task's description
- `d` - Delete task
- `r` - Reload tasks
- `q` or Ctrl+C - Quit

### Global Flags

- `-f, --file <PATH>` - Use custom task file
- `-v, --verbose` - Show verbose output
- `-k, --kanban` - Display Kanban view

## File Format

Tasks are stored in `.tasklist` using tab-separated format:

```
#max_id=3
1 🚀 Not Started Write documentation 2025-12-26 10:00
2 ⏳ In Progress Implement feature 2025-12-26 11:30
3 ✅ Done Fix bug 2025-12-26 09:15
```

## Development

```bash
# Build
cargo build

# Test
cargo test

# Format and lint
cargo fmt
cargo clippy
```

## License

MIT
