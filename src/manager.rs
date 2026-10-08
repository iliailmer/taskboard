use crate::task::{Status, Task};
use colored::Colorize;
use std::collections::hash_map::DefaultHasher;
use std::fs::{File, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, BufWriter, Error, ErrorKind, Write};
use std::num::IntErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use tabled::settings::object::Segment;
use tabled::settings::{Modify, Width};
use tabled::{Table, settings::Style};
#[derive(Debug)]
pub struct Mngr {
    tasklist_path: String,
    title: Option<String>,
}

impl Mngr {
    pub fn new(tasklist_path: String, title: Option<String>) -> Self {
        Self {
            tasklist_path,
            title,
        }
    }

    fn write_file(&self, tasks: &[Task], max_id: i32) -> Result<(), Error> {
        self.atomic_write(|writer| {
            self.write_metadata(writer, max_id)?;

            for task_line in tasks {
                task_line.write_to(writer)?;
            }
            Ok(())
        })?;
        Ok(())
    }
    fn read_or_create_file(&self) -> Result<(Vec<Task>, i32), Error> {
        let file = match OpenOptions::new().read(true).open(&self.tasklist_path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((vec![], 0)),
            Err(e) => {
                return Err(Error::new(
                    e.kind(),
                    format!("Could not read task list {}: {}", self.tasklist_path, e),
                ));
            },
        };
        let mut max_id = 0;
        let mut existing_tasks = Vec::new();
        let reader = BufReader::new(&file);
        for line in reader.lines() {
            let line =
                line.map_err(|e| Error::new(e.kind(), format!("Failed to read line: {}", e)))?;
            if line.is_empty() {
                continue;
            }
            if line.starts_with("#max_id=") {
                let id_str = line.strip_prefix("#max_id=").unwrap_or("");
                max_id = match id_str.parse::<i32>() {
                    Ok(id) => id,
                    Err(e) if e.kind() == &IntErrorKind::Empty => {
                        return Err(Error::new(
                            ErrorKind::InvalidData,
                            format!(
                                "Error parsing {}, metadata string {}",
                                self.tasklist_path, line
                            ),
                        ));
                    },
                    Err(_) => {
                        return Err(Error::new(
                            ErrorKind::InvalidData,
                            format!(
                                "Error parsing {}, metadata string {}",
                                self.tasklist_path, line
                            ),
                        ));
                    },
                };
            }
            if !line.starts_with("#") && !line.is_empty() {
                let migrated = Task::from_file_line(&line);
                match migrated {
                    Some(m) => {
                        max_id = max_id.max(m.id);
                        existing_tasks.push(m);
                    },
                    None => {
                        return Err(Error::new(
                            ErrorKind::InvalidData,
                            format!("Invalid Line Format: {} in {}", line, self.tasklist_path),
                        ));
                    },
                }
            }
        }
        Ok((existing_tasks, max_id))
    }

    pub fn add_task(&self, title: String, description: String) -> Result<(), Error> {
        let _lock = self.acquire_write_lock()?;

        let title = Self::sanitize_title(&title);
        if title.is_empty() {
            return Err(Error::new(
                std::io::ErrorKind::InvalidInput,
                "Task title cannot be empty",
            ));
        }
        let description = Self::sanitize_description(&description);

        let (mut existing_tasks, mut max_id) = self.read_or_create_file()?;
        max_id += 1;
        let today = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
        let task = Task::new(max_id, Status::NotStarted, title, description, today);
        existing_tasks.push(task);

        self.write_file(&existing_tasks, max_id)?;

        Ok(())
    }

