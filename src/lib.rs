pub(crate) mod app;
pub(crate) mod config;
pub(crate) mod consts;
pub(crate) mod docker;
pub(crate) mod ui;

use std::time::Duration;

use anyhow::Result;
use crossterm::{
    event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, poll},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend},
};
use ui::ui;

use crate::app::{App, Modal, Screen};

pub fn run_tui() -> Result<()> {
    enable_raw_mode()?;
    let mut stderr = std::io::stderr();
    execute!(stderr, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let result = run_app(&mut terminal, &mut app);

    let cleanup_result = (|| -> Result<()> {
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;
        Ok(())
    })();

    match (result, cleanup_result) {
        (Err(err), _) => Err(err),
        (Ok(()), Err(err)) => Err(err),
        (Ok(()), Ok(())) => Ok(()),
    }
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()>
where
    anyhow::Error: From<B::Error>,
{
    app.refresh();

    while !app.should_quit {
        app.poll_task_events();
        app.poll_refresh_events();
        app.poll_action_events();
        terminal.draw(|frame| ui(frame, app))?;

        if poll(Duration::from_millis(100))?
            && let Event::Key(key) = crossterm::event::read()?
        {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            handle_key(app, key);
        }
    }

    // Kill any build/push still running in the background so they don't
    // outlive the TUI as orphaned `nerdctl` processes.
    app.cancel_running_tasks();

    Ok(())
}

fn handle_key(app: &mut App, key: KeyEvent) {
    if app.modal.is_some() {
        handle_modal_key(app, key);
    } else {
        handle_main_key(app, key);
    }
}

fn handle_main_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.should_quit = true
        }
        KeyCode::Char('q') => {
            if app.has_running_tasks() {
                app.open_modal(Modal::Quit);
            } else {
                app.should_quit = true;
            }
        }
        KeyCode::Char('r') => app.refresh(),
        KeyCode::Tab => app.next_screen(),
        KeyCode::BackTab => app.previous_screen(),
        KeyCode::Char('1') => app.set_screen(Screen::Containers),
        KeyCode::Char('2') => app.set_screen(Screen::Images),
        KeyCode::Char('3') => app.set_screen(Screen::Build),
        KeyCode::Char('4') => app.set_screen(Screen::Tasks),
        KeyCode::Char('5') => app.set_screen(Screen::Logs),
        KeyCode::Up | KeyCode::Char('k') => app.move_up(),
        KeyCode::Down | KeyCode::Char('j') => app.move_down(),
        KeyCode::Char('s') if app.screen == Screen::Containers => {
            with_selected_container(app, "started", docker::start_container)
        }
        KeyCode::Char('x') if app.screen == Screen::Containers => {
            with_selected_container(app, "stopped", docker::stop_container)
        }
        KeyCode::Char('R') if app.screen == Screen::Containers => {
            with_selected_container(app, "restarted", docker::restart_container)
        }
        KeyCode::Char('d') if app.screen == Screen::Containers => {
            open_if_container_selected(app, Modal::RemoveContainer)
        }
        KeyCode::Char('p') if app.screen == Screen::Containers => {
            app.open_modal(Modal::SystemPrune)
        }
        KeyCode::Char('d') if app.screen == Screen::Images => {
            open_if_image_selected(app, Modal::RemoveImage)
        }
        KeyCode::Char('p') if app.screen == Screen::Images => app.open_modal(Modal::ImagePrune),
        KeyCode::Char('s') if app.screen == Screen::Images => {
            app.open_modal(Modal::PushImage);
        }
        KeyCode::Char('p') if app.screen == Screen::Build => app.open_modal(Modal::BuilderPrune),
        KeyCode::Enter if app.screen == Screen::Build => run_build(app),
        KeyCode::Backspace if app.screen == Screen::Build => {
            app.input.pop();
        }
        KeyCode::Char('u')
            if app.screen == Screen::Build && key.modifiers.contains(KeyModifiers::CONTROL) =>
        {
            app.input.clear();
            app.set_status("build path cleared");
        }
        KeyCode::Char('c') if app.screen == Screen::Tasks => app.clear_selected_task_lines(),
        KeyCode::Char('c') if app.screen == Screen::Logs => app.clear_logs(),
        KeyCode::Char('d') if app.screen == Screen::Tasks => app.delete_selected_task(),
        KeyCode::Esc if app.screen == Screen::Tasks => app.cancel_selected_task(),
        KeyCode::Char(character)
            if app.screen == Screen::Build
                && (key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT) =>
        {
            app.input.push(character);
        }
        _ => {}
    }
}

