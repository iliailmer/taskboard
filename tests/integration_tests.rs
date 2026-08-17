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

#[test]
fn test_add_task_creates_metadata() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    // Add a task using relative path (will be created in temp_dir)
    let output = run_command(&temp_path, &["--file", ".tasklist", "add", "First task"]);

    assert!(
        output.status.success(),
        "Command failed: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );

    let tasklist_path = temp_path.join(".tasklist");
    assert!(
        tasklist_path.exists(),
        "File was not created at {:?}",
        tasklist_path
    );

    // Verify metadata exists and task was added
    let content = fs::read_to_string(&tasklist_path).unwrap();
    assert!(content.starts_with("#max_id=1"), "Content: {}", content);
    assert!(content.contains("First task"));
}

#[test]
fn test_add_multiple_tasks_increments_id() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    // Add first task
    run_command(&temp_path, &["--file", ".tasklist", "add", "Task 1"]);

    // Add second task
    run_command(&temp_path, &["--file", ".tasklist", "add", "Task 2"]);

    // Verify max_id is 2
    let tasklist_path = temp_path.join(".tasklist");
    let content = fs::read_to_string(&tasklist_path).unwrap();
    assert!(content.starts_with("#max_id=2"));
    assert!(content.contains("Task 1"));
    assert!(content.contains("Task 2"));

    // Count task lines (excluding metadata)
    let task_count = content.lines().filter(|l| !l.starts_with("#")).count();
    assert_eq!(task_count, 2);
}

#[test]
fn test_update_task_preserves_metadata() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();
    let tasklist_path = temp_path.join(".tasklist");

    fs::write(
        &tasklist_path,
        "#max_id=1\n1\t🚀 Not Started\tOriginal task\t2025-01-01 10:00\n",
    )
    .unwrap();

    // Update the task
    let output = run_command(
        &temp_path,
        &[
            "--file",
            ".tasklist",
            "update",
            "--id",
            "1",
            "--status",
            "in_progress",
        ],
    );

    assert!(output.status.success());

    // Verify metadata is preserved
    let content = fs::read_to_string(&tasklist_path).unwrap();
    assert!(content.starts_with("#max_id=1"));
    assert!(content.contains("In Progress"));
    let task_line = content.lines().find(|line| !line.starts_with('#')).unwrap();
    assert_eq!(task_line.split('\t').count(), 5);
}

#[test]
fn test_update_nonexistent_task_fails() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    // Add a task
    run_command(&temp_path, &["--file", ".tasklist", "add", "Task 1"]);

    // Try to update non-existent task
    let output = run_command(
        &temp_path,
        &[
            "--file",
            ".tasklist",
            "update",
            "--id",
            "999",
            "--status",
            "done",
        ],
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Task with ID 999 not found"));
}

#[test]
fn test_delete_task_preserves_metadata() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();
    let tasklist_path = temp_path.join(".tasklist");

    fs::write(
        &tasklist_path,
        "#max_id=2\n1\t🚀 Not Started\tTask 1\t2025-01-01 10:00\n\
         2\t✅ Done\tTask 2\t2025-01-01 11:00\n",
    )
    .unwrap();

    // Delete first task
    let output = run_command(&temp_path, &["--file", ".tasklist", "delete", "--id", "1"]);

    assert!(output.status.success());

    // Verify metadata is preserved
    let content = fs::read_to_string(&tasklist_path).unwrap();
    assert!(content.starts_with("#max_id=2"));
    assert!(!content.contains("Task 1"));
    assert!(content.contains("Task 2"));
    let task_line = content.lines().find(|line| !line.starts_with('#')).unwrap();
    assert_eq!(task_line.split('\t').count(), 5);
}

#[test]
fn test_delete_nonexistent_task_fails() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    // Add a task
    run_command(&temp_path, &["--file", ".tasklist", "add", "Task 1"]);

    // Try to delete non-existent task
    let output = run_command(
        &temp_path,
        &["--file", ".tasklist", "delete", "--id", "999"],
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Task with ID 999 not found"));
}

