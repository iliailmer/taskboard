use crate::task::Status;
use clap::{Parser, Subcommand};
#[derive(Parser, Debug)]
#[command(
    version,
    name = "tasklist",
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
            value_name = "DESCRIPTION",
            help = "Task description",
            required_unless_present = "description",
            conflicts_with = "description"
        )]
        text: Option<String>,
        #[arg(short, long, help = "Task description (flag form)")]
        description: Option<String>,
    },
    #[command(about = "Update an existing task")]
    #[clap(visible_alias = "u")]
    Update {
        #[arg(short, long, help = "ID (index) of the task to update")]
        id: i32,
        #[arg(short, long, help = "New task status")]
        status: Status,
        #[arg(short, long, help = "New description")]
        description: Option<String>,
    },
    #[command(about = "View tasks")]
    #[clap(visible_alias = "ls")]
    #[clap(visible_alias = "list")]
    Show {
        #[arg(short, long, help = "Display tasks in Kanban board view")]
        kanban: bool,
    },
    #[command(about = "Delete task")]
    #[clap(visible_alias = "rm")]
    Delete {
        #[arg(short, long, help = "ID of task being deleted")]
        id: i32,
    },
    #[command(about = "Launch interactive TUI")]
    Tui,
}
