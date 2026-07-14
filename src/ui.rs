use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, Wrap},
};

use crate::{
    app::{App, Modal, Screen, TaskStatus},
    consts::{
        HELP_BUILD, HELP_BUILD_EDIT, HELP_CONTAINERS, HELP_IMAGES, HELP_LOGS, HELP_MODAL,
        HELP_TASKS, SIDEBAR_ITEMS,
    },
};
pub fn ui(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(5),
        ])
        .split(frame.area());

    render_header(frame, chunks[0], app);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(20), Constraint::Min(20)])
        .split(chunks[1]);

    render_sidebar(frame, body[0], app);
    render_main(frame, body[1], app);
    render_footer(frame, chunks[2], app);

    if app.modal.is_some() {
        render_modal(frame, app);
    }
}

fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let title = Line::from(vec![
        Span::styled(
            "nd Docker Manager",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(
            " | Containers: {} | Images: {} | {}",
            app.containers.len(),
            app.images.len(),
            app.status
        )),
    ]);

    let widget = Paragraph::new(title)
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Left);

    frame.render_widget(widget, area);
}

fn render_sidebar(frame: &mut Frame, area: Rect, app: &App) {
    let items = SIDEBAR_ITEMS
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let style = if index == app.screen.index() {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(Line::from(*item)).style(style)
        })
        .collect::<Vec<_>>();

    let list = List::new(items).block(Block::default().title("Screens").borders(Borders::ALL));
    frame.render_widget(list, area);
}

fn render_main(frame: &mut Frame, area: Rect, app: &App) {
    match app.screen {
        Screen::Containers => render_containers(frame, area, app),
        Screen::Images => render_images(frame, area, app),
        Screen::Build => render_build(frame, area, app),
        Screen::Tasks => render_tasks(frame, area, app),
        Screen::Logs => render_logs(frame, area, app),
    }
}

fn render_containers(frame: &mut Frame, area: Rect, app: &App) {
    if app.containers.is_empty() {
        render_empty(
            frame,
            area,
            "Containers",
            "No containers found. Press r to refresh.",
        );
        return;
    }

    let rows = app
        .containers
        .iter()
        .enumerate()
        .map(|(index, container)| {
            let style = selected_style(index == app.selected_container);
            Row::new(vec![
                Cell::from(container.id.clone()),
                Cell::from(container.image.clone()),
                Cell::from(container.status.clone()),
                Cell::from(container.names.clone()),
            ])
            .style(style)
        })
        .collect::<Vec<_>>();

    let table = Table::new(
        rows,
        [
            Constraint::Length(14),
            Constraint::Percentage(30),
            Constraint::Percentage(35),
            Constraint::Percentage(25),
        ],
    )
    .header(
        Row::new(vec!["ID", "Image", "Status", "Name"]).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(Block::default().title("Containers").borders(Borders::ALL));

    frame.render_widget(table, area);
}

fn render_images(frame: &mut Frame, area: Rect, app: &App) {
    if app.images.is_empty() {
        render_empty(
            frame,
            area,
            "Images",
            "No images found. Press r to refresh.",
        );
        return;
    }

    let rows = app
        .images
        .iter()
        .enumerate()
        .map(|(index, image)| {
            let style = selected_style(index == app.selected_image);
            Row::new(vec![
                Cell::from(image.repository.clone()),
                Cell::from(image.tag.clone()),
                Cell::from(image.id.clone()),
                Cell::from(image.size.clone()),
            ])
            .style(style)
        })
        .collect::<Vec<_>>();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(35),
            Constraint::Percentage(20),
            Constraint::Percentage(25),
            Constraint::Percentage(20),
        ],
    )
    .header(
        Row::new(vec!["Repository", "Tag", "ID", "Size"]).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(Block::default().title("Images").borders(Borders::ALL));

    frame.render_widget(table, area);
}

const BUILD_WIDE_BREAKPOINT: u16 = 80;

fn render_build(frame: &mut Frame, area: Rect, app: &App) {
    let (direction, constraints) = if area.width >= BUILD_WIDE_BREAKPOINT {
        (
            Direction::Horizontal,
            [Constraint::Percentage(44), Constraint::Percentage(56)],
        )
    } else {
        let target_height = (area.height.saturating_mul(2) / 5)
            .max(4)
            .min(area.height.saturating_sub(3));
        (
            Direction::Vertical,
            [Constraint::Length(target_height), Constraint::Min(3)],
        )
    };

    let chunks = Layout::default()
        .direction(direction)
        .constraints(constraints)
        .split(area);

    render_build_targets(frame, chunks[0], app);
    render_build_plan(frame, chunks[1], app);
}

