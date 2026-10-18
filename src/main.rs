mod cli;
mod manager;
mod task;
mod tui;

use crate::cli::{Cli, Commands};
use crate::manager::Mngr;
use crate::task::Task;
use clap::Parser;

use std::fs;
use std::io::Error;
use std::path::PathBuf;

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

fn main() {
    let args = Cli::parse();
    let (tasklist_path, project_title) = match get_tasklist_path(args.file) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        },
    };

    if args.verbose {
        eprintln!("Using tasklist file: {}", tasklist_path);
    }

    let mngr = Mngr::new(tasklist_path.clone(), Some(project_title));

    let result = match args.command {
        Some(Commands::Add {
            text,
            title,
            description,
            json,
        }) => {
            let title = text.or(title).expect("clap enforces one title");
            mngr.add_task(title, description.unwrap_or_default())
                .and_then(|task| report("Added", &task, json))
        },
        Some(Commands::Update {
            task_id,
            id,
            status,
            title,
            description,
            json,
        }) => {
            let id = task_id.or(id).expect("clap enforces one id");
            mngr.update_task(id, status, title, description)
                .and_then(|task| report("Updated", &task, json))
        },
        Some(Commands::Show {
            task_id,
            id,
            kanban,
            json,
            status,
            search,
        }) => mngr.list_tasks(task_id.or(id), kanban, json, status, search),
        Some(Commands::Delete { task_id, id, json }) => {
            let id = task_id.or(id).expect("clap enforces one id");
            mngr.delete_task(id)
                .and_then(|task| report("Deleted", &task, json))
        },
        Some(Commands::Init) => mngr
            .init_tasklist()
            .map(|()| println!("Created tasklist {}", tasklist_path)),
        Some(Commands::Tui) => tui::run(mngr),
        None => {
            // Default: show tasks, but check if file exists first
            if !PathBuf::from(&tasklist_path).exists() {
                println!("No tasklist file found at: {}", tasklist_path);
                println!("\nGet started by creating it, or by adding your first task:");
                println!("  tsk init");
                println!("  tsk add \"My first task\"");
                println!("\nOr run in interactive mode:");
                println!("  tsk tui");
                println!("\nFor more options:");
                println!("  tsk --help");
                return;
            }
            mngr.list_tasks(None, args.kanban, false, None, None)
        },
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn report(action: &str, task: &Task, json: bool) -> Result<(), Error> {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(task).map_err(Error::other)?
        );
    } else {
        println!("{} #{}: {}", action, task.id, task.title);
    }
    Ok(())
}
