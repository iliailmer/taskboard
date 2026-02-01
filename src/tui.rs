use crate::manager::Mngr;
use crate::task::{Status, Task};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};
use std::collections::HashMap;
use std::io;

pub struct App {
    manager: Mngr,
    tasks: Vec<Task>,
    selected_column: usize, // 0 = NotStarted, 1 = InProgress, 2 = Done
    selected_row: usize,    // Index within the selected column
    mode: AppMode,
    input: String,
    error_message: Option<String>,
}

#[derive(PartialEq)]
enum AppMode {
    Normal,
    AddingTask,
    ConfirmDelete,
}

impl App {
    pub fn new(manager: Mngr) -> io::Result<App> {
        let tasks = manager.get_tasks()?;
        Ok(App {
            manager,
            tasks,
            selected_column: 0,
            selected_row: 0,
            mode: AppMode::Normal,
            input: String::new(),
            error_message: None,
        })
    }

    fn reload_tasks(&mut self) -> io::Result<()> {
        let old_task_id = self.get_selected_task().map(|t| t.id);
        self.tasks = self.manager.get_tasks()?;

        // Try to maintain selection on the same task
        if let Some(task_id) = old_task_id {
            let grouped = self.get_grouped_tasks();
            let columns = [Status::NotStarted, Status::InProgress, Status::Done];

            for (col_idx, status) in columns.iter().enumerate() {
                let tasks = grouped.get(status);
                match tasks {
                    Some(value) => {
                        if let Some(row_idx) = value.iter().position(|t| t.id == task_id) {
                            self.selected_column = col_idx;
                            self.selected_row = row_idx;
                            return Ok(());
                        }
                    },
                    None => continue,
                };
            }
        }

        // If we couldn't find the task, reset to a valid position
        self.ensure_valid_selection();
        Ok(())
    }

    fn get_grouped_tasks(&self) -> HashMap<Status, Vec<&Task>> {
        let mut grouped: HashMap<Status, Vec<&Task>> = HashMap::new();
        for task in &self.tasks {
            grouped.entry(task.status).or_default().push(task);
        }
        grouped
    }

    fn get_selected_task(&self) -> Option<&Task> {
        let grouped = self.get_grouped_tasks();
        let columns = [Status::NotStarted, Status::InProgress, Status::Done];

        if self.selected_column >= columns.len() {
            return None;
        }

        let status = columns[self.selected_column];
        grouped
            .get(&status)
            .and_then(|tasks| tasks.get(self.selected_row).copied())
    }

    fn ensure_valid_selection(&mut self) {
        let grouped = self.get_grouped_tasks();
        let columns = [Status::NotStarted, Status::InProgress, Status::Done];

        // Make sure selected_column is valid
        let mut current_column = self.selected_column;
        if current_column >= columns.len() {
            current_column = 0;
        }

        // Make sure selected_row is valid for the current column
        let status = columns[current_column];
        let column_len = grouped.get(&status).map(|v| v.len()).unwrap_or(0);

        if column_len == 0 {
            // Current column is empty, try to find a non-empty column
            for (idx, col_status) in columns.iter().enumerate() {
                if grouped.get(col_status).map(|v| v.len()).unwrap_or(0) > 0 {
                    self.selected_column = idx;
                    self.selected_row = 0;
                    return;
                }
            }
            // All columns are empty
            self.selected_column = current_column;
            self.selected_row = 0;
        } else {
            self.selected_column = current_column;
            if self.selected_row >= column_len {
                self.selected_row = column_len - 1;
            }
        }
    }

    fn next_in_column(&mut self) {
        let grouped = self.get_grouped_tasks();
        let columns = [Status::NotStarted, Status::InProgress, Status::Done];
        let status = columns[self.selected_column];
        if let Some(tasks) = grouped.get(&status)
            && !tasks.is_empty()
        {
            self.selected_row = (self.selected_row + 1) % tasks.len();
        }
    }

    fn previous_in_column(&mut self) {
        let grouped = self.get_grouped_tasks();
        let columns = [Status::NotStarted, Status::InProgress, Status::Done];
        let status = columns[self.selected_column];

        if let Some(tasks) = grouped.get(&status)
            && !tasks.is_empty()
        {
            if self.selected_row == 0 {
                self.selected_row = tasks.len() - 1;
            } else {
                self.selected_row -= 1;
            }
        }
    }