fn render_build_targets(frame: &mut Frame, area: Rect, app: &App) {
    if app.configs.is_empty() {
        render_empty(
            frame,
            area,
            "Build targets · 0",
            "No configured builds. Use the default context or edit an override.",
        );
        return;
    }

    let visible =
        visible_build_config_range(app.configs.len(), app.selected_build_tag, area.height);
    let start = visible.start;
    let rows = app.configs[visible]
        .iter()
        .enumerate()
        .map(|(offset, config)| {
            let selected = start + offset == app.selected_build_tag;
            let marker = if selected { "›" } else { " " };
            let tag = config.tag.as_deref().unwrap_or("<no tag resolved>");
            let version = config.latest_version.as_deref().unwrap_or("<not resolved>");

            Row::new(vec![
                Cell::from(format!("{marker} {tag}")),
                Cell::from(config.context_or_default()),
                Cell::from(version),
            ])
            .style(selected_style(selected))
        })
        .collect::<Vec<_>>();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(46),
            Constraint::Percentage(34),
            Constraint::Min(8),
        ],
    )
    .header(
        Row::new(vec!["Tag", "Context", "Latest"]).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(
        Block::default()
            .title(format!("Build targets · {}", app.configs.len()))
            .borders(Borders::ALL),
    );

    frame.render_widget(table, area);
}

fn render_build_plan(frame: &mut Frame, area: Rect, app: &App) {
    let title = if app.editing_context {
        "Edit context"
    } else {
        "Build plan"
    };
    let label_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let value_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let selected_config = app.selected_config();
    let config_position = selected_config
        .map(|_| format!("{}/{}", app.selected_build_tag + 1, app.configs.len()))
        .unwrap_or_else(|| "none".to_string());
    let configured_context = selected_config
        .map(|config| config.context_or_default())
        .unwrap_or("<none>");
    let tag = selected_config
        .and_then(|config| config.tag.as_deref())
        .unwrap_or("<no tag resolved>");
    let version = selected_config
        .and_then(|config| config.latest_version.as_deref())
        .unwrap_or("<not resolved>");

    let target = Line::from(vec![
        Span::styled("Config · ", label_style),
        Span::styled(config_position, value_style),
        Span::styled(" · Tag · ", label_style),
        Span::styled(tag, value_style),
    ]);
    let configured = Line::from(vec![
        Span::styled("Configured context · ", label_style),
        Span::styled(configured_context, value_style),
    ]);
    let latest = Line::from(vec![
        Span::styled("Latest · ", label_style),
        Span::styled(version, value_style),
    ]);

    let text = if app.editing_context {
        let draft = if app.context_draft().is_empty() {
            "<empty — save to clear override>"
        } else {
            app.context_draft()
        };
        vec![
            Line::from(Span::styled("Draft context", label_style)),
            Line::from(Span::styled(draft, value_style)),
            Line::from("Enter save · Esc cancel"),
            Line::from("Backspace delete · Ctrl-U clear"),
            Line::from(""),
            Line::from(vec![
                Span::styled("Current · ", label_style),
                Span::styled(app.effective_build_context(), value_style),
            ]),
            Line::from(format!("Source · {}", app.build_context_source().label())),
            target,
            configured,
            latest,
        ]
    } else {
        vec![
            Line::from(Span::styled("Effective context", label_style)),
            Line::from(Span::styled(app.effective_build_context(), value_style)),
            Line::from(format!("Source · {}", app.build_context_source().label())),
            Line::from("↑/↓ select target · Enter build"),
            Line::from("e edit context · p prune builder"),
            Line::from(""),
            target,
            configured,
            latest,
        ]
    };

    let widget = Paragraph::new(text)
        .block(Block::default().title(title).borders(Borders::ALL))
        .wrap(Wrap { trim: false });

    frame.render_widget(widget, area);
}

const TASKS_WIDE_BREAKPOINT: u16 = 80;

