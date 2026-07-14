# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

`taskboard-rs` is a fast, reliable CLI/TUI task manager with atomic file operations and file locking. The binary is named `tsk`.

**Platform Support:** Linux and macOS (file locking uses the Rust standard library; Windows is expected to work but is untested)

## Development Commands

### Build and Run
```bash
# Build the project
cargo build

# Build release version
cargo build --release

# Run locally (must specify path to avoid installing)
cargo run -- --help
cargo run -- add "Test task"
cargo run -- tui

# Install from source
cargo install --path .
```

### Testing
```bash
# Run all tests
cargo test

# Run specific test
cargo test test_atomic_write

# Run with output shown
cargo test -- --nocapture
```

### Code Quality
```bash
# Format code
cargo fmt

# Lint
cargo clippy

# Check without building
cargo check
```

### Publishing
**Important:** Publishing to crates.io is automated via GitHub Actions. Do NOT manually run `cargo publish`.

To release:
1. Update version in `Cargo.toml`
2. Update `CHANGELOG.md` with release notes
3. Commit changes
4. Create and push a git tag: `git tag v0.1.2 && git push origin v0.1.2`
5. GitHub Actions will automatically:
   - Build cross-platform binaries
   - Create GitHub release with artifacts
   - Publish to crates.io (non-prerelease versions only)

## Architecture

### Module Responsibilities

**src/main.rs** - Entry point
- Routes CLI commands to appropriate handlers
- Resolves tasklist file path (defaults to `.tasklist`, canonicalized to absolute path)
- Creates `Mngr` instance and delegates to CLI or TUI

**src/cli.rs** - Argument parsing (clap)
- Defines `Cli` struct with global flags (`--file`, `--verbose`, `--kanban`)
- Defines `Commands` enum with aliases:
  - `add`/`a`, `update`/`u`, `show`/`ls`/`list`, `delete`/`rm`, `tui`
- `add` accepts a positional description (`tsk add "..."`) or `-d/--description`
- Status values: `not_started`/`ns`, `in_progress`/`ip`, `done`/`d`

**src/task.rs** - Data model
- `Task` struct: id, status, description, date
- `Status` enum: NotStarted (🚀), InProgress (⏳), Done (✅)
- Serialization format: tab-separated `ID\tSTATUS_EMOJI\tDESCRIPTION\tDATE`
- Handles conversion between emoji labels and status values

**src/manager.rs** - Core business logic
- `Mngr` struct: manages all task operations and file persistence
- Atomic write pattern (temp file + rename) with sidecar lockfile for mutual exclusion
- Descriptions sanitized (tabs/newlines → spaces) before writing
- Metadata caching for O(1) task addition
- Methods: `add_task()`, `update_task()`, `delete_task()`, `get_tasks()`, `list_tasks()`
- Display modes: table format and kanban board view

**src/tui.rs** - Interactive mode
- `App` struct: stateful TUI component with mode-based interaction
- `AppMode` enum: Normal, AddingTask, EditingTask, ConfirmDelete
- Event loop handles keyboard input and rerenders UI
- Uses ratatui for terminal UI, crossterm for events

### File Format

Tab-separated format with metadata header:
```
#max_id=3
1	🚀 Not Started	Task description	2025-12-26 10:00
2	⏳ In Progress	Another task	2025-12-26 11:30
```

**Key Details:**
- Metadata line (`#max_id=N`) is always first
- Enables O(1) task addition without full file scan
- Graceful migration from old format (without metadata)
- Separator constant: `SEP: &str = "\t"` in task.rs

### Atomic Write + Locking Pattern

Critical for data integrity. Two cooperating mechanisms in `manager.rs`:

**Mutual exclusion** — `acquire_write_lock()`:
1. Open (create if needed) a sidecar lockfile at `<tasklist_path>.lock`
2. Acquire an exclusive lock via `std::fs::File::lock()`
3. Hold it across the entire read-modify-write (`add_task`, `update_task`, `delete_task`)
4. Released when the handle drops at function exit

The lock lives on a sidecar file (never renamed/deleted) because locking the
tasklist itself is unsound with rename-replace: a waiter can end up holding a
lock on the old, already-replaced inode.

**Crash safety** — `atomic_write()`:
1. Create temp file in same directory (`.tasklist.tmp*`)
2. Write content to temp file via callback, flush
3. Atomically rename temp → original via `tempfile::persist()`

**Guarantees:**
- All-or-nothing writes (no partial updates)
- Concurrent writers are serialized (no lost updates)
- Original preserved if write fails

