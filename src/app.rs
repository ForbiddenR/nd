use std::{
    sync::mpsc::{self, Receiver},
    thread,
};

use crate::{config, docker};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Containers,
    Images,
    Build,
    BuildStatus,
    PushStatus,
    Logs,
}

impl Screen {
    pub fn index(self) -> usize {
        match self {
            Self::Containers => 0,
            Self::Images => 1,
            Self::Build => 2,
            Self::BuildStatus => 3,
            Self::PushStatus => 4,
            Self::Logs => 5,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Containers => Self::Images,
            Self::Images => Self::Build,
            Self::Build => Self::BuildStatus,
            Self::BuildStatus => Self::PushStatus,
            Self::PushStatus => Self::Logs,
            Self::Logs => Self::Containers,
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Self::Containers => Self::Logs,
            Self::Images => Self::Containers,
            Self::Build => Self::Images,
            Self::BuildStatus => Self::Build,
            Self::PushStatus => Self::BuildStatus,
            Self::Logs => Self::PushStatus,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modal {
    ConfirmRemoveContainer,
    ConfirmRemoveImage,
    ConfirmImagePrune,
    ConfirmBuilderPrune,
    ConfirmSystemPrune,
    ConfirmPushImage,
    ConfirmQuit,
}

#[derive(Debug, Clone)]
pub struct Container {
    pub id: String,
    pub image: String,
    pub command: String,
    pub status: String,
    pub names: String,
}

#[derive(Debug, Clone)]
pub struct Image {
    pub repository: String,
    pub tag: String,
    pub id: String,
    pub size: String,
}

/// Shared state for a streaming nerdctl operation (build, push, etc.).
/// Owns the output lines, running flag, and event receiver so the caller
/// doesn't need to duplicate this triple for each operation.
#[derive(Debug)]
pub struct ProgressState {
    pub lines: Vec<String>,
    pub running: bool,
    events: Option<Receiver<docker::ProgressEvent>>,
    cancel: Option<docker::CancelHandle>,
}

impl ProgressState {
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            running: false,
            events: None,
            cancel: None,
        }
    }

    pub fn start(
        &mut self,
        receiver: Receiver<docker::ProgressEvent>,
        cancel: docker::CancelHandle,
    ) {
        self.lines.clear();
        self.events = Some(receiver);
        self.cancel = Some(cancel);
        self.running = true;
    }

    /// Drain all buffered events without blocking. Returns an empty vec when
    /// no operation is running so idle ticks in the render loop are cheap.
    pub fn drain_events(&mut self) -> Vec<docker::ProgressEvent> {
        let Some(receiver) = &self.events else {
            return Vec::new();
        };
        let mut events = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            events.push(event);
        }
        events
    }

    /// Called when a Finished event arrives. Clears the running flag and
    /// drops the receiver and cancel handle.
    pub fn handle_finished(&mut self) {
        self.running = false;
        self.events = None;
        self.cancel = None;
    }

    /// Send SIGKILL to the running child, if any. The actual `Finished` event
    /// arrives later via `drain_events`, so the status/error is updated then.
    /// No-op when nothing is running.
    pub fn cancel(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            cancel.cancel();
        }
    }

    /// Clear output lines if not running. Returns true if cleared.
    pub fn clear_lines(&mut self) -> bool {
        if self.running {
            return false;
        }
        self.lines.clear();
        true
    }

    /// Append a line with a bounded cap. Public so the poll helpers on App
    /// can call it directly.
    pub fn push_line(lines: &mut Vec<String>, line: String) {
        if line.trim().is_empty() {
            return;
        }

        lines.push(line);

        if lines.len() > 2_000 {
            let remove_count = lines.len() - 2_000;
            lines.drain(0..remove_count);
        }
    }
}

/// Result of a background refresh, produced off the UI thread so listing
/// containers/images and resolving config (which may make a network call)
/// don't block the render loop.
#[derive(Debug)]
pub enum RefreshEvent {
    Done {
        configs: anyhow::Result<config::BuildConfig>,
        containers: anyhow::Result<Vec<Container>>,
        images: anyhow::Result<Vec<Image>>,
    },
}