fn render_tasks(frame: &mut Frame, area: Rect, app: &App) {
    if app.tasks.is_empty() {
        render_empty(
            frame,
            area,
            "Tasks",
            "No tasks. Start a build or push; it appears here automatically.",
        );
        return;
    }

    let (direction, constraints) = if area.width >= TASKS_WIDE_BREAKPOINT {
        (
            Direction::Horizontal,
            [Constraint::Percentage(38), Constraint::Percentage(62)],
        )
    } else {
        let list_height = (area.height.saturating_mul(2) / 5)
            .max(4)
            .min(area.height.saturating_sub(3));
        (
            Direction::Vertical,
            [Constraint::Length(list_height), Constraint::Min(3)],
        )
    };

    let chunks = Layout::default()
        .direction(direction)
        .constraints(constraints)
        .split(area);

    render_task_list(frame, chunks[0], app);
    render_task_output(frame, chunks[1], app);
}

fn render_task_list(frame: &mut Frame, area: Rect, app: &App) {
    if app.tasks.is_empty() {
        render_empty(
            frame,
            area,
            "Tasks",
            "No tasks. Start a build or push; it appears here automatically.",
        );
        return;
    }

    let visible = visible_task_range(app.tasks.len(), app.selected_task, area.height);
    let start = visible.start;
    let rows = app.tasks[visible]
        .iter()
        .enumerate()
        .map(|(offset, task)| {
            let selected = start + offset == app.selected_task;
            let marker = if selected { "›" } else { " " };
            let row_style = if selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::LightYellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(format!("{marker} {}", task.status.label()))
                    .style(Style::default().fg(task_status_color(task.status))),
                Cell::from(format!("{} · {}", task.kind.label(), task.title)),
            ])
            .style(row_style)
        })
        .collect::<Vec<_>>();

    let table = Table::new(rows, [Constraint::Length(10), Constraint::Min(8)])
        .header(
            Row::new(vec!["State", "Task"]).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(
            Block::default()
                .title(task_list_title(app))
                .borders(Borders::ALL),
        );

    frame.render_widget(table, area);
}

fn render_task_output(frame: &mut Frame, area: Rect, app: &App) {
    let Some(task) = app.selected_task() else {
        let widget = Paragraph::new("No task selected.")
            .block(Block::default().title("Output").borders(Borders::ALL))
            .wrap(Wrap { trim: false });
        frame.render_widget(widget, area);
        return;
    };

    let title = Line::from(vec![
        Span::raw("Output · "),
        Span::styled(task.kind.label(), Style::default().fg(Color::Yellow)),
        Span::raw(" · "),
        Span::styled(
            task.title.as_str(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" ["),
        Span::styled(
            task.status.label(),
            Style::default().fg(task_status_color(task.status)),
        ),
        Span::raw(format!("] · {} lines", task.lines.len())),
    ]);

    let text = if task.lines.is_empty() {
        Text::from(if task.is_running() {
            "Waiting for output..."
        } else {
            "No output captured."
        })
    } else {
        task.lines
            .iter()
            .map(|line| Line::from(line.as_str()))
            .collect()
    };

    let block = Block::default().title(title).borders(Borders::ALL);
    let inner = block.inner(area);
    let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });
    let scroll = output_tail_scroll(paragraph.line_count(inner.width), inner.height as usize);
    let widget = paragraph.block(block).scroll((scroll, 0));

    frame.render_widget(widget, area);
}

fn task_status_color(status: TaskStatus) -> Color {
    match status {
        TaskStatus::Running => Color::Cyan,
        TaskStatus::Succeeded => Color::Green,
        TaskStatus::Failed => Color::Red,
    }
}

fn task_state_counts(app: &App) -> (usize, usize, usize) {
    app.tasks
        .iter()
        .fold((0, 0, 0), |(running, succeeded, failed), task| {
            match task.status {
                TaskStatus::Running => (running + 1, succeeded, failed),
                TaskStatus::Succeeded => (running, succeeded + 1, failed),
                TaskStatus::Failed => (running, succeeded, failed + 1),
            }
        })
}

fn task_list_title(app: &App) -> Line<'static> {
    let (running, succeeded, failed) = task_state_counts(app);

    Line::from(vec![
        Span::raw(format!("Tasks {} · ", app.tasks.len())),
        Span::styled(format!("●{running}"), Style::default().fg(Color::Cyan)),
        Span::raw(" "),
        Span::styled(format!("✓{succeeded}"), Style::default().fg(Color::Green)),
        Span::raw(" "),
        Span::styled(format!("×{failed}"), Style::default().fg(Color::Red)),
    ])
}

