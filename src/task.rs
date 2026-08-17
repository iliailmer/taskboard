use clap::ValueEnum;
use serde::Serialize;
use std::fmt;
use std::io::{self, Write};
use tabled::Tabled;
#[derive(Debug, Clone, Copy, ValueEnum, Eq, Hash, PartialEq, Serialize, Tabled)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    #[value(name = "not_started", alias = "ns")]
    #[tabled(rename = "🚀 Not Started")]
    NotStarted,
    #[tabled(rename = "⏳ In Progress")]
    #[value(name = "in_progress", alias = "ip")]
    InProgress,
    #[value(name = "done", alias = "d")]
    #[tabled(rename = "✅ Done")]
    Done,
}

pub const SEP: &str = "\t";

#[allow(dead_code)]
impl Status {
    pub const DONE_LABEL: &'static str = "✅ Done";
    pub const IN_PROGRESS_LABEL: &'static str = "⏳ In Progress";
    pub const NOT_STARTED_LABEL: &'static str = "🚀 Not Started";

    pub fn from_str(s: &str) -> Self {
        match s {
            Self::DONE_LABEL => Status::Done,
            Self::IN_PROGRESS_LABEL => Status::InProgress,
            Self::NOT_STARTED_LABEL => Status::NotStarted,
            _ => Status::NotStarted,
        }
    }

    pub fn as_label(&self) -> &'static str {
        match self {
            Status::Done => Self::DONE_LABEL,
            Status::InProgress => Status::IN_PROGRESS_LABEL,
            Status::NotStarted => Status::NOT_STARTED_LABEL,
        }
    }
}
impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_label())
    }
}

#[derive(Debug, Serialize, Tabled)]
pub struct Task {
    pub id: i32,
    #[tabled(inline)]
    pub status: Status,
    pub title: String,
    pub description: String,
    pub date: String,
}

impl fmt::Display for Task {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} | {} | {} | {} | {}",
            self.id,
            self.date,
            self.status.as_label(),
            self.title,
            self.description,
        )
    }
}

impl Task {
    pub fn new(id: i32, status: Status, title: String, description: String, date: String) -> Task {
        Task {
            id,
            status,
            title,
            description,
            date,
        }
    }
    pub fn to_file_string(&self) -> String {
        format!(
            "{}{SEP}{}{SEP}{}{SEP}{}{SEP}{}",
            self.id,
            self.status.as_label(),
            self.title,
            Self::escape_description(&self.description),
            self.date
        )
    }

    pub fn from_file_line(line: &str) -> Option<Task> {
        let parts: Vec<&str> = line.split(SEP).collect();
        let id = parts.first()?.parse().ok()?;
        let status = Status::from_str(parts.get(1)?);

        match parts.as_slice() {
            [_, _, title, date] => Some(Task::new(
                id,
                status,
                (*title).to_string(),
                String::new(),
                (*date).to_string(),
            )),
            [_, _, title, description, date] => Some(Task::new(
                id,
                status,
                (*title).to_string(),
                Self::unescape_description(description),
                (*date).to_string(),
            )),
            _ => None,
        }
    }

    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writeln!(writer, "{}", self.to_file_string())
    }

    fn escape_description(description: &str) -> String {
        description.replace('\\', "\\\\").replace('\n', "\\n")
    }

    fn unescape_description(description: &str) -> String {
        let mut result = String::with_capacity(description.len());
        let mut chars = description.chars();

        while let Some(ch) = chars.next() {
            if ch != '\\' {
                result.push(ch);
                continue;
            }

            match chars.next() {
                Some('n') => result.push('\n'),
                Some('\\') => result.push('\\'),
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                },
                None => result.push('\\'),
            }
        }

        result
    }
}
