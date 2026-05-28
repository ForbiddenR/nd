use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, Wrap},
};

use crate::{
    app::{App, Modal, Screen},
    consts::{
        HELP_BUILD, HELP_BUILD_STATUS, HELP_CONTAINERS, HELP_IMAGES, HELP_LOGS, HELP_MODAL,
        SIDEBAR_ITEMS,
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
        Screen::BuildStatus => render_build_status(frame, area, app),
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
            let tag = config.build_tag.as_deref().unwrap_or("<no tag resolved>");
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

fn render_build_status(frame: &mut Frame, area: Rect, app: &App) {
    let height = area.height.saturating_sub(2) as usize;
    let start = app.build_lines.len().saturating_sub(height);
    let text = if app.build_lines.is_empty() {
        "No build output yet. Start a build from the Build page.".to_string()
    } else {
        app.build_lines[start..].join("\n")
    };
    let title = if app.build_running {
        "Build Status (running)"
    } else {
        "Build Status"
    };

    let status = Paragraph::new(text)
        .block(Block::default().title(title).borders(Borders::ALL))
        .wrap(Wrap { trim: false });

    frame.render_widget(status, area);
}

fn render_logs(frame: &mut Frame, area: Rect, app: &App) {
    let height = area.height.saturating_sub(2) as usize;
    let start = app.logs.len().saturating_sub(height);
    let text = if app.logs.is_empty() {
        "No logs yet.".to_string()
    } else {
        app.logs[start..].join("\n")
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
                        .build_tag_template
                        .as_deref()
                        .unwrap_or("No tag template."),
                    config.latest_version.as_deref().unwrap_or("Not resolved."),
                    config.build_tag.as_deref().unwrap_or("No tag resolved.")
                )
            })
            .unwrap_or_else(|| "No build config selected.".to_string()),
        Screen::BuildStatus => format!(
            "Build status\n{}\n\n{} output lines",
            if app.build_running { "Running" } else { "Idle" },
            app.build_lines.len()
        ),
        Screen::Logs => format!("{} log lines", app.logs.len()),
    };

    let help = match app.screen {
        Screen::Containers => HELP_CONTAINERS,
        Screen::Images => HELP_IMAGES,
        Screen::Build => HELP_BUILD,
        Screen::BuildStatus => HELP_BUILD_STATUS,
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
