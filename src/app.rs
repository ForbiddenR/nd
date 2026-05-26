use crate::docker;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Containers,
    Images,
    Build,
    Logs,
}

impl Screen {
    pub fn index(self) -> usize {
        match self {
            Self::Containers => 0,
            Self::Images => 1,
            Self::Build => 2,
            Self::Logs => 3,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Containers => Self::Images,
            Self::Images => Self::Build,
            Self::Build => Self::Logs,
            Self::Logs => Self::Containers,
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Self::Containers => Self::Logs,
            Self::Images => Self::Containers,
            Self::Build => Self::Images,
            Self::Logs => Self::Build,
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
    BuildImage,
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

#[derive(Debug)]
pub struct App {
    pub should_quit: bool,
    pub screen: Screen,
    pub modal: Option<Modal>,
    pub containers: Vec<Container>,
    pub images: Vec<Image>,
    pub selected_container: usize,
    pub selected_image: usize,
    pub status: String,
    pub logs: Vec<String>,
    pub input: String,
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
            status: "ready".to_string(),
            logs: Vec::new(),
            input: String::new(),
        }
    }

    pub fn refresh(&mut self) {
        let mut refreshed = true;

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

    pub fn open_modal(&mut self, modal: Modal) {
        self.modal = Some(modal);
        if modal == Modal::BuildImage {
            self.input.clear();
        }
    }

    pub fn close_modal(&mut self) {
        self.modal = None;
        self.input.clear();
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
}
