# Security Policy

## Reporting Security Vulnerabilities

If you discover a security vulnerability in taskboard, please report it by:

1. **Email**: Contact the maintainer directly (see `Cargo.toml` for contact info)
2. **GitHub**: Open a security advisory at <https://github.com/iliailmer/board-rs/security/advisories>
3. **Do not** open public issues for security vulnerabilities

We aim to respond within 48 hours and provide a fix within 7 days for critical issues.

---

## Security Overview

**Last Updated**: January 2026
**Security Rating**: 8.5/10 - Very Good
**Risk Level**: LOW for personal/team use

taskboard is a secure, well-designed CLI/TUI task manager built with Rust. The codebase leverages Rust's memory safety guarantees and implements proper file locking and atomic operations. No critical vulnerabilities were found during comprehensive security analysis.

---

## Current Security Status

### ✅ Strengths

| Area                 | Status       | Details                                                    |
| -------------------- | ------------ | ---------------------------------------------------------- |
| **Memory Safety**    | ✅ Perfect   | No unsafe code, no memory leaks, full RAII compliance      |
| **File Operations**  | ✅ Excellent | Atomic writes via tempfile, proper cleanup on failure      |
| **Concurrency**      | ✅ Good      | File locking prevents corruption from concurrent processes |
| **Dependencies**     | ✅ Good      | All deps actively maintained, no known CVEs                |
| **Error Handling**   | ✅ Good      | Comprehensive Result types, proper error propagation       |
| **Input Validation** | ⚠️ Fair      | Basic validation present, some edge cases need work        |

### ⚠️ Known Issues

| Severity | Issue                                 | Impact                         | Status    |
| -------- | ------------------------------------- | ------------------------------ | --------- |
| Medium   | Tab/newline injection in descriptions | File parsing corruption        | Open      |
| Medium   | Path traversal via `--file` flag      | Access to arbitrary files      | By Design |
| Low      | TOCTOU race in `add_task()`           | Possible silent task overwrite | Open      |
| Low      | Integer overflow at 2B tasks          | ID wraps to negative           | Open      |
| Very Low | Stale temp files if crash             | Disk clutter                   | Open      |

---

## Detailed Security Analysis

### 1. Memory Safety ✅

**Status: EXCELLENT - No Issues Found**

- **Zero unsafe blocks** in the entire codebase
- All memory managed by Rust's ownership system
- No manual allocations, raw pointers, or `mem::forget()`
- RAII guarantees all resources are freed (files, locks, buffers)
- No reference counting cycles (no `Rc`/`Arc` usage)

**Verification:**

```bash
# Confirm no unsafe code
rg "unsafe" src/
# (Returns no results)
```

### 2. File Operation Security ✅

**Status: EXCELLENT - Properly Implemented**

#### Atomic Writes (src/manager.rs:401-435)

The application uses a secure write-to-temp-then-rename pattern:

```rust
fn atomic_write<F>(&self, write_fn: F) -> Result<(), Error> {
    // 1. Create temp file in same directory (not /tmp)
    let temp_file = tempfile::Builder::new()
        .prefix(".tasklist.tmp")
        .tempfile_in(parent)?;

    // 2. Acquire exclusive lock
    file.lock_exclusive()?;

    // 3. Write to temp file
    write_fn(&mut writer)?;
    writer.flush()?;

    // 4. Release lock
    file.unlock()?;

    // 5. Atomically replace original
    temp_file.persist(&self.tasklist_path)?;
}
```

**Benefits:**

- All-or-nothing writes (no partial updates)
- Original file preserved if write fails
- Temp files inherit directory permissions
- Automatic cleanup via `tempfile` crate

**Minor Issue:**

```rust
temp_file.as_file().unlock().ok();  // Line 427
```

`.ok()` silently ignores unlock errors. Consider:

```rust
temp_file.as_file().unlock()
    .map_err(|e| warn!("Failed to unlock temp file: {}", e))
    .ok();
```

#### File Locking (src/manager.rs:416-427)

