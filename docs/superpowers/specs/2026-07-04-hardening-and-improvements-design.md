# tsk hardening and improvements — design

Date: 2026-07-04
Status: approved

## Goal

Fix six correctness/UX bugs found in code review, replace the unmaintained `fs2`
dependency with std file locking, add task editing to the TUI, and clean up repo
hygiene. No file-format changes. Every bug fix gets a regression test.

## 1. Real mutual exclusion via sidecar lockfile

**Problem:** `Mngr::atomic_write()` locks the temp file it just created — a file no
other process ever opens — so concurrent writers are not excluded. Two concurrent
`tsk add` invocations both read the original, write their own temp file, and rename;
the last rename wins and one task is silently lost. The rename still guarantees
no torn/partial files, but the advertised concurrency protection does not exist.

Locking the data file itself is not a fix: after a rename-replace, a process waiting
on the lock ends up holding a lock on the old, unlinked inode while a third process
locks the new one — two writers again.

**Design:** a sidecar lockfile at `<tasklist_path>.lock`:

- Created with `OpenOptions::new().create(true).write(true)` on first use; never
  renamed, truncated, or deleted, so its inode is stable.
- Every read-modify-write operation (`add_task`, `update_task`, `delete_task`)
  acquires an exclusive lock on the lockfile **before reading** the tasklist and
  releases it after `persist()` completes. A `Mngr::with_write_lock(f)` helper wraps
  this; the existing method bodies move inside it.
- Read-only paths (`get_tasks`, `list_tasks`) stay lock-free: the atomic rename
  means readers always see either the old or the new complete file.
- `atomic_write()` keeps the temp-file + `persist()` rename for crash safety; the
  pointless lock on the temp file is removed.

## 2. Replace fs2 with std file locking

`std::fs::File::lock()` / `unlock()` are stable since Rust 1.89; the crate already
requires 1.92. The lockfile from §1 uses std locking; `fs2` is removed from
`Cargo.toml`. Docs (README, CLAUDE.md, Cargo.toml description if needed) drop the
"Linux/macOS only due to fs2" rationale; Windows is described as expected-to-work
but untested rather than officially supported.

## 3. `--file` with a nonexistent path must be honored

**Problem:** `main.rs::get_tasklist_path` canonicalizes the full path;
`fs::canonicalize` fails for paths that do not exist yet, and the fallback silently
substitutes `./.tasklist`. `tsk --file ~/new-list add "x"` writes to the wrong file.

**Design:** canonicalize the **parent** directory and re-append the file name.

- Path exists → canonicalize as today.
- Path missing but parent exists → `canonicalize(parent).join(file_name)`.
- Parent missing too → print a clear error to stderr and exit 1 (no silent fallback).
- Title derivation (parent directory name) is unchanged.
- `get_tasklist_path` returns `Result` so `main` can report the error.

## 4. TUI robustness and editing

**Problems:**
- `App::new()` (which reads the tasklist) runs *after* raw mode + alternate screen
  are enabled; if it errors (e.g. no `.tasklist` in the directory) the terminal is
  left in raw mode.
- `run()` swallows the event-loop error and returns `Ok(())`, so failures exit 0.
- No way to edit a task's description.

**Design:**
- Construct `App` **before** any terminal setup.
- Missing tasklist file is treated as an empty board: `get_tasks()` returns an
  empty `Vec` when the file does not exist (`ErrorKind::NotFound`), so `tsk tui`
  works in a fresh directory. (CLI behavior for the default command is unchanged —
  `main` already prints the getting-started message when the file is absent.
  `tsk show` with no file now shows the friendly empty-list message instead of
  an error, an acceptable and arguably better behavior.)
- Terminal restore always runs after the event loop via the existing sequence;
  the loop's error is returned to `main` (after restore) so the process exits 1.
- New `AppMode::EditingTask { id }`: pressing `e` on a selected task opens the
  input box pre-filled with its description; Enter saves via
  `update_task(id, current_status, Some(input))`; Esc cancels. Help text updated.

## 5. Description handling

- **Positional description:** `Add` takes an optional positional `description`
  plus the existing `-d/--description` flag; exactly one is required (clap
  `required_unless_present` + `conflicts_with`). `tsk add "My first task"` — the
  command the onboarding hint already prints — now works; `-d` stays for
  backward compatibility.
- **Sanitization:** `Mngr` normalizes descriptions in `add_task` and
  `update_task`: `\t`, `\n`, `\r` become single spaces, then trim. Empty-after-trim
  is rejected (existing empty check covers it). Sanitizing in the manager covers
  CLI and TUI alike and keeps the tab-separated format safe.
- **UTF-8-safe truncation:** kanban truncation switches from byte slicing to
  char-boundary truncation (`char_indices`-based), removing the panic on emoji /
  multi-byte characters at the cut point.

## 6. Repo hygiene

New `.gitignore`: `target/`, `.DS_Store`, `.tasklist`, `git-release-cheatsheet.md`
(the cheatsheet remains on disk as a personal note, just untracked). Remove
`.DS_Store` from the working tree.

## Testing

Integration tests (in `tests/integration_tests.rs`) added for:
- concurrent adds from two processes both land (lockfile test, spawn two `tsk add`)
- `--file` pointing at a nonexistent path in an existing directory creates it there
- `--file` with a nonexistent parent directory exits 1 with an error
- positional `tsk add "desc"` and `-d` both work; supplying both fails
- tab/newline in description is stored sanitized (file has no embedded separators)
- kanban view with emoji descriptions longer than the column width does not panic
- `show` / `tui`-backing `get_tasks` on a missing file returns empty, not error

Existing 15 tests must keep passing (the `-d` ones unchanged).

## Out of scope

- File-format changes, new statuses, Windows CI, version bump / CHANGELOG entry
  (release is the maintainer's call).
