# tsk Hardening and Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix six correctness/UX bugs (ineffective locking, `--file` path fallback, TUI terminal breakage, UTF-8 panic, unusable onboarding hint, tab corruption), replace fs2 with std locking, add TUI task editing, and clean up repo hygiene.

**Architecture:** All persistence logic stays in `Mngr` (src/manager.rs); mutual exclusion moves to a sidecar lockfile (`<path>.lock`) locked via `std::fs::File::lock()` around every read-modify-write. CLI surface changes are confined to src/cli.rs + src/main.rs; TUI changes to src/tui.rs.

**Tech Stack:** Rust 2024 edition (rust-version 1.92), clap 4.5, ratatui 0.29, tempfile. fs2 is removed.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-07-04-hardening-and-improvements-design.md`
- **No git commits — the user commits manually.** Skip all commit steps; instead run the full test suite after each task.
- No file-format changes; the existing 15 integration tests must keep passing unmodified (except where a task explicitly says otherwise).
- `-d/--description` on `add` must keep working (backward compat).
- Test runner: `cargo test` from repo root. Verify with `cargo clippy --all-targets` and `cargo fmt` at the end.

---

### Task 1: Sidecar lockfile mutual exclusion; remove fs2

**Files:**
- Modify: `src/manager.rs` (imports, new `acquire_write_lock`, `add_task`, `update_task`, `delete_task`, `atomic_write`)
- Modify: `Cargo.toml` (remove fs2)
- Test: `tests/integration_tests.rs`

**Interfaces:**
- Produces: `Mngr::acquire_write_lock(&self) -> Result<File, Error>` (private); lockfile path is `format!("{}.lock", self.tasklist_path)`. Later tasks don't call it directly but Task 6 gitignores `.tasklist.lock`.

- [ ] **Step 1: Write the failing test** (append to `tests/integration_tests.rs`):

```rust
#[test]
fn test_concurrent_adds_do_not_lose_tasks() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let binary_path = format!("{}/target/debug/tsk", manifest_dir);

    let children: Vec<_> = (1..=8)
        .map(|i| {
            std::process::Command::new(&binary_path)
                .args([
                    "--file",
                    ".tasklist",
                    "add",
                    "--description",
                    &format!("Concurrent {}", i),
                ])
                .current_dir(&temp_path)
                .spawn()
                .expect("Failed to spawn command")
        })
        .collect();
    for mut child in children {
        assert!(child.wait().unwrap().success());
    }

    let content = fs::read_to_string(temp_path.join(".tasklist")).unwrap();
    let task_count = content.lines().filter(|l| !l.starts_with('#')).count();
    assert_eq!(task_count, 8, "Tasks lost under concurrency:\n{}", content);
    assert!(content.starts_with("#max_id=8"), "Content: {}", content);
}
```

- [ ] **Step 2: Run it, expect failure** (lost updates): `cargo test test_concurrent_adds -- --nocapture` → FAIL (task_count < 8 or wrong max_id; may occasionally pass — rerun to observe the race).

- [ ] **Step 3: Implement.** In `src/manager.rs`:

Remove `use fs2::FileExt;`. Add the helper:

```rust
fn acquire_write_lock(&self) -> Result<File, Error> {
    let lock_path = format!("{}.lock", self.tasklist_path);
    let lock_file = OpenOptions::new()
        .create(true)
        .write(true)
        .open(&lock_path)
        .map_err(|e| {
            Error::new(e.kind(), format!("Failed to open lock file {}: {}", lock_path, e))
        })?;
    lock_file
        .lock()
        .map_err(|e| Error::other(format!("Failed to lock {}: {}", lock_path, e)))?;
    Ok(lock_file)
}
```

Add as the first line of `add_task`, `update_task`, and `delete_task` (before any read):

```rust
let _lock = self.acquire_write_lock()?;
```

(The lock is released when `_lock` is dropped at function exit, after `persist()`.)

In `atomic_write`, delete the temp-file locking (the `file.lock_exclusive()...` block and `temp_file.as_file().unlock().ok();`) — the rename provides crash safety; mutual exclusion now comes from the lockfile.

In `Cargo.toml`, delete the `fs2 = "0.4"` line.

- [ ] **Step 4: Run:** `cargo test` → all pass, including the new concurrency test. `cargo clippy --all-targets` → clean.

---

### Task 2: Honor `--file` with nonexistent paths

**Files:**
- Modify: `src/main.rs` (`get_tasklist_path`, `main`)
- Test: `tests/integration_tests.rs`

**Interfaces:**
- Produces: `get_tasklist_path(custom: Option<String>) -> Result<(String, String), String>` (was infallible tuple).

- [ ] **Step 1: Write the failing tests:**

```rust
#[test]
fn test_file_flag_with_new_path_is_honored() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();
    let target = temp_path.join("newlist");

    let output = run_command(
        &temp_path,
        &["--file", target.to_str().unwrap(), "add", "--description", "X"],
    );
    assert!(output.status.success(), "{:?}", String::from_utf8_lossy(&output.stderr));
    assert!(target.exists(), "Task list not created at --file path");
    assert!(
        !temp_path.join(".tasklist").exists(),
        "Silently fell back to ./.tasklist"
    );
}