/// Result of a background one-shot action (remove container/image, prune).
/// These run `nerdctl` to completion with captured output, which can take
/// several seconds for prunes - running them off the UI thread keeps the TUI
/// responsive. Only one action runs at a time; starting another while one is
/// in flight is rejected with an error.
#[derive(Debug)]
pub enum ActionEvent {
    Done {
        success_status: String,
        result: anyhow::Result<String>,
    },
}

#[derive(Debug)]
pub struct App {
    pub should_quit: bool,
    pub screen: Screen,
    pub modal: Option<Modal>,
    pub containers: Vec<Container>,
    pub images: Vec<Image>,
    pub selected_container: usize,
    pub selected_image: usize,
    pub selected_build_tag: usize,
    pub status: String,
    pub logs: Vec<String>,
    pub input: String,
    pub configs: Vec<config::Config>,
    pub build_state: ProgressState,
    pub push_state: ProgressState,
    refresh_events: Option<Receiver<RefreshEvent>>,
    refresh_pending: bool,
    action_events: Option<Receiver<ActionEvent>>,
}

impl App {
    pub fn new() -> Self {
        Self {
            should_quit: false,
            screen: Screen::Containers,
            modal: None,
            containers: Vec::new(),
            images: Vec::new(),
            selected_container: 0,
            selected_image: 0,
            selected_build_tag: 0,
            status: "ready".to_string(),
            logs: Vec::new(),
            input: String::new(),
            configs: vec![],
            build_state: ProgressState::new(),
            push_state: ProgressState::new(),
            refresh_events: None,
            refresh_pending: false,
            action_events: None,
        }
    }

    /// Kick off a background refresh. Listing containers/images and resolving
    /// config (which may make a network request with a 10 s timeout) would
    /// block the render loop, so the work runs on a thread and the result is
    /// applied via `poll_refresh_events`. A refresh requested while one is
    /// already in flight is deferred until the current one finishes.
    pub fn refresh(&mut self) {
        if self.refresh_events.is_some() {
            self.refresh_pending = true;
            return;
        }
        self.start_refresh();
    }

    fn start_refresh(&mut self) {
        self.refresh_events = Some(spawn_refresh());
        self.status = "refreshing...".to_string();
    }

    /// Drain a completed refresh (if any) and apply the results. Called each
    /// tick from the main loop so it stays non-blocking.
    pub fn poll_refresh_events(&mut self) {
        let Some(receiver) = &self.refresh_events else {
            return;
        };

        if let Ok(RefreshEvent::Done {
            configs,
            containers,
            images,
        }) = receiver.try_recv()
        {
            self.refresh_events = None;
            self.apply_refresh(configs, containers, images);

            if self.refresh_pending {
                self.refresh_pending = false;
                self.start_refresh();
            }
        }
    }

    /// Kick off a one-shot action (remove, prune) on a background thread.
    /// The action runs `nerdctl` to completion, which can take several seconds
    /// for prune operations; running it off the UI thread keeps the TUI
    /// responsive. The result is applied via `poll_action_events`. Only one
    /// action runs at a time - a second request while one is in flight is
    /// rejected with an error.
    pub fn start_action<F>(&mut self, success_status: impl Into<String>, action: F)
    where
        F: FnOnce() -> anyhow::Result<String> + Send + 'static,
    {
        if self.action_events.is_some() {
            self.set_error("an action is already running");
            return;
        }

        let success_status = success_status.into();
        let (tx, rx) = mpsc::channel();
        self.action_events = Some(rx);
        self.status = format!("{success_status}...");

        thread::spawn(move || {
            let result = action();
            let _ = tx.send(ActionEvent::Done {
                success_status,
                result,
            });
        });
    }