Uses `fs2::FileExt::lock_exclusive()` for cross-platform locking:

- ✅ Prevents concurrent write corruption
- ✅ Works across processes (not just threads)
- ✅ OS releases locks if process crashes
- ⚠️ Advisory locks on Linux (not enforced by kernel)

### 3. Input Validation Issues ⚠️

#### Issue #1: Tab Character Injection (Medium Severity)

**Location:** src/task.rs:80-88, src/manager.rs:24-66

The file format uses tabs as separators, but descriptions aren't validated:

```rust
// Current - VULNERABLE
pub fn to_file_string(&self) -> String {
    format!("{}\t{}\t{}\t{}",
        self.id,
        self.status.as_label(),
        self.description,  // ← Can contain tabs!
        self.date)
}
```

**Exploit:**

```bash
$ tsk add "Task with$(printf '\t')injected$(printf '\t')data"
$ cat .tasklist
#max_id=1
1 🚀 Not Started Task with injected data 2026-01-07
# Parser sees extra fields and breaks!
```

**Fix:**

```rust
pub fn add_task(&self, description: &str) -> Result<(), Error> {
    if description.is_empty() {
        return Err(Error::new(ErrorKind::InvalidInput,
            "Task description cannot be empty"));
    }

    // Add this check:
    if description.contains(&['\t', '\n', '\r']) {
        return Err(Error::new(ErrorKind::InvalidInput,
            "Description cannot contain tab or newline characters"));
    }

    // Or sanitize:
    let sanitized = description
        .replace('\t', " ")
        .replace('\n', " ")
        .replace('\r', "");

    // ... rest of logic
}
```

**Alternative:** Use a proper serialization format (JSON, TOML) instead of TSV.

#### Issue #2: Newline Injection (Medium Severity)

Same issue as tabs - newlines break line-based parsing.

**Fix:** Same validation as above.

#### Issue #3: Path Traversal (Medium Severity)

**Location:** src/main.rs:13-27

Users can specify arbitrary file paths via `--file`:

```bash
$ tsk --file /etc/passwd add "malicious task"
# Attempts to write to /etc/passwd (fails due to permissions, but scary)

$ tsk --file ../../../sensitive.txt list
# Can read any file user has access to
```

**Risk Assessment:**

- **Impact:** User can access any file they already have permissions for
- **Likelihood:** Low (user must explicitly provide path)
- **Severity:** Medium (violates principle of least surprise)

**Mitigation Options:**

1. **Document as intended behavior** (easiest):

   ```markdown
   ## Security Note

   The `--file` flag accepts arbitrary paths. Users can access
   any file they have permissions for. This is by design for flexibility.
   ```

2. **Restrict to safe directories**:

   ```rust
   fn validate_path(path: &Path) -> Result<(), Error> {
       let canonical = path.canonicalize()?;
       let home = dirs::home_dir().ok_or(...)?;

       if !canonical.starts_with(&home) {
           return Err(Error::new(ErrorKind::PermissionDenied,
               "Tasklist must be within home directory"));
       }
       Ok(())
   }
   ```

3. **Add confirmation prompt** for paths outside current directory.

### 4. Concurrency & Race Conditions ⚠️

#### Issue: TOCTOU in add_task() (Low Severity)

**Location:** src/manager.rs:24-66

Time-of-check-time-of-use race condition:

```rust
pub fn add_task(&self, description: String) -> Result<(), Error> {
    // READ #1: Get max_id
    let (mut max_id, has_metadata) = self.read_metadata()?;

    // READ #2: Load existing tasks
    if let Ok(file) = OpenOptions::new().read(true).open(&self.tasklist_path) {
        // ... read tasks into Vec ...
    }

    // WINDOW: Another process could modify file here!

    // WRITE: Atomic write with lock
    self.atomic_write(|writer| { ... })?;
}
```

**Race Scenario:**

```
Process A: read max_id=5
Process B: read max_id=5
Process A: write task ID=6
Process B: write task ID=6  ← Overwrites A's task!
```