fn visible_build_config_range(
    config_count: usize,
    selected_config: usize,
    area_height: u16,
) -> std::ops::Range<usize> {
    visible_selected_range(
        config_count,
        selected_config,
        area_height.saturating_sub(3) as usize,
    )
}

fn visible_task_range(
    task_count: usize,
    selected_task: usize,
    area_height: u16,
) -> std::ops::Range<usize> {
    visible_selected_range(
        task_count,
        selected_task,
        area_height.saturating_sub(3) as usize,
    )
}

fn visible_selected_range(
    item_count: usize,
    selected_item: usize,
    capacity: usize,
) -> std::ops::Range<usize> {
    if item_count <= capacity || capacity == 0 {
        return 0..item_count.min(capacity);
    }

    let selected = selected_item.min(item_count - 1);
    let start = selected
        .saturating_sub(capacity / 2)
        .min(item_count - capacity);
    start..start + capacity
}

fn output_tail_scroll(line_count: usize, viewport_height: usize) -> u16 {
    line_count
        .saturating_sub(viewport_height)
        .min(u16::MAX as usize) as u16
}

fn render_logs(frame: &mut Frame, area: Rect, app: &App) {
    // Show only the lines that fit in the visible area, tailing the newest.
    let height = area.height.saturating_sub(2) as usize;
    let start = app.logs.len().saturating_sub(height);
    let visible = &app.logs[start..];

    // Build a Text from the line slice directly instead of joining into a
    // fresh String every frame (avoids an O(n) allocation per render).
    let text = if visible.is_empty() {
        Text::from("No logs yet.")
    } else {
        visible
            .iter()
            .map(|line| Line::from(line.as_str()))
            .collect()
    };

    let logs = Paragraph::new(text)
        .block(Block::default().title("Logs").borders(Borders::ALL))
        .wrap(Wrap { trim: false });

    frame.render_widget(logs, area);
}

fn render_footer(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    let details = match app.screen {
        Screen::Containers => Text::from(
            app.selected_container()
                .map(|container| {
                    format!(
                        "Container {}\nImage: {}\nCommand: {}",
                        container.id, container.image, container.command
                    )
                })
                .unwrap_or_else(|| "No container selected.".to_string()),
        ),
        Screen::Images => Text::from(
            app.selected_image()
                .map(|image| {
                    format!(
                        "Image {}:{}\nID: {}\nSize: {}",
                        image.repository, image.tag, image.id, image.size
                    )
                })
                .unwrap_or_else(|| "No image selected.".to_string()),
        ),
        Screen::Build => {
            let mut lines = if app.editing_context {
                vec![
                    Line::from(format!(
                        "Editing · {}",
                        if app.context_draft().is_empty() {
                            "<empty>"
                        } else {
                            app.context_draft()
                        }
                    )),
                    Line::from(format!(
                        "Current · {} [{}]",
                        app.effective_build_context(),
                        app.build_context_source().label()
                    )),
                ]
            } else {
                vec![Line::from(format!(
                    "Context · {} [{}]",
                    app.effective_build_context(),
                    app.build_context_source().label()
                ))]
            };

            if let Some(config) = app.selected_config() {
                lines.push(Line::from(format!(
                    "Tag · {}",
                    config.tag.as_deref().unwrap_or("<no tag resolved>")
                )));
                if !app.editing_context {
                    lines.push(Line::from(format!(
                        "Version · {}",
                        config.latest_version.as_deref().unwrap_or("<not resolved>")
                    )));
                }
            } else {
                lines.push(Line::from("Tag · <none>"));
            }

            Text::from(lines)
        }
        Screen::Tasks => {
            let running = app.tasks.iter().filter(|task| task.is_running()).count();
            let total = app.tasks.len();

            if let Some(task) = app.selected_task() {
                Text::from(vec![
                    Line::from(format!("Tasks · {running} running · {total} total")),
                    Line::from(vec![
                        Span::raw(format!(
                            "{}/{} {} ",
                            app.selected_task + 1,
                            total,
                            task.kind.label()
                        )),
                        Span::styled(
                            task.status.label(),
                            Style::default().fg(task_status_color(task.status)),
                        ),
                        Span::raw(format!(" · {} lines", task.lines.len())),
                    ]),
                    Line::from(task.title.as_str()),
                ])
            } else {
                Text::from(vec![
                    Line::from(format!("Tasks · {running} running · {total} total")),
                    Line::from("No task selected."),
                ])
            }
        }
        Screen::Logs => Text::from(format!("{} log lines", app.logs.len())),
    };

    let help = match app.screen {
        Screen::Containers => HELP_CONTAINERS,
        Screen::Images => HELP_IMAGES,
        Screen::Build if app.editing_context => HELP_BUILD_EDIT,
        Screen::Build => HELP_BUILD,
        Screen::Tasks => HELP_TASKS,
        Screen::Logs => HELP_LOGS,
    };

    frame.render_widget(
        Paragraph::new(details)
            .block(Block::default().title("Details").borders(Borders::ALL))
            .wrap(Wrap { trim: true }),
        chunks[0],
    );

    frame.render_widget(
        Paragraph::new(help)
            .block(Block::default().title("Help").borders(Borders::ALL))
            .wrap(Wrap { trim: true }),
        chunks[1],
    );
}