    fn next_column(&mut self) {
        self.selected_column = (self.selected_column + 1) % 3;
        self.ensure_valid_selection();
    }

    fn previous_column(&mut self) {
        if self.selected_column == 0 {
            self.selected_column = 2;
        } else {
            self.selected_column -= 1;
        }
        self.ensure_valid_selection();
    }

    fn update_task_status(&mut self, status: Status) -> io::Result<()> {
        if let Some(task) = self.get_selected_task() {
            let task_id = task.id;
            self.manager
                .update_task(task_id, status, None)
                .map_err(io::Error::other)?;
            self.reload_tasks()?;
        }
        Ok(())
    }

    fn delete_current_task(&mut self) -> io::Result<()> {
        if let Some(task) = self.get_selected_task() {
            let task_id = task.id;
            self.manager
                .delete_task(task_id)
                .map_err(io::Error::other)?;
            self.reload_tasks()?;
        }
        Ok(())
    }

    fn add_task(&mut self) -> io::Result<()> {
        if !self.input.is_empty() {
            self.manager
                .add_task(self.input.clone())
                .map_err(io::Error::other)?;
            self.input.clear();
            self.mode = AppMode::Normal;
            self.reload_tasks()?;
            // Select the newly added task (goes to NotStarted column)
            self.selected_column = 0;
            self.ensure_valid_selection();
            // Move to the last task in NotStarted column
            let grouped = self.get_grouped_tasks();
            if let Some(tasks) = grouped.get(&Status::NotStarted)
                && !tasks.is_empty()
            {
                self.selected_row = tasks.len() - 1;
            }
        }
        Ok(())
    }
}