#[test]
fn test_migration_from_old_format() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();
    let tasklist_path = temp_path.join(".tasklist");

    // Create old format file (without metadata)
    fs::write(
        &tasklist_path,
        "1\t🚀 Not Started\tOld task 1\t2025-01-01 10:00\n\
         2\t✅ Done\tOld task 2\t2025-01-01 11:00\n",
    )
    .unwrap();

    // Add a new task (should trigger migration)
    let output = run_command(
        &temp_path,
        &["--file", ".tasklist", "add", "New task after migration"],
    );

    assert!(output.status.success());

    // Verify migration happened
    let content = fs::read_to_string(&tasklist_path).unwrap();
    assert!(content.starts_with("#max_id=3"));
    assert!(content.contains("Old task 1"));
    assert!(content.contains("Old task 2"));
    assert!(content.contains("New task after migration"));
}

#[test]
fn test_list_tasks_skips_metadata() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    // Add tasks
    run_command(&temp_path, &["--file", ".tasklist", "add", "Task 1"]);
    run_command(&temp_path, &["--file", ".tasklist", "add", "Task 2"]);

    // List tasks
    let output = run_command(&temp_path, &["--file", ".tasklist", "show"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Task 1"));
    assert!(stdout.contains("Task 2"));
    assert!(!stdout.contains("#max_id"));
}

#[test]
fn test_empty_tasklist_shows_message() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();
    let tasklist_path = temp_path.join("empty.tasklist");

    // Create empty file
    fs::write(&tasklist_path, "#max_id=0\n").unwrap();

    // List tasks
    let output = run_command(&temp_path, &["--file", "empty.tasklist", "show"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No tasks found"));
}

#[test]
fn test_kanban_view_works() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    // Add tasks with different statuses
    run_command(&temp_path, &["--file", ".tasklist", "add", "Todo task"]);
    run_command(
        &temp_path,
        &["--file", ".tasklist", "add", "In progress task"],
    );
    run_command(
        &temp_path,
        &[
            "--file",
            ".tasklist",
            "update",
            "--id",
            "2",
            "--status",
            "in_progress",
        ],
    );

    // Show kanban view
    let output = run_command(&temp_path, &["--file", ".tasklist", "show", "--kanban"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("NOT STARTED"));
    assert!(stdout.contains("IN PROGRESS"));
    assert!(stdout.contains("DONE"));
}

#[test]
fn test_verbose_flag_shows_file_path() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    // Add a task first
    run_command(&temp_path, &["--file", ".tasklist", "add", "Test task"]);

    // Run with verbose flag
    let output = run_command(&temp_path, &["--file", ".tasklist", "--verbose", "show"]);

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Using tasklist file:"));
    assert!(stderr.contains(".tasklist"));
}

#[test]
fn test_default_command_shows_tasks() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    // Add a task
    run_command(&temp_path, &["--file", ".tasklist", "add", "Test task"]);

    // Run without subcommand (should default to show)
    let output = run_command(&temp_path, &["--file", ".tasklist"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Test task"));
    assert!(stdout.contains("Project:"));
}

#[test]
fn test_global_kanban_flag() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    // Add tasks
    run_command(&temp_path, &["--file", ".tasklist", "add", "Task 1"]);
    run_command(&temp_path, &["--file", ".tasklist", "add", "Task 2"]);

    // Use global --kanban flag (without subcommand)
    let output = run_command(&temp_path, &["--file", ".tasklist", "--kanban"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("NOT STARTED"));
    assert!(stdout.contains("IN PROGRESS"));
    assert!(stdout.contains("DONE"));
    assert!(stdout.contains("Task 1"));
}

#[test]
fn test_kanban_shows_dates() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    // Add a task
    run_command(
        &temp_path,
        &["--file", ".tasklist", "add", "Task with date"],
    );

    // Show in kanban view
    let output = run_command(&temp_path, &["--file", ".tasklist", "show", "--kanban"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Check that dates are displayed (format: YYYY-MM-DD HH:MM)
    assert!(stdout.contains("20")); // Year starts with "20"
    assert!(stdout.contains("Task with date"));
}

#[test]
fn test_file_flag_with_new_path_is_honored() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();
    let target = temp_path.join("newlist");

    let output = run_command(
        &temp_path,
        &["--file", target.to_str().unwrap(), "add", "X"],
    );
    assert!(
        output.status.success(),
        "{:?}",
        String::from_utf8_lossy(&output.stderr)
    );
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
        &["--file", target.to_str().unwrap(), "add", "X"],
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("does not exist"), "stderr: {}", stderr);
}

#[test]
fn test_add_with_positional_title_and_description() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    let output = run_command(
        &temp_path,
        &["--file", ".tasklist", "add", "Positional task"],
    );
    assert!(
        output.status.success(),
        "{:?}",
        String::from_utf8_lossy(&output.stderr)
    );

    let content = fs::read_to_string(temp_path.join(".tasklist")).unwrap();
    assert!(content.contains("Positional task"));

    // Positional title and -d description are independent
    let output = run_command(
        &temp_path,
        &["--file", ".tasklist", "add", "pos", "--description", "flag"],
    );
    assert!(output.status.success());
    let content = fs::read_to_string(temp_path.join(".tasklist")).unwrap();
    assert!(content.contains("pos\tflag"));

    // Neither is an error
    let output = run_command(&temp_path, &["--file", ".tasklist", "add"]);
    assert!(!output.status.success());
}