**Impact:**

- File isn't corrupted (atomic write works)
- But one task silently overwrites the other
- Last writer wins

**Fix:** Acquire lock before reading:

```rust
pub fn add_task(&self, description: String) -> Result<(), Error> {
    self.atomic_write(|writer| {
        // Lock acquired here, before reading!
        let (max_id, _) = self.read_metadata_locked()?;
        let new_id = max_id + 1;

        // Read existing tasks while locked
        let existing_tasks = self.read_tasks_locked()?;

        // Write everything
        self.write_metadata(writer, new_id)?;
        for task in existing_tasks {
            writeln!(writer, "{}", task)?;
        }
        // ...
        Ok(())
    })
}
```

#### Verified: No Data Races

File locking prevents:

- Concurrent write corruption
- Torn reads/writes
- Lost updates (last writer wins, not random corruption)

### 5. Integer Overflow (Very Low Severity)

**Location:** src/manager.rs:50

```rust
let new_id = max_id + 1;  // Panics at i32::MAX in debug, wraps in release
```

**Risk:** After 2,147,483,647 tasks, ID wraps to -2,147,483,648.

**Fix:**

```rust
let new_id = max_id.checked_add(1)
    .ok_or_else(|| Error::new(ErrorKind::Other,
        "Task ID limit reached (max 2 billion tasks)"))?;
```

### 6. Dependency Security ✅

**Status: GOOD - All Dependencies Secure**

Current dependencies (30 total, including transitive):

| Crate     | Version | Status    | Notes                       |
| --------- | ------- | --------- | --------------------------- |
| clap      | 4.5.53  | ✅ Secure | Official Rust arg parser    |
| ratatui   | 0.29    | ✅ Secure | Active TUI framework        |
| crossterm | 0.28    | ✅ Secure | Terminal handling           |
| tempfile  | 3.15    | ✅ Secure | Security-focused temp files |
| fs2       | 0.4     | ✅ Secure | File locking wrapper        |
| chrono    | 0.4.42  | ✅ Secure | Widely audited date/time    |
| colored   | 3.0.0   | ✅ Secure | Terminal colors             |
| tabled    | 0.18.0  | ✅ Secure | Table formatting            |

**No Known CVEs** in dependency tree (as of Jan 2026)

**Recommendations:**

```bash
# Install audit tool
cargo install cargo-audit

# Check for vulnerabilities
cargo audit

# Update dependencies
cargo update
cargo audit
```

---

## Code Quality Assessment

### Clippy Warnings: 60 Found (All Fixable)

**Categories:**

- 35 × `uninlined_format_args` - Use `format!("{e}")` instead of `format!("{}", e)`
- 8 × `needless_pass_by_value` - Use `&str` instead of `String` parameters
- 6 × `single_char_pattern` - Use `'#'` instead of `"#"` for `starts_with()`
- 5 × `unnested_or_patterns` - Use `'y' | 'Y'` instead of separate patterns
- 6 × Other minor issues

**Fix all:**

```bash
cargo clippy --fix --allow-dirty --allow-staged
cargo fmt
```

### Missing Documentation

No public API documentation (`///` doc comments):

````rust
// Add this:
/// Adds a new task to the task list.
///
/// # Arguments
/// * `description` - Task description (cannot contain tabs/newlines)
///
/// # Errors
/// Returns `Error` if description is empty or contains invalid characters.
///
/// # Example
/// ```
/// let manager = Mngr::new("tasks.txt".into(), None);
/// manager.add_task("Write documentation")?;
/// ```
pub fn add_task(&self, description: &str) -> Result<(), Error>
````

---

## Recommendations

### 🔴 HIGH Priority (Security)

