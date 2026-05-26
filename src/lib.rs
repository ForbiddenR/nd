pub(crate) mod app;
pub(crate) mod consts;
pub(crate) mod docker;
pub(crate) mod ui;

use anyhow::Result;
use crossterm::{
    event::{
        DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers,
    },
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
    execute!(stderr, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let result = run_app(&mut terminal, &mut app);

    let cleanup_result = (|| -> Result<()> {
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture,
        )?;
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
        terminal.draw(|frame| ui(frame, app))?;

        if let Event::Key(key) = crossterm::event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            handle_key(app, key);
        }
    }

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
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('r') => app.refresh(),
        KeyCode::Tab => app.next_screen(),
        KeyCode::BackTab => app.previous_screen(),
        KeyCode::Char('1') => app.set_screen(Screen::Containers),
        KeyCode::Char('2') => app.set_screen(Screen::Images),
        KeyCode::Char('3') => app.set_screen(Screen::Logs),
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
            open_if_container_selected(app, Modal::ConfirmRemoveContainer)
        }
        KeyCode::Char('p') if app.screen == Screen::Containers => {
            app.open_modal(Modal::ConfirmSystemPrune)
        }
        KeyCode::Char('d') if app.screen == Screen::Images => {
            open_if_image_selected(app, Modal::ConfirmRemoveImage)
        }
        KeyCode::Char('p') if app.screen == Screen::Images => {
            app.open_modal(Modal::ConfirmImagePrune)
        }
        KeyCode::Char('p') if app.screen == Screen::Build => {
            app.open_modal(Modal::ConfirmBuilderPrune)
        }
        KeyCode::Char('b') if app.screen == Screen::Images => app.open_modal(Modal::BuildImage),
        KeyCode::Char('c') if app.screen == Screen::Logs => app.clear_logs(),
        _ => {}
    }
}

fn handle_modal_key(app: &mut App, key: KeyEvent) {
    let Some(modal) = app.modal else {
        return;
    };

    match modal {
        Modal::BuildImage => handle_build_modal_key(app, key),
        Modal::ConfirmRemoveContainer
        | Modal::ConfirmRemoveImage
        | Modal::ConfirmImagePrune
        | Modal::ConfirmBuilderPrune
        | Modal::ConfirmSystemPrune => handle_confirmation_key(app, modal, key),
    }
}

fn handle_confirmation_key(app: &mut App, modal: Modal, key: KeyEvent) {
    match key.code {
        KeyCode::Char('y') => {
            app.close_modal();
            match modal {
                Modal::ConfirmRemoveContainer => {
                    with_selected_container(app, "removed", docker::remove_container)
                }
                Modal::ConfirmRemoveImage => {
                    with_selected_image(app, "removed", docker::remove_image)
                }
                Modal::ConfirmImagePrune => run_action(app, "images pruned", docker::prune_images),
                Modal::ConfirmBuilderPrune => {
                    run_action(app, "builder pruned", docker::prune_builder)
                }
                Modal::ConfirmSystemPrune => run_action(app, "system pruned", docker::system_prune),
                Modal::BuildImage => {}
            }
        }
        KeyCode::Char('n') | KeyCode::Esc => app.close_modal(),
        _ => {}
    }
}

fn handle_build_modal_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Enter => {
            let path = app.input.trim().to_string();
            app.close_modal();

            if path.is_empty() {
                app.set_error("build path is required");
            } else {
                run_action(app, "image build completed", || docker::build_image(&path));
            }
        }
        KeyCode::Backspace => {
            app.input.pop();
        }
        KeyCode::Esc => app.close_modal(),
        KeyCode::Char(character) => {
            if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                app.input.push(character);
            }
        }
        _ => {}
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

    run_action(app, format!("container {success_status}"), || action(&id));
}

fn with_selected_image(app: &mut App, success_status: &str, action: fn(&str) -> Result<String>) {
    let Some(id) = app.selected_image().map(|image| image.id.clone()) else {
        app.set_error("no image selected");
        return;
    };

    run_action(app, format!("image {success_status}"), || action(&id));
}

fn run_action<F>(app: &mut App, success_status: impl Into<String>, action: F)
where
    F: FnOnce() -> Result<String>,
{
    let success_status = success_status.into();

    match action() {
        Ok(output) => {
            app.set_status(success_status);
            app.push_log(output);
            app.refresh();
        }
        Err(err) => app.set_error(err),
    }
}