fn handle_modal_key(app: &mut App, key: KeyEvent) {
    let Some(modal) = app.modal else {
        return;
    };

    match modal {
        Modal::RemoveContainer
        | Modal::PushImage
        | Modal::RemoveImage
        | Modal::ImagePrune
        | Modal::BuilderPrune
        | Modal::SystemPrune
        | Modal::Quit => handle_confirmation_key(app, modal, key),
    }
}

fn handle_confirmation_key(app: &mut App, modal: Modal, key: KeyEvent) {
    match key.code {
        KeyCode::Char('y') => {
            app.close_modal();
            match modal {
                Modal::RemoveContainer => {
                    with_selected_container(app, "removed", docker::remove_container)
                }
                Modal::PushImage => run_push(app),
                Modal::RemoveImage => with_selected_image(app, "removed", docker::remove_image),
                Modal::ImagePrune => run_action(app, "images pruned", docker::prune_images),
                Modal::BuilderPrune => run_action(app, "builder pruned", docker::prune_builder),
                Modal::SystemPrune => run_action(app, "system pruned", docker::system_prune),
                Modal::Quit => app.should_quit = true,
            }
        }
        KeyCode::Char('n') | KeyCode::Esc => app.close_modal(),
        _ => {}
    }
}

fn run_build(app: &mut App) {
    let path = app.input.trim().to_string();

    if path.is_empty() {
        app.set_error("build path is required");
        return;
    }

    let tag = app.selected_config().and_then(|config| config.tag.clone());

    match docker::build_image_stream(path.clone(), tag.clone()) {
        Ok((receiver, cancel)) => {
            let title = tag.unwrap_or_else(|| format!("build {path}"));
            app.start_build(receiver, cancel, title);
            app.set_screen(Screen::Tasks);
        }
        Err(err) => app.set_error(err),
    }
}

fn run_push(app: &mut App) {
    let Some((repo, tag)) = app
        .selected_image()
        .map(|image| (image.repository.clone(), image.tag.clone()))
    else {
        app.set_error("no image selected");
        return;
    };

    match docker::push_image_stream(repo.clone(), tag.clone()) {
        Ok((receiver, cancel)) => {
            let title = format!("{repo}:{tag}");
            app.start_push(receiver, cancel, title);
        }
        Err(err) => app.set_error(err),
    }
}

fn open_if_container_selected(app: &mut App, modal: Modal) {
    if app.selected_container().is_some() {
        app.open_modal(modal);
    } else {
        app.set_error("no container selected");
    }
}

fn open_if_image_selected(app: &mut App, modal: Modal) {
    if app.selected_image().is_some() {
        app.open_modal(modal);
    } else {
        app.set_error("no image selected");
    }
}

fn with_selected_container(
    app: &mut App,
    success_status: &str,
    action: fn(&str) -> Result<String>,
) {
    let Some(id) = app
        .selected_container()
        .map(|container| container.id.clone())
    else {
        app.set_error("no container selected");
        return;
    };

    run_action(app, format!("container {success_status}"), move || {
        action(&id)
    });
}

fn with_selected_image(
    app: &mut App,
    success_status: &str,
    action: fn(&str, Option<&str>) -> Result<String>,
) {
    let Some((repo, tag)) = app
        .selected_image()
        .map(|image| (image.repository.clone(), image.tag.clone()))
    else {
        app.set_error("no image selected");
        return;
    };

    run_action(app, format!("image {success_status}"), move || {
        action(&repo, Some(&tag))
    });
}

fn run_action<F>(app: &mut App, success_status: impl Into<String>, action: F)
where
    F: FnOnce() -> Result<String> + Send + 'static,
{
    app.start_action(success_status, action);
}
