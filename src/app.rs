use std::sync::mpsc::Receiver;

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
}

impl ProgressState {
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            running: false,
            events: None,
        }
    }

    pub fn start(&mut self, receiver: Receiver<docker::ProgressEvent>) {
        self.lines.clear();
        self.events = Some(receiver);
        self.running = true;
    }

    /// Drain all buffered events without blocking.
    pub fn drain_events(&mut self) -> Vec<docker::ProgressEvent> {
        let mut events = Vec::new();
        if let Some(receiver) = &self.events {
            while let Ok(event) = receiver.try_recv() {
                events.push(event);
            }
        }
        events
    }

    /// Called when a Finished event arrives. Clears the running flag and
    /// drops the receiver.
    pub fn handle_finished(&mut self) {
        self.running = false;
        self.events = None;
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
    pub configs: Vec<Config>,
    pub build_state: ProgressState,
    pub push_state: ProgressState,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub build_tag_template: Option<String>,
    pub build_tag: Option<String>,
    pub latest_version: Option<String>,
    pub version_url: Option<String>,
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
        }
    }

    pub fn refresh(&mut self) {
        let mut refreshed = true;

        match config::read_build_config() {
            Ok(build_config) => {
                self.configs = build_config
                    .configs
                    .into_iter()
                    .map(|config| Config {
                        build_tag_template: config.tag_template,
                        build_tag: config.tag,
                        latest_version: config.latest_version,
                        version_url: config.version_url,
                    })
                    .collect();
                self.clamp_build_selection();
            }
            Err(err) => {
                refreshed = false;
                self.set_error(format!("config: {err}"));
            }
        }

        match docker::list_containers() {
            Ok(containers) => {
                self.containers = containers;
                self.clamp_container_selection();
            }
            Err(err) => {
                refreshed = false;
                self.set_error(format!("containers: {err}"));
            }
        }

        match docker::list_images() {
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

    pub fn selected_config(&self) -> Option<&Config> {
        self.configs.get(self.selected_build_tag)
    }

    pub fn open_modal(&mut self, modal: Modal) {
        self.modal = Some(modal);
    }

    pub fn close_modal(&mut self) {
        self.modal = None;
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

    pub fn start_build(&mut self, receiver: Receiver<docker::ProgressEvent>) {
        self.build_state.start(receiver);
        self.status = "build started".to_string();
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

    pub fn start_push(&mut self, receiver: Receiver<docker::ProgressEvent>) {
        self.push_state.start(receiver);
        self.status = "push started".to_string();
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