    pub fn update_task(
        &self,
        id: i32,
        status: Option<Status>,
        title: Option<String>,
        description: Option<String>,
    ) -> Result<(), Error> {
        let _lock = self.acquire_write_lock()?;

        if status.is_none() && title.is_none() && description.is_none() {
            return Err(Error::new(
                std::io::ErrorKind::InvalidInput,
                "Nothing to update",
            ));
        }
        let title = title.map(|value| Self::sanitize_title(&value));
        if title.as_ref().is_some_and(String::is_empty) {
            return Err(Error::new(
                std::io::ErrorKind::InvalidInput,
                "Task title cannot be empty",
            ));
        }
        let description = description.map(|d| Self::sanitize_description(&d));
        let (mut existing_tasks, max_id) = self.read_or_create_file()?;

        let mut task_found = false;
        let mut idx = 0;
        for task in &existing_tasks {
            if task.id == id {
                task_found = true;
                break;
            } else {
                idx += 1;
            }
            // updated_tasks.push(task);
        }
        if let Some(task) = existing_tasks.get_mut(idx) {
            task.status = status.unwrap_or(task.status);
            task.title = title.clone().unwrap_or(task.title.clone());
            task.description = description.clone().unwrap_or(task.description.clone());
            task.date = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
        }

        if !task_found {
            return Err(Error::new(
                std::io::ErrorKind::NotFound,
                format!("Task with ID {} not found", id),
            ));
        }

        self.write_file(&existing_tasks, max_id)?;

        Ok(())
    }

    pub fn get_tasks(&self) -> Result<Vec<Task>, Error> {
        // A missing file is an empty task list, not an error
        let (tasks, _) = self.read_or_create_file()?;
        Ok(tasks)
    }

    pub fn list_tasks(&self, kanban: bool, json: bool) -> Result<(), Error> {
        let tasks = self.get_tasks()?;
        if json {
            let output = serde_json::to_string_pretty(&tasks).map_err(Error::other)?;
            println!("{output}");
            return Ok(());
        }

        println!(
            "Project: {}",
            self.title.as_ref().unwrap_or(&String::from("My Tasks"))
        );
        if tasks.is_empty() {
            println!("{}", "No tasks found. Add a task to get started!".yellow());
            return Ok(());
        }

        if kanban {
            self.display_kanban(&tasks);
        } else {
            let builder = Table::builder(tasks).index().column(0).name(None);
            let mut table = builder.build();
            table
                .with(Style::modern())
                .with(Modify::new(Segment::all()).with(Width::wrap(64).keep_words(true)));
            println!("{table}");
        }
        Ok(())
    }

    fn display_kanban(&self, tasks: &[Task]) {
        use std::collections::HashMap;

        let mut grouped: HashMap<Status, Vec<&Task>> = HashMap::new();
        for task in tasks {
            grouped.entry(task.status).or_default().push(task);
        }

        let terminal_width = terminal_size::terminal_size()
            .map(|(terminal_size::Width(w), _)| w as usize)
            .unwrap_or(100); // Default to 100 if detection fails

        let column_width = ((terminal_width - 6) / 3).clamp(25, 50);

        let columns = vec![
            (Status::NotStarted, "🚀 NOT STARTED".cyan().bold()),
            (Status::InProgress, "⏳ IN PROGRESS".yellow().bold()),
            (Status::Done, "✅ DONE".green().bold()),
        ];

        // Print column headers
        println!();
        for (_, header) in &columns {
            print!("{:width$} ", header, width = column_width);
        }
        println!();

        // Print separator
        for _ in &columns {
            print!("{} ", "─".repeat(column_width).bright_black());
        }
        println!();

        // Find max number of tasks in any column
        let max_tasks = grouped.values().map(|v| v.len()).max().unwrap_or(0);

        // Print tasks row by row
        for i in 0..max_tasks {
            for (status, _) in &columns {
                if let Some(task_list) = grouped.get(status) {
                    if let Some(task) = task_list.get(i) {
                        let id_prefix = format!("[{}] ", task.id);
                        let title_max_len = column_width.saturating_sub(id_prefix.len() + 3);

                        let truncated = if task.title.chars().count() > title_max_len {
                            let cut: String = task
                                .title
                                .chars()
                                .take(title_max_len.saturating_sub(3))
                                .collect();
                            format!("{}...", cut)
                        } else {
                            task.title.clone()
                        };

                        let display = format!("{}{}", id_prefix, truncated);
                        print!("{:width$} ", display, width = column_width);
                    } else {
                        print!("{:width$} ", "", width = column_width);
                    }
                } else {
                    print!("{:width$} ", "", width = column_width);
                }
            }
            println!();

            for (status, _) in &columns {
                if let Some(task_list) = grouped.get(status) {
                    if let Some(task) = task_list.get(i) {
                        let date_display = if !task.date.is_empty() {
                            format!("  {}", task.date.bright_black())
                        } else {
                            String::new()
                        };
                        print!("{:width$} ", date_display, width = column_width);
                    } else {
                        print!("{:width$} ", "", width = column_width);
                    }
                } else {
                    print!("{:width$} ", "", width = column_width);
                }
            }
            println!();
        }
        println!();
    }