**Usage pattern:**
```rust
self.atomic_write(|writer| {
    self.write_metadata(writer, max_id)?;
    for task in &tasks {
        task.write_to(writer)?;
    }
    Ok(())
})?;
```

### Metadata System

**Purpose:** Avoid O(n) file scans when adding tasks

**Implementation:**
- `read_metadata()`: Reads first line, extracts `#max_id=N`
- `write_metadata()`: Writes metadata line
- `scan_max_id()`: Fallback for old format files (full scan)
- Migration: Old files get metadata added on first write

**Performance:**
- Add task: O(1) with metadata, O(n) without
- Update/delete: O(n) (requires full rewrite)
- List: O(n) (full file parse)

### TUI State Machine

**AppMode transitions:**
- Normal → AddingTask (press `n`)
- AddingTask → Normal (press Enter to save, Esc to cancel)
- Normal → EditingTask (press `e`, input pre-filled with description)
- EditingTask → Normal (press Enter to save, Esc to cancel)
- Normal → ConfirmDelete (press `d`)
- ConfirmDelete → Normal (press `y` to confirm, `n`/Esc to cancel)

**UI Layout (vertical):**
1. Title + task count (3 lines)
2. Task list with selection highlight (expandable)
3. Help/Input panel (8 lines, changes based on mode)

**State persistence:**
- All changes immediately written via `Mngr` methods
- Tasks reloaded after operations
- File locking ensures consistency with other processes

### Error Handling

- Uses `std::io::Error` throughout
- Custom error messages at failure points
- CLI: errors printed to stderr, exit code 1
- TUI: errors captured and displayed in UI panel

## Common Patterns

### Adding New Status Types
If adding a new status:
1. Add variant to `Status` enum in task.rs
2. Update `Status::as_label()` with emoji
3. Update `Status::from_str()` for parsing
4. Add to clap `ValueEnum` derive
5. Update TUI keyboard handlers in tui.rs
6. Update tests in integration_tests.rs

### Adding New Commands
1. Add variant to `Commands` enum in cli.rs
2. Add handler in `main.rs::main()` match statement
3. Implement business logic in `manager.rs` if needed
4. Add integration test
5. Update README.md usage section

### Modifying File Format
**CRITICAL:** Maintain backward compatibility!
- Old format: plain tab-separated rows (no metadata)
- New format: metadata header + rows
- `read_metadata()` detects format version
- `scan_max_id()` provides fallback for old files
- Migration happens transparently on first write

## Testing Guidelines

**Integration tests** in `tests/integration_tests.rs`:
- Use `tempfile::NamedTempFile` for isolated test files
- Test atomic write behavior
- Verify metadata persistence
- Check migration from old format
- Validate both display modes (table, kanban)

**Test coverage areas:**
- Metadata creation and preservation
- Task CRUD operations
- Edge cases (non-existent IDs, empty lists)
- Data integrity across multiple operations
- Display formatting
- CLI flag combinations

## Key Dependencies

- **clap 4.5** - CLI parsing with derive macros
- **ratatui 0.29** - TUI framework
- **crossterm 0.28** - Terminal handling
- **tempfile 3.15** - Atomic file operations
- **chrono 0.4** - Date/time handling

File locking uses `std::fs::File::lock()` (stable since Rust 1.89) — no external crate.

## CI/CD

### GitHub Actions Workflows

**.github/workflows/ci.yml** - Runs on every push
- Builds project
- Runs tests
- Checks formatting and clippy

**.github/workflows/release.yml** - Runs on version tags
- Uses `cargo-dist` to build cross-platform binaries
- Creates GitHub releases with artifacts
- Publishes to crates.io (non-prerelease versions only)

### Release Process

1. Update version in `Cargo.toml`
2. Update `CHANGELOG.md`
3. Commit changes
4. Create and push tag: `git tag v0.1.x && git push origin v0.1.x`
5. GitHub Actions automatically:
   - Builds binaries for all platforms
   - Creates GitHub release
   - Publishes to crates.io

## Performance Considerations

- **O(1) additions:** Metadata caching avoids full file scans
- **O(n) updates/deletes:** Full file rewrite required for atomicity
- **Terminal width detection:** Kanban view auto-adjusts to terminal size
- **Buffered I/O:** Uses `BufReader`/`BufWriter` for efficiency

## Known Limitations

- **Platform:** Tested on Linux/macOS; Windows expected to work but untested
- **Concurrency:** File locking blocks concurrent writers (they queue, not fail)
- **Scale:** Full file rewrite on updates limits scalability to ~1000s of tasks
- **Format:** Tabs/newlines in descriptions are sanitized to spaces before writing