#[test]
fn test_file_flag_with_missing_parent_dir_errors() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();
    let target = temp_path.join("no_such_dir").join("newlist");

    let output = run_command(
        &temp_path,
        &["--file", target.to_str().unwrap(), "add", "--description", "X"],
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("does not exist"), "stderr: {}", stderr);
}
```

- [ ] **Step 2: Run, expect FAIL** (first: task lands in `./.tasklist`; second: exits 0).

- [ ] **Step 3: Implement** in `src/main.rs`:

```rust
fn get_tasklist_path(custom: Option<String>) -> Result<(String, String), String> {
    let raw_path = custom.unwrap_or_else(|| ".tasklist".to_string());

    let path_buf = match fs::canonicalize(&raw_path) {
        Ok(p) => p,
        Err(_) => {
            // File doesn't exist yet: canonicalize the parent and re-append the name
            let raw = PathBuf::from(&raw_path);
            let file_name = raw
                .file_name()
                .ok_or_else(|| format!("Invalid tasklist path: {}", raw_path))?
                .to_os_string();
            let parent = match raw.parent() {
                Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
                _ => PathBuf::from("."),
            };
            let canonical_parent = fs::canonicalize(&parent).map_err(|_| {
                format!(
                    "Cannot use tasklist path {}: directory {} does not exist",
                    raw_path,
                    parent.display()
                )
            })?;
            canonical_parent.join(file_name)
        },
    };
    let path_string = path_buf.to_string_lossy().to_string();

    let title = match path_buf.parent() {
        Some(parent) => parent
            .file_name()
            .map(|os_str| os_str.to_string_lossy().to_string())
            .unwrap_or_else(|| parent.to_string_lossy().to_string()),
        None => ".".to_string(),
    };

    Ok((path_string, title))
}
```

And in `main`:

```rust
let (tasklist_path, project_title) = match get_tasklist_path(args.file) {
    Ok(v) => v,
    Err(e) => {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    },
};
```

- [ ] **Step 4: Run:** `cargo test` → all pass (existing tests use relative `.tasklist` with `current_dir` set, which resolves identically through the new parent-canonicalize branch).

---

### Task 3: Positional description + tab/newline sanitization

**Files:**
- Modify: `src/cli.rs` (`Commands::Add`), `src/main.rs` (Add arm), `src/manager.rs` (`add_task`, `update_task`)
- Test: `tests/integration_tests.rs`

**Interfaces:**
- Produces: `Commands::Add { text: Option<String>, description: Option<String> }`; `Mngr::sanitize_description(description: &str) -> String` (private associated fn).

- [ ] **Step 1: Write the failing tests:**

```rust
#[test]
fn test_add_with_positional_description() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    let output = run_command(&temp_path, &["--file", ".tasklist", "add", "Positional task"]);
    assert!(output.status.success(), "{:?}", String::from_utf8_lossy(&output.stderr));

    let content = fs::read_to_string(temp_path.join(".tasklist")).unwrap();
    assert!(content.contains("Positional task"));

    // Both positional and -d is an error
    let output = run_command(
        &temp_path,
        &["--file", ".tasklist", "add", "pos", "--description", "flag"],
    );
    assert!(!output.status.success());

    // Neither is an error
    let output = run_command(&temp_path, &["--file", ".tasklist", "add"]);
    assert!(!output.status.success());
}