    /// Drain a completed action (if any) and apply the result. Called each tick
    /// from the main loop so it stays non-blocking.
    pub fn poll_action_events(&mut self) {
        let Some(receiver) = &self.action_events else {
            return;
        };

        if let Ok(ActionEvent::Done {
            success_status,
            result,
        }) = receiver.try_recv()
        {
            self.action_events = None;
            match result {
                Ok(output) => {
                    self.set_status(success_status);
                    self.push_log(output);
                    self.refresh();
                }
                Err(err) => self.set_error(err),
            }
        }
    }

    fn apply_refresh(
        &mut self,
        configs: anyhow::Result<config::BuildConfig>,
        containers: anyhow::Result<Vec<Container>>,
        images: anyhow::Result<Vec<Image>>,
    ) {
        let mut refreshed = true;

        match configs {
            Ok(build_config) => {
                self.configs = build_config.configs;
                self.clamp_build_selection();
            }
            Err(err) => {
                refreshed = false;
                self.set_error(format!("config: {err}"));
            }
        }

        match containers {
            Ok(containers) => {
                self.containers = containers;
                self.clamp_container_selection();
            }
            Err(err) => {
                refreshed = false;
                self.set_error(format!("containers: {err}"));
            }
        }

        match images {
            Ok(images) => {
                self.images = images;
                self.clamp_image_selection();
            }
            Err(err) => {
                refreshed = false;
                self.set_error(format!("images: {err}"));
            }
        }

        if refreshed {
            self.status = format!(
                "refreshed: {} containers, {} images",
                self.containers.len(),
                self.images.len()
            );
        }
    }

    pub fn move_up(&mut self) {
        match self.screen {
            Screen::Containers if !self.containers.is_empty() => {
                self.selected_container = self.selected_container.saturating_sub(1);
            }
            Screen::Images if !self.images.is_empty() => {
                self.selected_image = self.selected_image.saturating_sub(1);
            }
            Screen::Build if !self.configs.is_empty() => {
                self.selected_build_tag = self.selected_build_tag.saturating_sub(1);
            }
            _ => {}
        }
    }

    pub fn move_down(&mut self) {
        match self.screen {
            Screen::Containers if !self.containers.is_empty() => {
                self.selected_container =
                    (self.selected_container + 1).min(self.containers.len().saturating_sub(1));
            }
            Screen::Images if !self.images.is_empty() => {
                self.selected_image =
                    (self.selected_image + 1).min(self.images.len().saturating_sub(1));
            }
            Screen::Build if !self.configs.is_empty() => {
                self.selected_build_tag =
                    (self.selected_build_tag + 1).min(self.configs.len().saturating_sub(1));
            }
            _ => {}
        }
    }

    pub fn next_screen(&mut self) {
        self.screen = self.screen.next();
    }

    pub fn previous_screen(&mut self) {
        self.screen = self.screen.previous();
    }

    pub fn set_screen(&mut self, screen: Screen) {
        self.screen = screen;
    }

    pub fn selected_container(&self) -> Option<&Container> {
        self.containers.get(self.selected_container)
    }

    pub fn selected_image(&self) -> Option<&Image> {
        self.images.get(self.selected_image)
    }

    pub fn selected_config(&self) -> Option<&config::Config> {
        self.configs.get(self.selected_build_tag)
    }

    pub fn open_modal(&mut self, modal: Modal) {
        self.modal = Some(modal);
    }

    pub fn close_modal(&mut self) {
        self.modal = None;
    }

    /// Returns true if a build or push is still running in the background.
    pub fn has_running_tasks(&self) -> bool {
        self.build_state.running || self.push_state.running
    }

    /// Kill any running build/push children so they don't survive the TUI as
    /// orphaned `nerdctl` processes. Called on quit.
    pub fn cancel_running_tasks(&mut self) {
        self.build_state.cancel();
        self.push_state.cancel();
    }

    pub fn push_log(&mut self, message: impl Into<String>) {
        let message = message.into();
        if message.trim().is_empty() {
            return;
        }

        for line in message.lines() {
            self.logs.push(line.to_string());
        }

        if self.logs.len() > 1_000 {
            let remove_count = self.logs.len() - 1_000;
            self.logs.drain(0..remove_count);
        }
    }