fn render_modal(frame: &mut Frame, app: &App) {
    let Some(modal) = app.modal else {
        return;
    };

    let area = centered_rect(60, 30, frame.area());
    frame.render_widget(Clear, area);

    let (title, body, help) = match modal {
        Modal::RemoveContainer => {
            let target = app
                .selected_container()
                .map(|container| format!("{} ({})", container.names, container.id))
                .unwrap_or_else(|| "selected container".to_string());
            ("Remove container", format!("Remove {target}?"), HELP_MODAL)
        }
        Modal::PushImage => {
            let target = app
                .selected_image()
                .map(|image| format!("{}:{} ({})", image.repository, image.tag, image.id))
                .unwrap_or_else(|| "selected image".to_string());
            (
                "Push image",
                format!("Push {target} to registry?"),
                HELP_MODAL,
            )
        }
        Modal::RemoveImage => {
            let target = app
                .selected_image()
                .map(|image| format!("{}:{} ({})", image.repository, image.tag, image.id))
                .unwrap_or_else(|| "selected image".to_string());
            ("Remove image", format!("Remove {target}?"), HELP_MODAL)
        }
        Modal::ImagePrune => (
            "Prune images",
            "Remove unused dangling images?".to_string(),
            HELP_MODAL,
        ),
        Modal::BuilderPrune => (
            "Prune builder",
            "Remove builder cache?".to_string(),
            HELP_MODAL,
        ),
        Modal::SystemPrune => (
            "System prune",
            "Remove unused Docker data?".to_string(),
            HELP_MODAL,
        ),
        Modal::Quit => {
            let running = app.tasks.iter().filter(|task| task.is_running()).count();
            let body = match running {
                0 => "Quit?".to_string(),
                1 => "A task is still running. Quit anyway?".to_string(),
                n => format!("{n} tasks are still running. Quit anyway?"),
            };
            ("Quit", body, HELP_MODAL)
        }
    };

    let modal = Paragraph::new(format!("{body}\n\n{help}"))
        .block(Block::default().title(title).borders(Borders::ALL))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    frame.render_widget(modal, area);
}

fn render_empty(frame: &mut Frame, area: Rect, title: &str, message: &str) {
    let widget = Paragraph::new(message)
        .block(Block::default().title(title).borders(Borders::ALL))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    frame.render_widget(widget, area);
}

