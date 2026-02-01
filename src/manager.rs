use crate::task::{Status, Task};
use colored::Colorize;
use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Error, Write};
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

    pub fn add_task(&self, description: String) -> Result<(), Error> {
        if description.is_empty() {
            return Err(Error::new(
                std::io::ErrorKind::InvalidInput,
                "Task description cannot be empty",
            ));
        }

        let (mut max_id, has_metadata) = self.read_metadata()?;

        if !has_metadata {
            max_id = self.scan_max_id()?;
        }

        let mut existing_tasks = Vec::new();
        if let Ok(file) = OpenOptions::new().read(true).open(&self.tasklist_path) {
            let reader = BufReader::new(&file);
            for line in reader.lines() {
                let line =
                    line.map_err(|e| Error::new(e.kind(), format!("Failed to read line: {}", e)))?;
                if !line.starts_with("#") && !line.is_empty() {
                    existing_tasks.push(line);
                }
            }
        }

        let new_id = max_id + 1;
        let today = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
        let task = Task::new(new_id, Status::NotStarted, description.clone(), today);

        self.atomic_write(|writer| {
            self.write_metadata(writer, new_id)?;

            for task_line in &existing_tasks {
                writeln!(writer, "{}", task_line)?;
            }
            task.write_to(writer)?;

            Ok(())
        })?;

        Ok(())
    }

    pub fn update_task(
        &self,
        id: i32,
        status: Status,
        description: Option<String>,
    ) -> Result<(), Error> {
        let (max_id, has_metadata) = self.read_metadata()?;

        let tasklist = OpenOptions::new()
            .read(true)
            .open(&self.tasklist_path)
            .map_err(|e| {
                Error::new(
                    e.kind(),
                    format!("Could not open task list {}: {}", self.tasklist_path, e),
                )
            })?;
        let reader = BufReader::new(&tasklist);
        let lines: Vec<String> = reader
            .lines()
            .collect::<Result<Vec<String>, _>>()
            .map_err(|e| Error::new(e.kind(), format!("Failed to read lines: {}", e)))?;

        let mut task_found = false;
        let mut updated_lines = Vec::new();

        for line in lines {
            if line.starts_with("#") {
                continue;
            }

            let parts: Vec<&str> = line.split("\t").collect();
            if parts.len() >= 3 {
                if let Ok(task_id) = parts[0].parse::<i32>() {
                    if task_id == id {
                        task_found = true;
                        let new_description =
                            String::from(description.as_deref().unwrap_or(parts[2]));
                        let today = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
                        let task = Task::new(id, status, new_description, today);
                        updated_lines.push(task.to_file_string());
                    } else {
                        updated_lines.push(line);
                    }
                } else {
                    updated_lines.push(line);
                }
            } else if !line.is_empty() {
                updated_lines.push(line);
            }
        }

        if !task_found {
            return Err(Error::new(
                std::io::ErrorKind::NotFound,
                format!("Task with ID {} not found", id),
            ));
        }

        let current_max_id = if has_metadata {
            max_id
        } else {
            self.scan_max_id()?
        };

        self.atomic_write(|writer| {
            self.write_metadata(writer, current_max_id)?;

            for line in &updated_lines {
                writeln!(writer, "{}", line)?;
            }

            Ok(())
        })?;

        Ok(())
    }

    pub fn get_tasks(&self) -> Result<Vec<Task>, Error> {
        let tasklist = OpenOptions::new()
            .read(true)
            .open(&self.tasklist_path)
            .map_err(|e| {
                Error::new(
                    e.kind(),
                    format!("Could not read task list {}: {}", self.tasklist_path, e),
                )
            })?;
        let reader = BufReader::new(&tasklist);
        let mut tasks: Vec<Task> = vec![];
        for line in reader.lines() {
            let line =
                line.map_err(|e| Error::new(e.kind(), format!("Failed to read line: {}", e)))?;
            if line.starts_with("#") {
                continue;
            }
            let parts: Vec<&str> = line.split("\t").collect();
            if parts.len() >= 3
                && let Ok(id) = parts[0].parse::<i32>()
            {
                let status = Status::from_str(parts[1]);
                let today = String::from(match parts.get(3) {
                    Some(x) => x,
                    None => "",
                });
                let task = Task::new(id, status, parts[2].to_string(), today);
                tasks.push(task);
            }
        }
        Ok(tasks)
    }

    pub fn list_tasks(&self, kanban: bool) -> Result<(), Error> {
        println!(
            "Project: {}",
            self.title.as_ref().unwrap_or(&String::from("My Tasks"))
        );
        let tasks = self.get_tasks()?;

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
                        // Include date in the display (first line: ID + desc, second line: date)
                        let id_prefix = format!("[{}] ", task.id);
                        let desc_max_len = column_width.saturating_sub(id_prefix.len() + 3);

                        let truncated = if task.description.len() > desc_max_len {
                            format!("{}...", &task.description[..desc_max_len.saturating_sub(3)])
                        } else {
                            task.description.clone()
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
        let (max_id, has_metadata) = self.read_metadata()?;

        let tasklist = OpenOptions::new()
            .read(true)
            .open(&self.tasklist_path)
            .map_err(|e| {
                Error::new(
                    e.kind(),
                    format!("Could not open task list {}: {}", self.tasklist_path, e),
                )
            })?;
        let reader = BufReader::new(&tasklist);
        let lines: Vec<String> = reader
            .lines()
            .collect::<Result<Vec<String>, _>>()
            .map_err(|e| Error::new(e.kind(), format!("Failed to read lines: {}", e)))?;

        let mut task_found = false;
        let mut filtered_lines = Vec::new();

        for line in lines {
            if line.starts_with("#") {
                continue;
            }

            let parts: Vec<&str> = line.split("\t").collect();
            if let Some(first_part) = parts.first()
                && let Ok(task_id) = first_part.parse::<i32>()
                && task_id == id
            {
                task_found = true;
                continue; // Skip this line (delete it)
            }
            filtered_lines.push(line);
        }

        if !task_found {
            return Err(Error::new(
                std::io::ErrorKind::NotFound,
                format!("Task with ID {} not found", id),
            ));
        }

        let current_max_id = if has_metadata {
            max_id
        } else {
            self.scan_max_id()?
        };

        self.atomic_write(|writer| {
            self.write_metadata(writer, current_max_id)?;

            for line in &filtered_lines {
                writeln!(writer, "{}", line)?;
            }

            Ok(())
        })?;

        Ok(())
    }

    fn read_metadata(&self) -> Result<(i32, bool), Error> {
        let file = OpenOptions::new().read(true).open(&self.tasklist_path);

        match file {
            Ok(f) => {
                let reader = BufReader::new(f);
                let mut lines = reader.lines();

                if let Some(Ok(first_line)) = lines.next()
                    && first_line.starts_with("#max_id=")
                    && let Some(id_str) = first_line.strip_prefix("#max_id=")
                    && let Ok(max_id) = id_str.parse::<i32>()
                {
                    return Ok((max_id, true));
                }
                Ok((0, false))
            },
            Err(_) => Ok((0, false)),
        }
    }

    fn write_metadata<W: Write>(&self, writer: &mut W, max_id: i32) -> Result<(), Error> {
        writeln!(writer, "#max_id={}", max_id)
    }

    fn scan_max_id(&self) -> Result<i32, Error> {
        let file = OpenOptions::new().read(true).open(&self.tasklist_path);

        match file {
            Ok(f) => {
                let reader = BufReader::new(f);
                let mut max_id = 0;
                for line in reader.lines() {
                    let line = line
                        .map_err(|e| Error::new(e.kind(), format!("Failed to read line: {}", e)))?;
                    if line.starts_with("#") {
                        continue; // Skip metadata
                    }
                    if let Some(id_str) = line.split("\t").next()
                        && let Ok(id) = id_str.parse::<i32>()
                    {
                        max_id = max_id.max(id);
                    }
                }
                Ok(max_id)
            },
            Err(_) => Ok(0),
        }
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

        // Get the file handle and lock it exclusively
        let file = temp_file.as_file();
        file.lock_exclusive()
            .map_err(|e| Error::other(format!("Failed to lock temporary file: {}", e)))?;

        // Write to the temporary file
        {
            let mut writer = BufWriter::new(file);
            write_fn(&mut writer)?;
            writer.flush()?;
        } // Writer dropped here, releasing the file reference

        // Unlock the file
        temp_file.as_file().unlock().ok();

        // Atomically replace the original file
        temp_file
            .persist(&self.tasklist_path)
            .map_err(|e| Error::other(format!("Failed to persist temporary file: {}", e)))?;

        Ok(())
    }
}