#[test]
fn test_title_tabs_and_newlines_sanitized() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    let output = run_command(
        &temp_path,
        &["--file", ".tasklist", "add", "part1\tpart2\npart3"],
    );
    assert!(output.status.success());

    let content = fs::read_to_string(temp_path.join(".tasklist")).unwrap();
    let task_line = content.lines().find(|l| !l.starts_with('#')).unwrap();
    assert_eq!(
        task_line.split('\t').count(),
        5,
        "Embedded separators corrupted the row: {:?}",
        task_line
    );
    assert!(task_line.contains("part1 part2 part3"));
}

#[test]
fn test_kanban_with_long_emoji_title_does_not_panic() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    let long_emoji = "🎉".repeat(60);
    let output = run_command(&temp_path, &["--file", ".tasklist", "add", &long_emoji]);
    assert!(output.status.success());

    let output = run_command(&temp_path, &["--file", ".tasklist", "show", "--kanban"]);
    assert!(
        output.status.success(),
        "kanban panicked: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_show_with_missing_file_is_friendly() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    let output = run_command(&temp_path, &["--file", "nothere.tasklist", "show"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No tasks found"));
}

#[test]
fn test_concurrent_adds_do_not_lose_tasks() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let binary_path = format!("{}/target/debug/tsk", manifest_dir);

    let children: Vec<_> = (1..=8)
        .map(|i| {
            std::process::Command::new(&binary_path)
                .args(["--file", ".tasklist", "add", &format!("Concurrent {}", i)])
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

#[test]
fn test_atomic_write_prevents_corruption() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    // Add multiple tasks in quick succession
    for i in 1..=5 {
        let output = run_command(
            &temp_path,
            &["--file", ".tasklist", "add", &format!("Task {}", i)],
        );
        assert!(output.status.success());
    }

    // Verify all tasks were added correctly
    let tasklist_path = temp_path.join(".tasklist");
    let content = fs::read_to_string(&tasklist_path).unwrap();

    // Should have metadata
    assert!(content.starts_with("#max_id=5"));

    // Should have all 5 tasks
    for i in 1..=5 {
        assert!(content.contains(&format!("Task {}", i)));
    }

    // Should have exactly 6 lines (1 metadata + 5 tasks)
    assert_eq!(content.lines().count(), 6);
}

#[test]
fn test_add_writes_title_and_description_as_five_fields() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    let output = run_command(
        &temp_path,
        &[
            "--file",
            ".tasklist",
            "add",
            "--title",
            "Ship title",
            "--description",
            "first line\nsecond \\ line\tend",
        ],
    );

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let content = fs::read_to_string(temp_path.join(".tasklist")).unwrap();
    let task_line = content.lines().find(|line| !line.starts_with('#')).unwrap();
    let fields: Vec<_> = task_line.split('\t').collect();
    assert_eq!(fields.len(), 5, "task row: {task_line:?}");
    assert_eq!(fields[2], "Ship title");
    assert_eq!(fields[3], r"first line\nsecond \\ line end");
}

#[test]
fn test_add_requires_nonempty_sanitized_title() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    let output = run_command(
        &temp_path,
        &["--file", ".tasklist", "add", "--title", "\t\n\r"],
    );

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Task title cannot be empty"));
}

