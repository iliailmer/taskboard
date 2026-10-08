use serde_json::Value;
use std::env;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

// Helper to run a tsk command with a specific file
fn run_command(temp_dir: &PathBuf, args: &[&str]) -> std::process::Output {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let binary_path = format!("{}/target/debug/tsk", manifest_dir);

    std::process::Command::new(&binary_path)
        .args(args)
        .current_dir(temp_dir)
        .output()
        .expect("Failed to run command")
}

fn read_json(temp_dir: &PathBuf) -> Value {
    let output = run_command(temp_dir, &["--file", ".tasklist", "show", "--json"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn ok(temp_dir: &PathBuf, args: &[&str]) {
    let mut full = vec!["--file", ".tasklist"];
    full.extend_from_slice(args);
    let output = run_command(temp_dir, &full);
    assert!(
        output.status.success(),
        "{:?}: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn fails(temp_dir: &PathBuf, args: &[&str], expected_stderr: &str) {
    let mut full = vec!["--file", ".tasklist"];
    full.extend_from_slice(args);
    let output = run_command(temp_dir, &full);
    assert!(!output.status.success(), "{:?} should fail", args);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(expected_stderr), "stderr: {}", stderr);
}

#[test]
fn test_task_lifecycle() {
    let temp_dir = TempDir::new().unwrap();
    let dir = temp_dir.path().to_path_buf();
    let title = "Quote \" and 🎉";
    let description = "first\nsecond \\ \"quoted\"";

    ok(&dir, &["add", title, "--description", description]);
    ok(&dir, &["add", "Second"]);
    let tasks = read_json(&dir);
    assert_eq!(tasks[0]["id"], 1);
    assert_eq!(tasks[0]["title"], title);
    assert_eq!(tasks[0]["description"], description);
    assert_eq!(tasks[0]["status"], "not_started");
    assert_eq!(tasks[1]["id"], 2);

    ok(&dir, &["update", "--id", "1", "--title", "Renamed"]);
    ok(&dir, &["update", "--id", "1", "--status", "done"]);
    let tasks = read_json(&dir);
    assert_eq!(tasks[0]["title"], "Renamed");
    assert_eq!(tasks[0]["description"], description);
    assert_eq!(tasks[0]["status"], "done");

    fails(&dir, &["update", "--id", "1"], "Nothing to update");
    fails(
        &dir,
        &["update", "--id", "999", "--status", "done"],
        "not found",
    );
    fails(&dir, &["delete", "--id", "999"], "not found");
    fails(&dir, &["add", "--title", "\t\n"], "title cannot be empty");

    ok(&dir, &["delete", "--id", "1"]);
    let tasks = read_json(&dir);
    assert_eq!(tasks.as_array().unwrap().len(), 1);
    assert_eq!(tasks[0]["id"], 2);
}

#[test]
fn test_deleted_id_is_not_reused() {
    let temp_dir = TempDir::new().unwrap();
    let dir = temp_dir.path().to_path_buf();

    ok(&dir, &["add", "First"]);
    ok(&dir, &["add", "Second"]);
    ok(&dir, &["delete", "--id", "2"]);
    ok(&dir, &["add", "Third"]);

    let tasks = read_json(&dir);
    assert_eq!(tasks[1]["id"], 3);
}

#[test]
fn test_separators_in_title_do_not_corrupt_row() {
    let temp_dir = TempDir::new().unwrap();
    let dir = temp_dir.path().to_path_buf();

    ok(&dir, &["add", "part1\tpart2\npart3"]);

    let content = fs::read_to_string(dir.join(".tasklist")).unwrap();
    let row = content.lines().find(|l| !l.starts_with('#')).unwrap();
    assert_eq!(row.split('\t').count(), 5, "row: {:?}", row);
    assert!(row.contains("part1 part2 part3"));
}

#[test]
fn test_legacy_format_is_read_and_migrated() {
    let temp_dir = TempDir::new().unwrap();
    let dir = temp_dir.path().to_path_buf();
    fs::write(
        dir.join(".tasklist"),
        "1\t🚀 Not Started\tOld 1\t2025-01-01 10:00\n2\t✅ Done\tOld 2\t2025-01-01 11:00\n",
    )
    .unwrap();

    assert_eq!(read_json(&dir)[0]["title"], "Old 1");

    ok(&dir, &["add", "New"]);
    let content = fs::read_to_string(dir.join(".tasklist")).unwrap();
    assert!(content.starts_with("#max_id=3"), "content: {content}");
    assert!(
        content
            .lines()
            .filter(|l| !l.starts_with('#'))
            .all(|l| l.split('\t').count() == 5),
        "content: {content}"
    );
}

#[test]
fn test_concurrent_adds_do_not_lose_tasks() {
    let temp_dir = TempDir::new().unwrap();
    let dir = temp_dir.path().to_path_buf();
    let binary_path = format!("{}/target/debug/tsk", env!("CARGO_MANIFEST_DIR"));

    let children: Vec<_> = (1..=8)
        .map(|i| {
            std::process::Command::new(&binary_path)
                .args(["--file", ".tasklist", "add", &format!("Concurrent {}", i)])
                .current_dir(&dir)
                .spawn()
                .expect("Failed to spawn command")
        })
        .collect();
    for mut child in children {
        assert!(child.wait().unwrap().success());
    }

    let tasks = read_json(&dir);
    assert_eq!(tasks.as_array().unwrap().len(), 8);
    let content = fs::read_to_string(dir.join(".tasklist")).unwrap();
    assert!(content.starts_with("#max_id=8"), "content: {content}");
}

#[test]
fn test_lockfile() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();
    let args = ["add", "Task"];
    let _ = run_command(&temp_path, &args);

    let list_exists = temp_path.join(".tasklist").exists();
    let lock_exists = temp_path.join(".tasklist.lock").exists();

    assert!(list_exists);
    assert!(!lock_exists);
}

// Runs a command on a tasklist with the given content. The command must fail,
// name the file on stderr, print nothing on stdout, and leave the file as it was.
fn assert_rejected(content: &str, args: &[&str]) {
    let temp_dir = TempDir::new().unwrap();
    let dir = temp_dir.path().to_path_buf();
    let path = dir.join(".tasklist");
    fs::write(&path, content).unwrap();

    let mut full = vec!["--file", ".tasklist"];
    full.extend_from_slice(args);
    let output = run_command(&dir, &full);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "{:?} should fail\nstdout: {}",
        args,
        stdout
    );
    assert!(stderr.contains(".tasklist"), "stderr: {}", stderr);
    assert!(stdout.is_empty(), "stdout: {}", stdout);
    assert_eq!(fs::read_to_string(&path).unwrap(), content);
}

const GOOD_ROW: &str = "1\t🚀 Not Started\tTask\t\t2025-01-01 10:00\n";

#[test]
fn test_bad_max_id_header_is_rejected() {
    let content = format!("#max_id=abc\n{GOOD_ROW}");
    assert_rejected(&content, &["show", "--json"]);
    assert_rejected(&content, &["add", "New"]);
}

#[test]
fn test_truncated_row_is_rejected() {
    let content = format!("#max_id=2\n{GOOD_ROW}2\t🚀 Not Started\n");
    assert_rejected(&content, &["show", "--json"]);
    assert_rejected(&content, &["add", "New"]);
    assert_rejected(&content, &["delete", "--id", "1"]);
}

#[cfg(unix)]
#[test]
fn test_unreadable_file_is_not_replaced() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = TempDir::new().unwrap();
    let dir = temp_dir.path().to_path_buf();
    let path = dir.join(".tasklist");
    let content = format!("#max_id=1\n{GOOD_ROW}");
    fs::write(&path, &content).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();

    let output = run_command(&dir, &["--file", ".tasklist", "add", "New"]);

    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "add should fail");
    assert!(stderr.contains(".tasklist"), "stderr: {}", stderr);
    assert_eq!(fs::read_to_string(&path).unwrap(), content);
}

#[cfg(unix)]
#[test]
fn test_failed_write_keeps_old_file() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = TempDir::new().unwrap();
    let dir = temp_dir.path().to_path_buf();
    let path = dir.join(".tasklist");
    let content = format!("#max_id=1\n{GOOD_ROW}");
    fs::write(&path, &content).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();

    let output = run_command(&dir, &["--file", ".tasklist", "add", "New"]);

    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(!output.status.success(), "add should fail");
    assert_eq!(fs::read_to_string(&path).unwrap(), content);
    let leftovers: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .filter(|n| n != ".tasklist")
        .collect();
    assert!(leftovers.is_empty(), "leftover files: {:?}", leftovers);
}

#[test]
fn test_unknown_comment_line_is_ignored() {
    let temp_dir = TempDir::new().unwrap();
    let dir = temp_dir.path().to_path_buf();
    fs::write(
        dir.join(".tasklist"),
        format!("# my notes\n#max_id=1\n{GOOD_ROW}"),
    )
    .unwrap();

    assert_eq!(read_json(&dir)[0]["id"], 1);
}
