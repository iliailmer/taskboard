use crate::task::Status;
use clap::{Parser, Subcommand};
#[derive(Parser, Debug)]
#[command(
    version,
    name = "taskboard (tsk)",
    about = "A to-do list app for command line"
)]
pub struct Cli {
    #[arg(short, long, global = true, help = "Path to .tasklist file.")]
    pub file: Option<String>, // Path to custom tasklist file

    #[arg(
        short,
        long,
        global = true,
        help = "Show verbose output (file paths, etc.)"
    )]
    pub verbose: bool,

    #[arg(
        short,
        long,
        global = true,
        help = "Display tasks in Kanban board view"
    )]
    pub kanban: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(about = "Add a new task")]
    #[clap(visible_alias = "a")]
    Add {
        #[arg(
            value_name = "TITLE",
            help = "Task title",
            required_unless_present = "title",
            conflicts_with = "title"
        )]
        text: Option<String>,
        #[arg(short, long, help = "Task title (flag form)")]
        title: Option<String>,
        #[arg(short, long, help = "Optional longer description")]
        description: Option<String>,
        #[arg(long, help = "Print the new task as JSON")]
        json: bool,
    },
    #[command(about = "Update an existing task")]
    #[clap(visible_alias = "u")]
    Update {
        #[arg(short, long, help = "ID (index) of the task to update")]
        id: i32,
        #[arg(short, long, help = "New task status")]
        status: Option<Status>,
        #[arg(short, long, help = "New title")]
        title: Option<String>,
        #[arg(short, long, help = "New description")]
        description: Option<String>,
        #[arg(long, help = "Print the updated task as JSON")]
        json: bool,
    },
    #[command(about = "View tasks")]
    #[clap(visible_alias = "ls")]
    #[clap(visible_alias = "list")]
    Show {
        #[arg(short, long, help = "Filter by `id`")]
        id: Option<i32>,
        #[arg(short, long, help = "Filter by status")]
        status: Option<Status>,
        #[arg(long, help = "Search and return matching by substring")]
        search: Option<String>,
        #[arg(short, long, help = "Display tasks in Kanban board view")]
        kanban: bool,
        #[arg(long, help = "Print tasks as JSON")]
        json: bool,
    },
    #[command(about = "Delete task")]
    #[clap(visible_alias = "rm")]
    Delete {
        #[arg(short, long, help = "ID of task being deleted")]
        id: i32,
        #[arg(long, help = "Print the deleted task as JSON")]
        json: bool,
    },
    #[command(about = "Launch interactive TUI")]
    Tui,
}