#[test]
fn test_legacy_rows_read_and_migrate_to_five_fields() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();
    let tasklist_path = temp_path.join(".tasklist");
    fs::write(
        &tasklist_path,
        "#max_id=1\n1\t🚀 Not Started\tLegacy label\t2025-01-01 10:00\n",
    )
    .unwrap();

    let output = run_command(&temp_path, &["--file", ".tasklist", "show", "--json"]);
    assert!(output.status.success());
    let tasks: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(tasks[0]["title"], "Legacy label");
    assert_eq!(tasks[0]["description"], "");

    let output = run_command(&temp_path, &["--file", ".tasklist", "add", "New task"]);
    assert!(output.status.success());
    let content = fs::read_to_string(tasklist_path).unwrap();
    assert!(
        content
            .lines()
            .filter(|line| !line.starts_with('#'))
            .all(|line| line.split('\t').count() == 5),
        "content: {content}"
    );
}

#[test]
fn test_update_fields_independently_and_rejects_noop() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();
    assert!(
        run_command(
            &temp_path,
            &[
                "--file",
                ".tasklist",
                "add",
                "Original",
                "--description",
                "Original body",
            ],
        )
        .status
        .success()
    );

    let output = run_command(
        &temp_path,
        &[
            "--file",
            ".tasklist",
            "update",
            "--id",
            "1",
            "--title",
            "Renamed",
        ],
    );
    assert!(output.status.success());
    let tasks = read_json(&temp_path);
    assert_eq!(tasks[0]["title"], "Renamed");
    assert_eq!(tasks[0]["description"], "Original body");
    assert_eq!(tasks[0]["status"], "not_started");

    let output = run_command(
        &temp_path,
        &[
            "--file",
            ".tasklist",
            "update",
            "--id",
            "1",
            "--description",
            "New body",
        ],
    );
    assert!(output.status.success());
    let tasks = read_json(&temp_path);
    assert_eq!(tasks[0]["title"], "Renamed");
    assert_eq!(tasks[0]["description"], "New body");
    assert_eq!(tasks[0]["status"], "not_started");

    let output = run_command(
        &temp_path,
        &[
            "--file",
            ".tasklist",
            "update",
            "--id",
            "1",
            "--status",
            "done",
        ],
    );
    assert!(output.status.success());
    let tasks = read_json(&temp_path);
    assert_eq!(tasks[0]["title"], "Renamed");
    assert_eq!(tasks[0]["description"], "New body");
    assert_eq!(tasks[0]["status"], "done");

    let output = run_command(
        &temp_path,
        &[
            "--file",
            ".tasklist",
            "update",
            "--id",
            "1",
            "--status",
            "in_progress",
            "--title",
            "Combined",
            "--description",
            "Combined body",
        ],
    );
    assert!(output.status.success());
    let tasks = read_json(&temp_path);
    assert_eq!(tasks[0]["title"], "Combined");
    assert_eq!(tasks[0]["description"], "Combined body");
    assert_eq!(tasks[0]["status"], "in_progress");

    let output = run_command(
        &temp_path,
        &[
            "--file",
            ".tasklist",
            "update",
            "--id",
            "1",
            "--description",
            "",
        ],
    );
    assert!(output.status.success());
    let tasks = read_json(&temp_path);
    assert_eq!(tasks[0]["description"], "");

    let output = run_command(&temp_path, &["--file", ".tasklist", "update", "--id", "1"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Nothing to update"));
}

#[test]
fn test_json_round_trips_multiline_and_special_characters() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();
    let title = "Quote \" and 🎉";
    let description = "first\nsecond \\ \"quoted\"";
    assert!(
        run_command(
            &temp_path,
            &[
                "--file",
                ".tasklist",
                "add",
                title,
                "--description",
                description,
            ],
        )
        .status
        .success()
    );

    let output = run_command(&temp_path, &["--file", ".tasklist", "show", "--json"]);
    assert!(output.status.success());
    let tasks: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(tasks[0]["status"], "not_started");
    assert_eq!(tasks[0]["title"], title);
    assert_eq!(tasks[0]["description"], description);
}
