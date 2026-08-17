# taskboard

[![CI](https://github.com/iliailmer/taskboard/actions/workflows/ci.yml/badge.svg)](https://github.com/iliailmer/taskboard/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/taskboard.svg)](https://crates.io/crates/taskboard)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![codecov](https://codecov.io/gh/iliailmer/taskboard/branch/main/graph/badge.svg)](https://codecov.io/gh/iliailmer/taskboard)

> > [!WARNING]
> > This project is still in early development and mostly considered a learning experiment.

A fast, reliable command-line task manager written in Rust with atomic file operations and file locking.

## Features

- Fast O(1) task ID allocation with metadata caching
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

# Add a titled task, optionally with a longer description
tsk add "Task title"
tsk a "Task title" --description "Longer body"
tsk add --title "Task title" -d "Longer body"

# Update any combination of status, title, and description
tsk update --id 1 --status in_progress
tsk u --id 1 --title "New title" --description "New body"

# Delete task
tsk delete --id 1
tsk rm --id 1  # short alias

# Kanban view
tsk --kanban
tsk show --kanban

# Machine-readable view
tsk show --json
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
- `n` - Add a title, then an optional multiline description
- `e` - Edit the selected task's title, then its description
- `Enter` - Continue from title or insert a description newline
- `Ctrl+S` - Save while editing a description
- `d` - Delete task
- `r` - Reload tasks
- `q` or Ctrl+C - Quit

### Global Flags

- `-f, --file <PATH>` - Use custom task file
- `-v, --verbose` - Show verbose output
- `-k, --kanban` - Display Kanban view

## File Format

Tasks are stored in `.tasklist` as five tab-separated fields: ID, status,
title, escaped description, and date. Embedded description newlines are stored
as `\n`; existing four-field files migrate on their next mutation.

```
#max_id=3
1\t🚀 Not Started\tWrite documentation\tSee the install section\t2025-12-26 10:00
2\t⏳ In Progress\tImplement feature\tMultiline\nbody survives\t2025-12-26 11:30
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