1. **Sanitize Input for Tab/Newline Characters**
   - Location: `src/manager.rs::add_task()`, `src/manager.rs::update_task()`
   - Impact: Prevents file corruption
   - Fix: Add validation or sanitization (see Issue #1 above)

2. **Fix TOCTOU Race in add_task()**
   - Location: `src/manager.rs:24-66`
   - Impact: Prevents silent task overwrites
   - Fix: Move `read_metadata()` inside `atomic_write()` callback

3. **Document Path Security Implications**
   - Location: README.md, `--help` output
   - Impact: Users understand `--file` flag can access any path
   - Fix: Add security notice to documentation

### 🟡 MEDIUM Priority (Code Quality)

1. **Fix All Clippy Warnings**
   - Command: `cargo clippy --fix --allow-dirty --allow-staged`
   - Impact: More idiomatic Rust, easier to maintain
   - Effort: ~15 minutes

2. **Add Public API Documentation**
   - Impact: Better developer experience
   - Effort: ~1 hour for all public functions

3. **Handle Integer Overflow**
   - Location: `src/manager.rs:50`
   - Impact: Graceful error vs panic at 2B tasks
   - Fix: Use `checked_add()`

### 🟢 LOW Priority (Nice-to-Have)

1. **Add Edge Case Tests**
   - Concurrent modifications
   - Tab/newline in descriptions
   - Large files (10K+ tasks)
   - Unicode edge cases

2. **Consider Better Error Types**
   - Replace `std::io::Error` with `anyhow::Error`
   - Add structured error types with context

3. **Add Logging**
   - Use `log` crate with `env_logger`
   - Debug concurrent operations
   - Audit trail for security

4. **Stale Temp File Cleanup**
    - Remove `.tasklist.tmp*` files on startup
    - Low impact (files are small)

---

## Testing Security

### Recommended Security Tests

```bash
# 1. Test concurrent writes (requires multiple terminals)
# Terminal 1:
for i in {1..100}; do cargo run -- add "Task A-$i"; done

# Terminal 2 (simultaneously):
for i in {1..100}; do cargo run -- add "Task B-$i"; done

# Verify: Should have 200 tasks, no corruption
cargo run -- list | wc -l

# 2. Test tab injection
cargo run -- add "Test$(printf '\t')injection"
cargo run -- list
# Expected: Should show error or sanitized description

# 3. Test crash recovery
# Kill process during write:
cargo run -- add "Test" &
PID=$!
sleep 0.1
kill -9 $PID
# Verify: .tasklist should be intact (atomic write works)

# 4. Test path traversal
cargo run -- --file /tmp/test.txt add "Test"
# Expected: Creates /tmp/test.txt (user has permissions)
```

### Fuzzing (Advanced)

```bash
# Install cargo-fuzz
cargo install cargo-fuzz

# Create fuzz target for description input
# See: https://rust-fuzz.github.io/book/cargo-fuzz.html
```

---

## Security Checklist for Contributors

Before submitting PRs that touch file operations or user input:

- [ ] No new `unsafe` blocks without justification
- [ ] User input validated and sanitized
- [ ] File operations maintain atomicity
- [ ] Locks acquired before reads in critical sections
- [ ] Errors propagated with context (no silent failures)
- [ ] Clippy warnings resolved
- [ ] Documentation updated
- [ ] Tests added for new functionality
- [ ] Security implications documented

---

## Conclusion

**taskboard is secure for personal and team use.** The codebase demonstrates:

✅ Excellent use of Rust's memory safety guarantees
✅ Proper atomic file operations with locking
✅ Good architecture and error handling
⚠️ Minor input validation issues (easily fixed)
⚠️ Some idiomatic Rust improvements needed

**Recommended for:**

- Personal task management ✅
- Team collaboration ✅
- Multi-user systems (with proper file permissions) ✅
- Production use (after fixing HIGH priority issues) ✅

**Not recommended for:**

- High-security environments requiring audit trails ❌
- Systems requiring guaranteed consistency under attack ❌
- Untrusted multi-tenant environments ❌

**Overall Security Rating: 8.5/10** - Very Good

---

**Last Reviewed**: January 7, 2026
**Reviewer**: Automated Security Analysis + Manual Code Review
**Next Review**: Recommended after any changes to file operations or input handling
