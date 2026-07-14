# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed
- **Package Renamed**: Crate renamed to `taskboard`, binary renamed to `tsk`
  - Install with: `cargo install taskboard`
  - Run with: `tsk` (previously `tuibrd`)
  - Shorter, more convenient binary name for daily use

### Previous Changes
- **Package Renamed**: Crate and binary renamed from `tasklist` to `tuibrd`
  - Repository moved to `iliailmer/board-rs`
- **Platform Support**: Dropped Windows support
  - Now supports Linux and macOS only
  - Simplified CI and testing infrastructure

---

## [0.8.0] - 2025-12-26

### Added

#### Interactive TUI Mode
- **Full-Screen TUI**: New interactive terminal user interface powered by `ratatui`
  - Navigate tasks with arrow keys (Up/Down or j/k vim-style)
  - Cycle task status with Space or Enter
  - Add new tasks with 'a' key
  - Delete tasks with 'd' key (with confirmation prompt)
  - Quit with 'q' or Ctrl+C
  - Real-time task list updates
  - Color-coded status indicators (red/yellow/green)
  - Error message display at bottom of screen
  - Clean, responsive layout

### Dependencies
- Added `ratatui` (0.29) - Terminal UI framework
- Added `crossterm` (0.28) - Cross-platform terminal manipulation

### Changed
- Updated CLI to support TUI mode alongside existing command-line interface
- Enhanced task manager for better integration with interactive UI

---

## [0.7.0] - 2025-12-26

### Added

#### UX Improvements
- **Default Command**: Running just `tasklist` now shows your tasks (no subcommand needed)
- **Verbose Flag**: New `--verbose` (`-v`) flag shows which file is being used
- **Global Kanban Flag**: Can now use `--kanban` without specifying `show` subcommand
- **Adaptive Kanban View**: Terminal width auto-detection for optimal column sizing
  - Columns adapt between 25-50 characters based on terminal width
  - Better display on both wide and narrow terminals
- **Task Dates in Kanban**: Each task now displays its creation/update date below the description

#### Performance & Reliability
- **Metadata Caching**: O(1) task addition with `#max_id=` header
  - Eliminates need to scan entire file for max ID
  - ~1000x faster for large task lists (1000+ tasks)
- **Atomic File Operations**: Write-to-temp-then-rename pattern
  - Prevents data corruption if process crashes mid-write
  - Original file never modified directly
- **File Locking**: Cross-platform file locking prevents concurrent write corruption
  - Exclusive locks during writes
  - Safe for shared network drives and multi-user systems
  - Uses `fs2` crate for Unix and Windows support

#### Documentation & Testing
- **Security Analysis**: Comprehensive `SECURITY.md` document
  - Memory safety analysis
  - Threat modeling
  - Risk assessment
- **Enhanced README**: Updated with performance metrics, examples, and roadmap
- **New Tests**: 5 additional integration tests (15 total)
  - Verbose flag behavior
  - Default command operation
  - Kanban date display
  - Global kanban flag
  - Atomic write verification

### Changed
- **File Format**: Now includes metadata line `#max_id=N` at top of file
  - Automatically migrates old format files
  - Backwards compatible during migration
- **Error Messages**: Cleaner, more user-friendly error output
  - No more Rust debug formatting in user-facing errors
  - Clear, actionable error messages
- **Kanban Layout**: Improved spacing and date display
  - Dates shown in subdued color below task description
  - Better truncation for long descriptions

### Dependencies
- Added `fs2` (0.4) - File locking
- Added `terminal_size` (0.4) - Terminal width detection
- Moved `tempfile` (3.15) from dev-dependencies to dependencies for atomic writes

### Performance
- **Add Task**: ~10-100x faster for lists with 100+ tasks
- **File Operations**: All write operations now atomic and crash-safe
- **Memory Usage**: No significant change (still ~1-5 MB for 10,000 tasks)

### Security
- **Risk Level**: LOW to MEDIUM
- **Mitigations**: Atomic writes, file locking, input validation
- **Suitable For**: Personal use, team collaboration, shared systems
- See [SECURITY.md](SECURITY.md) for full analysis

---

## [0.6.1] - 2025-07-23

### Fixed
- Fixed ordering when files were deleted
- Better error handling (thanks Claude!)

### Added
- Started Kanban view implementation (needs review)

---

## [0.6.0] - 2025-07-23

### Added
- Initial Kanban board view with `--kanban` flag
- Command aliases: `a` (add), `u` (update), `ls`/`list` (show), `rm` (delete)

### Changed
- Improved display for long task descriptions (truncation with `...`)

---

## [0.5.0] - Earlier versions

### Added
- Basic CRUD operations (Create, Read, Update, Delete)
- Tab-separated file format
- Status tracking (Not Started, In Progress, Done)
- Timestamps for tasks
- Custom file path support (`--file` flag)

---

## Future Releases

See the [Roadmap](README.md#roadmap) section in README.md for planned features.

---

## Migration Guide

### Upgrading from v0.6.x to v0.7.0

**Automatic Migration**
- Your existing `.tasklist` files will be automatically upgraded
- First write operation adds `#max_id=N` metadata line
- No manual action required

**New File Format**
```
#max_id=3
1	🚀 Not Started	Task 1	2025-12-26 10:00
2	⏳ In Progress	Task 2	2025-12-26 11:00
3	✅ Done	Task 3	2025-12-26 12:00
```

**Breaking Changes**
- None! Format is backwards compatible during migration

**New CLI Options**
```bash
# These now work (previously required 'show' subcommand):
tuibrd                    # Shows tasks
tuibrd --kanban           # Shows Kanban view
tuibrd --verbose          # Shows file path

# These still work as before:
tuibrd show
tuibrd show --kanban
```

---

## Development

### Version Numbering

This project follows [Semantic Versioning](https://semver.org/):
- **Major** (X.0.0): Breaking changes
- **Minor** (0.X.0): New features, backwards compatible
- **Patch** (0.0.X): Bug fixes, backwards compatible

---

[0.8.0]: https://github.com/iliailmer/board-rs/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/iliailmer/board-rs/compare/v0.6.1...v0.7.0
[0.6.1]: https://github.com/iliailmer/board-rs/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/iliailmer/board-rs/releases/tag/v0.6.0