#[test]
fn test_description_tabs_and_newlines_sanitized() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    let output = run_command(
        &temp_path,
        &["--file", ".tasklist", "add", "--description", "part1\tpart2\npart3"],
    );
    assert!(output.status.success());

    let content = fs::read_to_string(temp_path.join(".tasklist")).unwrap();
    let task_line = content.lines().find(|l| !l.starts_with('#')).unwrap();
    assert_eq!(
        task_line.split('\t').count(),
        4,
        "Embedded separators corrupted the row: {:?}",
        task_line
    );
    assert!(task_line.contains("part1 part2 part3"));
}
```

- [ ] **Step 2: Run, expect FAIL** (positional add is a clap error; tab row has 5+ fields).

- [ ] **Step 3: Implement.** In `src/cli.rs`:

```rust
    #[command(about = "Add a new task")]
    #[clap(visible_alias = "a")]
    Add {
        #[arg(
            value_name = "DESCRIPTION",
            help = "Task description",
            required_unless_present = "description",
            conflicts_with = "description"
        )]
        text: Option<String>,
        #[arg(short, long, help = "Task description (flag form)")]
        description: Option<String>,
    },
```

In `src/main.rs`:

```rust
Some(Commands::Add { text, description }) => {
    let description = text.or(description).expect("clap enforces one description");
    mngr.add_task(description)
},
```

In `src/manager.rs`, add:

```rust
fn sanitize_description(description: &str) -> String {
    description.replace(['\t', '\n', '\r'], " ").trim().to_string()
}
```

In `add_task`, first line becomes `let description = Self::sanitize_description(&description);` (before the existing empty check, which now also rejects whitespace-only input). In `update_task`, before the loop: `let description = description.map(|d| Self::sanitize_description(&d));`.

- [ ] **Step 4: Run:** `cargo test` → all pass. Update `README.md` usage examples to show `tsk add "My first task"` as primary form (keep `-d` documented).

---

### Task 4: UTF-8-safe kanban truncation

**Files:**
- Modify: `src/manager.rs` (`display_kanban`)
- Test: `tests/integration_tests.rs`

- [ ] **Step 1: Write the failing test:**

```rust
#[test]
fn test_kanban_with_long_emoji_description_does_not_panic() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    let long_emoji = "🎉".repeat(60);
    let output = run_command(&temp_path, &["--file", ".tasklist", "add", "--description", &long_emoji]);
    assert!(output.status.success());

    let output = run_command(&temp_path, &["--file", ".tasklist", "show", "--kanban"]);
    assert!(
        output.status.success(),
        "kanban panicked: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
```

- [ ] **Step 2: Run, expect FAIL** (byte-index slice panics mid-codepoint; process aborts).

- [ ] **Step 3: Implement.** In `display_kanban`, replace the truncation block:

```rust
let truncated = if task.description.chars().count() > desc_max_len {
    let cut: String = task
        .description
        .chars()
        .take(desc_max_len.saturating_sub(3))
        .collect();
    format!("{}...", cut)
} else {
    task.description.clone()
};
```

- [ ] **Step 4: Run:** `cargo test` → all pass.

---

### Task 5: TUI robustness (terminal restore, exit code, empty board) + edit mode

**Files:**
- Modify: `src/manager.rs` (`get_tasks`), `src/tui.rs` (`run`, `AppMode`, `run_app`, `ui`, new `App::save_edited_task`)
- Test: `tests/integration_tests.rs` (missing-file behavior; TUI interaction verified manually)

**Interfaces:**
- Produces: `AppMode::EditingTask { id: i32, status: Status }`; `App::save_edited_task(&mut self, id: i32, status: Status) -> io::Result<()>`.

- [ ] **Step 1: Write the failing test** (covers the `get_tasks` change that makes `tsk tui`/`tsk show` safe with no file):

```rust
#[test]
fn test_show_with_missing_file_is_friendly() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    let output = run_command(&temp_path, &["--file", "nothere.tasklist", "show"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No tasks found"));
}
```

- [ ] **Step 2: Run, expect FAIL** (currently exits 1 with "Could not read task list").

- [ ] **Step 3: Implement `get_tasks` missing-file case** in `src/manager.rs`:

```rust
let tasklist = match OpenOptions::new().read(true).open(&self.tasklist_path) {
    Ok(f) => f,
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
    Err(e) => {
        return Err(Error::new(
            e.kind(),
            format!("Could not read task list {}: {}", self.tasklist_path, e),
        ));
    },
};
```

- [ ] **Step 4: Fix `tui::run` ordering and error propagation:**

```rust
pub fn run(manager: Mngr) -> io::Result<()> {
    // Create app before touching the terminal so a failure can't leave raw mode on
    let app = App::new(manager)?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal, app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    res
}
```

(`main` already turns the `Err` into exit code 1.)

- [ ] **Step 5: Add edit mode.** In `src/tui.rs`:

`AppMode` gains a variant:

```rust
#[derive(PartialEq)]
enum AppMode {
    Normal,
    AddingTask,
    EditingTask { id: i32, status: Status },
    ConfirmDelete,
}
```

New method on `App` (next to `add_task`):

```rust
fn save_edited_task(&mut self, id: i32, status: Status) -> io::Result<()> {
    if !self.input.trim().is_empty() {
        self.manager
            .update_task(id, status, Some(self.input.clone()))
            .map_err(io::Error::other)?;
    }
    self.input.clear();
    self.mode = AppMode::Normal;
    self.reload_tasks()
}
```

In `run_app`, Normal mode gains (next to the `'d'` handler):

```rust
KeyCode::Char('e') => {
    if let Some(task) = app.get_selected_task() {
        let id = task.id;
        let status = task.status;
        app.input = task.description.clone();
        app.mode = AppMode::EditingTask { id, status };
        app.error_message = None;
    }
},
```

New match arm (mirrors `AddingTask`):

```rust
AppMode::EditingTask { id, status } => match key.code {
    KeyCode::Enter => {
        if let Err(e) = app.save_edited_task(id, status) {
            app.error_message = Some(format!("Error: {}", e));
            app.mode = AppMode::Normal;
        }
    },
    KeyCode::Esc => {
        app.mode = AppMode::Normal;
        app.input.clear();
        app.error_message = None;
    },
    KeyCode::Char(c) => {
        app.input.push(c);
    },
    KeyCode::Backspace => {
        app.input.pop();
    },
    _ => {},
},
```

In `ui()`, the input panel handles both modes (replace the `AppMode::AddingTask` arm):

```rust
AppMode::AddingTask | AppMode::EditingTask { .. } => {
    let title = if matches!(app.mode, AppMode::AddingTask) {
        "New Task Description"
    } else {
        "Edit Task Description"
    };
    let input = Paragraph::new(app.input.as_str())
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::ALL).title(title));
    f.render_widget(input, chunks[2]);
},
```

Help line: change `"n (new) d (delete) r (reload)"` to `"n (new) e (edit) d (delete) r (reload)"`.

- [ ] **Step 6: Run:** `cargo test` → all pass; `cargo clippy --all-targets` → clean. Manually verify: `cargo run -- tui` in an empty temp dir opens an empty board (terminal restored cleanly on `q`); `e` edits a task.

---

### Task 6: Repo hygiene + docs

**Files:**
- Create: `.gitignore`
- Modify: `README.md`, `CLAUDE.md` (platform/locking claims, add usage)
- Delete: `.DS_Store` (untracked junk)

- [ ] **Step 1: Create `.gitignore`:**

```gitignore
/target
Cargo.lock.orig
.DS_Store
.tasklist
.tasklist.lock
git-release-cheatsheet.md
```

(Keep `Cargo.lock` tracked — this is a binary crate.)

- [ ] **Step 2:** `rm -f .DS_Store`.

- [ ] **Step 3: Docs.** In `README.md` and `CLAUDE.md`:
- Replace "Linux and macOS only (due to fs2 file locking limitations)" with: file locking now uses the Rust standard library; Linux and macOS are supported, Windows is expected to work but is untested.
- Update the atomic-write/locking description: exclusive lock on a sidecar `<file>.lock` held across the whole read-modify-write; temp-file + atomic rename for crash safety.
- Remove fs2 from the dependency list; note `add` accepts a positional description.

- [ ] **Step 4: Final verification:** `cargo fmt && cargo clippy --all-targets && cargo test` → clean, all tests pass (15 old + 7 new = 22).