    pub fn delete_task(&self, id: i32) -> Result<(), Error> {
        let _lock = self.acquire_write_lock()?;
        let (mut tasks, max_id) = self.read_or_create_file()?;
        let len_before = tasks.len();
        tasks.retain(|t| t.id != id);
        let deleted = tasks.len() != len_before;
        if !deleted {
            return Err(Error::new(
                std::io::ErrorKind::NotFound,
                format!("Task with ID {} not found", id),
            ));
        }

        self.write_file(&tasks, max_id)?;

        Ok(())
    }

    fn sanitize_title(title: &str) -> String {
        title.replace(['\t', '\n', '\r'], " ").trim().to_string()
    }

    fn sanitize_description(description: &str) -> String {
        description.replace(['\t', '\r'], " ").trim().to_string()
    }

    // Serializes read-modify-write cycles across processes. The lock lives on a
    // sidecar file (never renamed or deleted) because locking the tasklist itself
    // is unsound with rename-replace: a waiter can end up holding a lock on the
    // old, already-replaced inode. Released when the returned handle is dropped.
    fn acquire_write_lock(&self) -> Result<File, Error> {
        let mut hasher = DefaultHasher::new();
        let tsk_path = Path::new(&self.tasklist_path);
        tsk_path.hash(&mut hasher);
        let hash = hasher.finish();
        let tmp_lock = std::env::temp_dir().join(format!("tsk-{:x}.lock", hash));
        let lock_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&tmp_lock)
            .map_err(|e| {
                Error::new(
                    e.kind(),
                    format!("Failed to open lock file {}: {}", tmp_lock.display(), e),
                )
            })?;
        lock_file
            .lock()
            .map_err(|e| Error::other(format!("Failed to lock {}: {}", tmp_lock.display(), e)))?;
        Ok(lock_file)
    }

    fn write_metadata<W: Write>(&self, writer: &mut W, max_id: i32) -> Result<(), Error> {
        writeln!(writer, "#max_id={}", max_id)
    }

    fn atomic_write<F>(&self, write_fn: F) -> Result<(), Error>
    where
        F: FnOnce(&mut BufWriter<&File>) -> Result<(), Error>,
    {
        let path = Path::new(&self.tasklist_path);
        let parent = path.parent().unwrap_or_else(|| Path::new("."));

        // Create a temporary file in the same directory
        let temp_file = tempfile::Builder::new()
            .prefix(".tasklist.tmp")
            .tempfile_in(parent)
            .map_err(|e| Error::new(e.kind(), format!("Failed to create temporary file: {}", e)))?;

        // Write to the temporary file
        {
            let mut writer = BufWriter::new(temp_file.as_file());
            write_fn(&mut writer)?;
            writer.flush()?;
        } // Writer dropped here, releasing the file reference
        let metadata = std::fs::metadata(path);
        #[cfg(unix)]
        let permissions = match metadata {
            Ok(m) => m.permissions(),
            Err(e) if e.kind() == ErrorKind::NotFound => PermissionsExt::from_mode(0o644),
            Err(e) => return Err(e),
        };
        std::fs::set_permissions(&temp_file, permissions)?;
        // Atomically replace the original file
        temp_file
            .persist(&self.tasklist_path)
            .map_err(|e| Error::other(format!("Failed to persist temporary file: {}", e)))?;

        Ok(())
    }
}