pub fn run(manager: Mngr) -> io::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run
    let app = App::new(manager)?;
    let res = run_app(&mut terminal, app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {}", err);
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        if let Event::Key(key) = event::read()? {
            match app.mode {
                AppMode::Normal => match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(());
                    },
                    KeyCode::Down | KeyCode::Char('j') => app.next_in_column(),
                    KeyCode::Up | KeyCode::Char('k') => app.previous_in_column(),
                    KeyCode::Right | KeyCode::Char('l') => app.next_column(),
                    KeyCode::Left | KeyCode::Char('h') => app.previous_column(),
                    KeyCode::Char('n') => {
                        app.mode = AppMode::AddingTask;
                        app.input.clear();
                        app.error_message = None;
                    },
                    KeyCode::Char('d') => {
                        if app.get_selected_task().is_some() {
                            app.mode = AppMode::ConfirmDelete;
                            app.error_message = None;
                        }
                    },
                    KeyCode::Char('1') => {
                        if let Err(e) = app.update_task_status(Status::NotStarted) {
                            app.error_message = Some(format!("Error: {}", e));
                        }
                    },
                    KeyCode::Char('2') => {
                        if let Err(e) = app.update_task_status(Status::InProgress) {
                            app.error_message = Some(format!("Error: {}", e));
                        }
                    },
                    KeyCode::Char('3') => {
                        if let Err(e) = app.update_task_status(Status::Done) {
                            app.error_message = Some(format!("Error: {}", e));
                        }
                    },
                    KeyCode::Char('r') => {
                        if let Err(e) = app.reload_tasks() {
                            app.error_message = Some(format!("Error reloading: {}", e));
                        } else {
                            app.error_message = None;
                        }
                    },
                    _ => {},
                },
                AppMode::AddingTask => match key.code {
                    KeyCode::Enter => {
                        if let Err(e) = app.add_task() {
                            app.error_message = Some(format!("Error: {}", e));
                            app.mode = AppMode::Normal;
                        }
                    },
                    KeyCode::Esc => {
                        app.mode = AppMode::Normal;
                        app.input.clear();
                        app.error_message = None;
                    },
                    KeyCode::Char(c) => {
                        app.input.push(c);
                    },
                    KeyCode::Backspace => {
                        app.input.pop();
                    },
                    _ => {},
                },
                AppMode::ConfirmDelete => match key.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        if let Err(e) = app.delete_current_task() {
                            app.error_message = Some(format!("Error: {}", e));
                        }
                        app.mode = AppMode::Normal;
                    },
                    KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                        app.mode = AppMode::Normal;
                        app.error_message = None;
                    },
                    _ => {},
                },
            }
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Min(10),   // Kanban board
            Constraint::Length(6), // Help
        ])
        .split(f.area());

    // Title
    let title = Paragraph::new(vec![
        Line::from(vec![Span::styled(
            "TaskBoard - Kanban View",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            format!("Total tasks: {}", app.tasks.len()),
            Style::default().fg(Color::DarkGray),
        )]),
    ])
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    // Kanban board (3 columns)
    let board_area = chunks[1];
    render_kanban_board(f, app, board_area);

    // Input or Help
    match app.mode {
        AppMode::AddingTask => {
            let input = Paragraph::new(app.input.as_str())
                .style(Style::default().fg(Color::Yellow))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("New Task Description"),
                );
            f.render_widget(input, chunks[2]);
        },
        AppMode::ConfirmDelete => {
            let confirm = Paragraph::new("Delete this task? (y/n)")
                .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                .block(Block::default().borders(Borders::ALL).title("Confirm"));
            f.render_widget(confirm, chunks[2]);
        },
        _ => {
            let help_lines = vec![
                Line::from(vec![
                    Span::styled("Navigate: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw("↑↓/jk (tasks) ←→/hl (columns) | "),
                    Span::styled("Actions: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw("n (new) d (delete) r (reload)"),
                ]),
                Line::from(vec![
                    Span::styled("Move task: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::styled("1", Style::default().fg(Color::Yellow)),
                    Span::raw(" Backlog "),
                    Span::styled("2", Style::default().fg(Color::Blue)),
                    Span::raw(" In Progress "),
                    Span::styled("3", Style::default().fg(Color::Green)),
                    Span::raw(" Done | "),
                    Span::styled("Exit: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw("q/Ctrl+C"),
                ]),
            ];

            let help_widget = if let Some(error) = &app.error_message {
                let mut lines = vec![Line::from(vec![Span::styled(
                    error,
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )])];
                lines.extend(help_lines);
                Paragraph::new(lines)
                    .block(Block::default().borders(Borders::ALL).title("Help"))
                    .wrap(Wrap { trim: true })
            } else {
                Paragraph::new(help_lines)
                    .block(Block::default().borders(Borders::ALL).title("Help"))
                    .wrap(Wrap { trim: true })
            };

            f.render_widget(help_widget, chunks[2]);
        },
    }
}

fn render_kanban_board(f: &mut Frame, app: &App, area: Rect) {
    let columns_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(area);

    let grouped = app.get_grouped_tasks();
    let columns = [
        (Status::NotStarted, "🚀 BACKLOG", Color::Yellow, 0),
        (Status::InProgress, "⏳ IN PROGRESS", Color::Blue, 1),
        (Status::Done, "✅ DONE", Color::Green, 2),
    ];

    for (status, title, color, col_idx) in columns {
        let tasks = grouped.get(&status).cloned().unwrap_or_default();

        let items: Vec<ListItem> = tasks
            .iter()
            .enumerate()
            .map(|(idx, task)| {
                let is_selected = app.selected_column == col_idx && app.selected_row == idx;

                let id_text = format!("[{}]", task.id);
                let date_text = if !task.date.is_empty() {
                    format!(" {}", task.date)
                } else {
                    String::new()
                };

                let content = vec![
                    Line::from(vec![
                        Span::styled(id_text.clone(), Style::default().fg(Color::DarkGray)),
                        Span::raw(" "),
                        Span::styled(
                            task.description.clone(),
                            if is_selected {
                                Style::default().add_modifier(Modifier::BOLD)
                            } else {
                                Style::default()
                            },
                        ),
                    ]),
                    Line::from(vec![Span::styled(
                        date_text.clone(),
                        Style::default().fg(Color::DarkGray),
                    )]),
                ];

                let item = ListItem::new(content);
                if is_selected {
                    item.style(Style::default().bg(Color::DarkGray))
                } else {
                    item
                }
            })
            .collect();

        let is_active_column = app.selected_column == col_idx;
        let block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(
                title,
                Style::default()
                    .fg(color)
                    .add_modifier(if is_active_column {
                        Modifier::BOLD | Modifier::UNDERLINED
                    } else {
                        Modifier::BOLD
                    }),
            ))
            .border_style(if is_active_column {
                Style::default().fg(color)
            } else {
                Style::default()
            });

        let list = List::new(items).block(block);
        f.render_widget(list, columns_layout[col_idx]);
    }
}
