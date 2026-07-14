mod cli;
mod manager;
mod task;
mod tui;

use crate::cli::{Cli, Commands};
use crate::manager::Mngr;
use clap::Parser;

use std::fs;
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
        Some(Commands::Add { text, description }) => {
            let description = text.or(description).expect("clap enforces one description");
            mngr.add_task(description)
        },
        Some(Commands::Update {
            id,
            status,
            description,
        }) => mngr.update_task(id, status, description),
        Some(Commands::Show { kanban }) => mngr.list_tasks(kanban),
        Some(Commands::Delete { id }) => mngr.delete_task(id),
        Some(Commands::Tui) => tui::run(mngr),
        None => {
            // Default: show tasks, but check if file exists first
            if !PathBuf::from(&tasklist_path).exists() {
                println!("No tasklist file found at: {}", tasklist_path);
                println!("\nGet started by adding your first task:");
                println!("  tsk add \"My first task\"");
                println!("\nOr run in interactive mode:");
                println!("  tsk tui");
                println!("\nFor more options:");
                println!("  tsk --help");
                return;
            }
            mngr.list_tasks(args.kanban)
        },
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