    pub fn clear_logs(&mut self) {
        self.logs.clear();
        self.status = "logs cleared".to_string();
    }

    pub fn start_build(
        &mut self,
        receiver: Receiver<docker::ProgressEvent>,
        cancel: docker::CancelHandle,
    ) {
        self.build_state.start(receiver, cancel);
        self.status = "build started".to_string();
    }

    pub fn cancel_build(&mut self) {
        if self.build_state.running {
            self.build_state.cancel();
            self.status = "cancelling build...".to_string();
        }
    }

    pub fn poll_build_events(&mut self) {
        for event in self.build_state.drain_events() {
            match event {
                docker::ProgressEvent::Line(line) => {
                    ProgressState::push_line(&mut self.build_state.lines, line);
                }
                docker::ProgressEvent::Finished { success, message } => {
                    ProgressState::push_line(&mut self.build_state.lines, message.clone());
                    self.build_state.handle_finished();
                    if success {
                        self.status = "build completed".to_string();
                        self.refresh();
                    } else {
                        self.set_error(message);
                    }
                }
            }
        }
    }

    pub fn clear_build_lines(&mut self) {
        if self.build_state.clear_lines() {
            self.status = "build status cleared".to_string();
        }
    }

    pub fn start_push(
        &mut self,
        receiver: Receiver<docker::ProgressEvent>,
        cancel: docker::CancelHandle,
    ) {
        self.push_state.start(receiver, cancel);
        self.status = "push started".to_string();
    }

    pub fn cancel_push(&mut self) {
        if self.push_state.running {
            self.push_state.cancel();
            self.status = "cancelling push...".to_string();
        }
    }

    pub fn poll_push_events(&mut self) {
        for event in self.push_state.drain_events() {
            match event {
                docker::ProgressEvent::Line(line) => {
                    ProgressState::push_line(&mut self.push_state.lines, line);
                }
                docker::ProgressEvent::Finished { success, message } => {
                    ProgressState::push_line(&mut self.push_state.lines, message.clone());
                    self.push_state.handle_finished();
                    if success {
                        self.status = "push completed".to_string();
                        self.refresh();
                    } else {
                        self.set_error(message);
                    }
                }
            }
        }
    }

    pub fn clear_push_lines(&mut self) {
        if self.push_state.clear_lines() {
            self.status = "push status cleared".to_string();
        }
    }

    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status = status.into();
    }

    pub fn set_error(&mut self, err: impl std::fmt::Display) {
        let message = err.to_string();
        self.status = format!("error: {message}");
        self.push_log(format!("error: {message}"));
    }

    fn clamp_container_selection(&mut self) {
        if self.containers.is_empty() {
            self.selected_container = 0;
        } else {
            self.selected_container = self
                .selected_container
                .min(self.containers.len().saturating_sub(1));
        }
    }

    fn clamp_image_selection(&mut self) {
        if self.images.is_empty() {
            self.selected_image = 0;
        } else {
            self.selected_image = self.selected_image.min(self.images.len().saturating_sub(1));
        }
    }

    fn clamp_build_selection(&mut self) {
        if self.configs.is_empty() {
            self.selected_build_tag = 0;
        } else {
            self.selected_build_tag = self
                .selected_build_tag
                .min(self.configs.len().saturating_sub(1));
        }
    }
}

/// Run all three refresh sources on a background thread and send a single
/// `RefreshEvent::Done` back. Each source is wrapped in its own `Result` so a
/// failure in one (e.g. a config network error) doesn't discard the others.
fn spawn_refresh() -> Receiver<RefreshEvent> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let configs = config::read_build_config();
        let containers = docker::list_containers();
        let images = docker::list_images();
        let _ = tx.send(RefreshEvent::Done {
            configs,
            containers,
            images,
        });
    });
    rx
}
