use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, Wrap},
};

use crate::{
    app::{App, Modal, Screen},
    consts::{
        HELP_BUILD, HELP_CONTAINERS, HELP_IMAGES, HELP_LOGS, HELP_MODAL, HELP_TASKS, SIDEBAR_ITEMS,
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

fn render_build(frame: &mut Frame, area: Rect, app: &App) {
    let input = if app.input.is_empty() {
        "<enter build context path>"
    } else {
        app.input.as_str()
    };

    let mut text = vec![
        Line::from("Build an image with nerdctl from a local context path."),
        Line::from(""),
        Line::from(vec![
            Span::styled("Context path: ", Style::default().fg(Color::Yellow)),
            Span::styled(input, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(""),
        Line::from("Configured tags:"),
    ];

    if app.configs.is_empty() {
        text.push(Line::from("  <no configs in nd.toml>"));
    } else {
        for (index, config) in app.configs.iter().enumerate() {
            let marker = if index == app.selected_build_tag {
                ">"
            } else {
                " "
            };
            let tag = config.tag.as_deref().unwrap_or("<no tag resolved>");
            let version = config.latest_version.as_deref().unwrap_or("<not resolved>");
            let url = config.version_url.as_deref().unwrap_or("<no version url>");
            let style = if index == app.selected_build_tag {
                Style::default().fg(Color::Black).bg(Color::LightYellow)
            } else {
                Style::default()
            };

            text.push(Line::styled(
                format!("{marker} {tag} | latest: {version} | {url}"),
                style,
            ));
        }
    }

    text.extend([
        Line::from(""),
        Line::from("Use ↑/↓ to select a configured tag."),
        Line::from("Press Enter to run `nerdctl build -t <selected-tag> <path>`."),
        Line::from("Press r to reload nd.toml and re-resolve versions."),
        Line::from("Press p to prune the builder cache after confirmation."),
    ]);

    let widget = Paragraph::new(text)
        .block(Block::default().title("Build").borders(Borders::ALL))
        .wrap(Wrap { trim: true });

    frame.render_widget(widget, area);
}

fn render_tasks(frame: &mut Frame, area: Rect, app: &App) {
    // Split the Tasks screen into a left list of tasks and a right pane
    // showing the selected task's streaming output. The list is a fixed width
    // so the output pane grows with the terminal; both panes fill the height.
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(40), Constraint::Min(20)])
        .split(area);

    render_task_list(frame, chunks[0], app);
    render_task_output(frame, chunks[1], app);
}

fn render_task_list(frame: &mut Frame, area: Rect, app: &App) {
    let title = format!("Tasks ({})", app.tasks.len());

    if app.tasks.is_empty() {
        render_empty(
            frame,
            area,
            &title,
            "No tasks. Start a build or push; it appears here automatically.",
        );
        return;
    }

    let rows = app
        .tasks
        .iter()
        .enumerate()
        .map(|(index, task)| {
            let selected = index == app.selected_task;

            // Status keeps its own color in both states so a running/done/
            // failed task is identifiable at a glance even when highlighted.
            let (status_label, status_fg) = match task.status {
                crate::app::TaskStatus::Running => ("running", Color::Cyan),
                crate::app::TaskStatus::Succeeded => ("done", Color::Green),
                crate::app::TaskStatus::Failed => ("failed", Color::Red),
            };

            let row_style = if selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::LightYellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(task.kind.label()),
                Cell::from(status_label).style(Style::default().fg(status_fg)),
                Cell::from(task.title.as_str()),
            ])
            .style(row_style)
        })
        .collect::<Vec<_>>();

    let table = Table::new(
        rows,
        [
            Constraint::Length(6),
            Constraint::Length(9),
            Constraint::Min(15),
        ],
    )
    .header(
        Row::new(vec!["Kind", "Status", "Task"]).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(Block::default().title(title).borders(Borders::ALL));

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

    // Show only the lines that fit in the visible area, tailing the newest.
    let height = area.height.saturating_sub(2) as usize;
    let start = task.lines.len().saturating_sub(height);
    let visible = &task.lines[start..];

    // Color the status in the title so the selected task's state is obvious
    // without scanning the list.
    let status_fg = match task.status {
        crate::app::TaskStatus::Running => Color::Cyan,
        crate::app::TaskStatus::Succeeded => Color::Green,
        crate::app::TaskStatus::Failed => Color::Red,
    };
    let title = Line::from(vec![
        Span::raw("Output: "),
        Span::styled(task.title.as_str(), Style::default().fg(Color::Cyan)),
        Span::raw(" ["),
        Span::styled(task.status.label(), Style::default().fg(status_fg)),
        Span::raw("]"),
    ]);

    // Build a Text from the line slice directly instead of joining into a
    // fresh String every frame (avoids an O(n) allocation per render).
    let text = if visible.is_empty() {
        Text::from(if task.is_running() {
            "Waiting for output..."
        } else {
            "No output captured."
        })
    } else {
        visible
            .iter()
            .map(|line| Line::from(line.as_str()))
            .collect()
    };

    let widget = Paragraph::new(text)
        .block(Block::default().title(title).borders(Borders::ALL))
        .wrap(Wrap { trim: false });

    frame.render_widget(widget, area);
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
        Screen::Containers => app
            .selected_container()
            .map(|container| {
                format!(
                    "Container {}\nImage: {}\nCommand: {}",
                    container.id, container.image, container.command
                )
            })
            .unwrap_or_else(|| "No container selected.".to_string()),
        Screen::Images => app
            .selected_image()
            .map(|image| {
                format!(
                    "Image {}:{}\nID: {}\nSize: {}",
                    image.repository, image.tag, image.id, image.size
                )
            })
            .unwrap_or_else(|| "No image selected.".to_string()),
        Screen::Build => app
            .selected_config()
            .map(|config| {
                format!(
                    "Build context path\n{}\n\nTag template\n{}\n\nLatest version\n{}\n\nResolved tag\n{}",
                    if app.input.is_empty() {
                        "No path entered."
                    } else {
                        app.input.as_str()
                    },
                    config
                        .tag_template
                        .as_deref()
                        .unwrap_or("No tag template."),
                    config.latest_version.as_deref().unwrap_or("Not resolved."),
                    config.tag.as_deref().unwrap_or("No tag resolved.")
                )
            })
            .unwrap_or_else(|| "No build config selected.".to_string()),
        Screen::Tasks => {
            let running = app.tasks.iter().filter(|task| task.is_running()).count();
            let total = app.tasks.len();
            let selected = app
                .selected_task()
                .map(|task| {
                    format!(
                        "{} [{}]\n{} output lines",
                        task.title,
                        task.status.label(),
                        task.lines.len()
                    )
                })
                .unwrap_or_else(|| "No task selected.".to_string());
            format!(
                "Tasks\n{running} running, {total} total\n\nSelected\n{selected}"
            )
        }
        Screen::Logs => format!("{} log lines", app.logs.len()),
    };

    let help = match app.screen {
        Screen::Containers => HELP_CONTAINERS,
        Screen::Images => HELP_IMAGES,
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
        Modal::ConfirmRemoveContainer => {
            let target = app
                .selected_container()
                .map(|container| format!("{} ({})", container.names, container.id))
                .unwrap_or_else(|| "selected container".to_string());
            ("Remove container", format!("Remove {target}?"), HELP_MODAL)
        }
        Modal::ConfirmPushImage => {
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
        Modal::ConfirmRemoveImage => {
            let target = app
                .selected_image()
                .map(|image| format!("{}:{} ({})", image.repository, image.tag, image.id))
                .unwrap_or_else(|| "selected image".to_string());
            ("Remove image", format!("Remove {target}?"), HELP_MODAL)
        }
        Modal::ConfirmImagePrune => (
            "Prune images",
            "Remove unused dangling images?".to_string(),
            HELP_MODAL,
        ),
        Modal::ConfirmBuilderPrune => (
            "Prune builder",
            "Remove builder cache?".to_string(),
            HELP_MODAL,
        ),
        Modal::ConfirmSystemPrune => (
            "System prune",
            "Remove unused Docker data?".to_string(),
            HELP_MODAL,
        ),
        Modal::ConfirmQuit => {
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