fn selected_style(selected: bool) -> Style {
    if selected {
        Style::default().fg(Color::Black).bg(Color::LightYellow)
    } else {
        Style::default()
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

    use super::*;
    use crate::{
        app::{Task, TaskKind},
        config::Config,
    };

    fn task(kind: TaskKind, title: &str, status: TaskStatus, lines: &[&str]) -> Task {
        let mut task = Task::new(kind, title.to_string());
        task.status = status;
        task.lines = lines.iter().map(|line| (*line).to_string()).collect();
        task
    }

    fn sample_app() -> App {
        let mut app = App::new();
        app.tasks = vec![
            task(TaskKind::Build, "image:latest", TaskStatus::Running, &[]),
            task(
                TaskKind::Push,
                "registry/image:v1",
                TaskStatus::Succeeded,
                &[],
            ),
            task(TaskKind::Build, "broken-image", TaskStatus::Failed, &[]),
        ];
        app
    }

    fn build_config(context: Option<&str>, tag: &str) -> Config {
        Config {
            context: context.map(str::to_string),
            tag: Some(tag.to_string()),
            latest_version: Some("1.2.3".to_string()),
            ..Config::default()
        }
    }

    fn render_tasks_buffer(width: u16, height: u16, app: &App) -> Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render_tasks(frame, frame.area(), app))
            .unwrap();
        terminal.backend().buffer().clone()
    }

    fn render_build_buffer(width: u16, height: u16, app: &App) -> Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render_build(frame, frame.area(), app))
            .unwrap();
        terminal.backend().buffer().clone()
    }

    fn render_build_targets_buffer(width: u16, height: u16, app: &App) -> Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render_build_targets(frame, frame.area(), app))
            .unwrap();
        terminal.backend().buffer().clone()
    }

    fn render_app_buffer(width: u16, height: u16, app: &App) -> Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| ui(frame, app)).unwrap();
        terminal.backend().buffer().clone()
    }

    fn render_task_list_buffer(width: u16, height: u16, app: &App) -> Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render_task_list(frame, frame.area(), app))
            .unwrap();
        terminal.backend().buffer().clone()
    }

    fn render_task_output_buffer(width: u16, height: u16, app: &App) -> Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render_task_output(frame, frame.area(), app))
            .unwrap();
        terminal.backend().buffer().clone()
    }

    fn buffer_lines(buffer: &Buffer) -> Vec<String> {
        (0..buffer.area.height)
            .map(|y| {
                let mut line = String::new();
                for x in 0..buffer.area.width {
                    line.push_str(buffer.cell((x, y)).unwrap().symbol());
                }
                line
            })
            .collect()
    }

    #[test]
    fn build_layout_switches_between_wide_and_narrow() {
        let mut app = App::new();
        app.configs = vec![
            build_config(Some("./app"), "example:app"),
            build_config(Some("./worker"), "example:worker"),
        ];

        let wide = buffer_lines(&render_build_buffer(100, 16, &app));
        assert!(wide[0].contains("Build targets · 2"));
        assert!(wide[0].contains("Build plan"));

        let narrow = buffer_lines(&render_build_buffer(60, 16, &app));
        let plan_row = narrow
            .iter()
            .position(|line| line.contains("Build plan"))
            .unwrap();
        assert!(narrow[0].contains("Build targets · 2"));
        assert!(plan_row > 0);
    }

    #[test]
    fn build_screen_keeps_the_default_plan_available_without_config() {
        let mut app = App::new();
        app.screen = Screen::Build;

        let rendered = buffer_lines(&render_app_buffer(110, 28, &app)).join("\n");
        assert!(rendered.contains("Build targets · 0"));
        assert!(rendered.contains("No configured builds."));
        assert!(rendered.contains("Build plan"));
        assert!(rendered.contains("Effective context"));
        assert!(rendered.contains("Source · default"));
        assert!(rendered.contains("Config · none"));
        assert!(rendered.contains("Enter build"));
        assert!(rendered.contains("Context · . [default]"));
    }

    #[test]
    fn build_plan_separates_manual_and_configured_contexts() {
        let mut app = App::new();
        app.configs = vec![build_config(Some(" ./configured "), "example:app")];
        app.input = " ./manual path ".to_string();

        let rendered = buffer_lines(&render_build_buffer(100, 18, &app)).join("\n");
        assert!(rendered.contains("Effective context"));
        assert!(rendered.contains("./manual path"));
        assert!(rendered.contains("Source · manual"));
        assert!(rendered.contains("Configured context"));
        assert!(rendered.contains("./configured"));
        assert!(rendered.contains("Config · 1/1"));
        assert!(rendered.contains("example:app"));
    }

    #[test]
    fn selected_build_target_stays_inside_the_visible_range() {
        let mut app = App::new();
        app.configs = (0..10)
            .map(|index| build_config(Some("./context"), &format!("example:{index}")))
            .collect();
        app.selected_build_tag = 9;

        assert_eq!(visible_build_config_range(10, 9, 7), 6..10);

        let buffer = render_build_targets_buffer(50, 7, &app);
        let rendered = buffer_lines(&buffer).join("\n");
        assert!(rendered.contains("example:9"));
        assert!(!rendered.contains("example:0"));
        assert_eq!(buffer.cell((1, 5)).unwrap().bg, Color::LightYellow);
    }

    #[test]
    fn build_plan_wraps_edit_drafts_and_keeps_edit_controls_visible() {
        let mut app = App::new();
        app.configs = vec![build_config(Some("./configured"), "example:app")];
        app.begin_context_edit();
        app.clear_context_draft();
        for character in "./a-long-context-path/with-q-r-p-j-k-and-12345".chars() {
            app.push_context_char(character);
        }

        let rendered = buffer_lines(&render_build_buffer(60, 22, &app)).join("\n");
        assert!(rendered.contains("Edit context"));
        assert!(rendered.contains("Draft context"));
        assert!(rendered.contains("a-long-context-path"));
        assert!(rendered.contains("Enter save"));
        assert!(rendered.contains("Backspace delete"));
        assert!(!rendered.contains("Enter build"));

        app.clear_context_draft();
        let empty = buffer_lines(&render_build_buffer(60, 22, &app)).join("\n");
        assert!(empty.contains("empty — save to clear override"));
    }

    #[test]
    fn tasks_layout_switches_between_wide_and_narrow() {
        let app = sample_app();

        let wide = buffer_lines(&render_tasks_buffer(100, 12, &app));
        assert!(wide[0].contains("Tasks 3"));
        assert!(wide[0].contains("●1 ✓1 ×1"));
        assert!(wide[0].contains("Output ·"));

        let narrow = buffer_lines(&render_tasks_buffer(60, 12, &app));
        let output_row = narrow
            .iter()
            .position(|line| line.contains("Output ·"))
            .unwrap();
        assert!(narrow[0].contains("Tasks 3"));
        assert!(output_row > 0);
    }

    #[test]
    fn narrow_footer_keeps_selected_status_visible() {
        let mut app = sample_app();
        app.screen = Screen::Tasks;

        let rendered = buffer_lines(&render_app_buffer(70, 28, &app)).join("\n");
        assert!(rendered.contains("1/3 build running · 0 lines"));
        assert!(rendered.contains("image:latest"));
    }

    #[test]
    fn empty_tasks_use_the_full_pane() {
        let app = App::new();
        let rendered = buffer_lines(&render_tasks_buffer(70, 10, &app)).join("\n");

        assert!(rendered.contains("No tasks. Start a build or push"));
        assert!(!rendered.contains("Output"));
    }

    #[test]
    fn selected_task_stays_inside_the_visible_range() {
        let mut app = App::new();
        app.tasks = (0..10)
            .map(|index| {
                task(
                    TaskKind::Build,
                    &format!("task-{index}"),
                    TaskStatus::Succeeded,
                    &[],
                )
            })
            .collect();
        app.selected_task = 9;

        assert_eq!(visible_task_range(10, 9, 7), 6..10);

        let buffer = render_task_list_buffer(40, 7, &app);
        let rendered = buffer_lines(&buffer).join("\n");
        assert!(rendered.contains("task-9"));
        assert!(!rendered.contains("task-0"));
        assert_eq!(buffer.cell((1, 5)).unwrap().bg, Color::LightYellow);
    }

    #[test]
    fn output_wraps_and_tails_the_newest_rows() {
        let mut app = App::new();
        app.tasks.push(task(
            TaskKind::Build,
            "long-output",
            TaskStatus::Running,
            &[
                "old output that wraps across many terminal rows and should scroll away",
                "middle output",
                "newest-marker",
            ],
        ));

        let rendered = buffer_lines(&render_task_output_buffer(24, 5, &app)).join("\n");
        assert!(rendered.contains("newest-marker"));
        assert_eq!(output_tail_scroll(8, 3), 5);
        assert_eq!(output_tail_scroll(2, 3), 0);
    }

    #[test]
    fn task_statuses_keep_their_signal_colors() {
        assert_eq!(task_status_color(TaskStatus::Running), Color::Cyan);
        assert_eq!(task_status_color(TaskStatus::Succeeded), Color::Green);
        assert_eq!(task_status_color(TaskStatus::Failed), Color::Red);

        let mut app = sample_app();
        app.selected_task = 2;
        let buffer = render_task_list_buffer(50, 8, &app);
        let selected = buffer.cell((1, 4)).unwrap();
        assert_eq!(selected.fg, Color::Red);
        assert_eq!(selected.bg, Color::LightYellow);
    }
}
