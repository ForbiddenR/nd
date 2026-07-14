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
    Tasks,
    Logs,
}

impl Screen {
    pub fn index(self) -> usize {
        match self {
            Self::Containers => 0,
            Self::Images => 1,
            Self::Build => 2,
            Self::Tasks => 3,
            Self::Logs => 4,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Containers => Self::Images,
            Self::Images => Self::Build,
            Self::Build => Self::Tasks,
            Self::Tasks => Self::Logs,
            Self::Logs => Self::Containers,
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Self::Containers => Self::Logs,
            Self::Images => Self::Containers,
            Self::Build => Self::Images,
            Self::Tasks => Self::Build,
            Self::Logs => Self::Tasks,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modal {
    RemoveContainer,
    RemoveImage,
    ImagePrune,
    BuilderPrune,
    SystemPrune,
    PushImage,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildContextSource {
    Manual,
    Configured,
    Default,
}

impl BuildContextSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Configured => "configured",
            Self::Default => "default",
        }
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    Build,
    Push,
}

impl TaskKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Push => "push",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Running,
    Succeeded,
    Failed,
}

impl TaskStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Succeeded => "done",
            Self::Failed => "failed",
        }
    }
}

/// A backgrounded streaming nerdctl operation (build or push) shown on the
/// Tasks screen. Owns its output lines, status, event receiver, and cancel
/// handle. Multiple tasks can run at once; each is drained every tick by
/// `poll_task_events`.
#[derive(Debug)]
pub struct Task {
    pub kind: TaskKind,
    pub title: String,
    pub status: TaskStatus,
    pub lines: Vec<String>,
    events: Option<Receiver<docker::ProgressEvent>>,
    cancel: Option<docker::CancelHandle>,
}

impl Task {
    pub fn new(kind: TaskKind, title: String) -> Self {
        Self {
            kind,
            title,
            status: TaskStatus::Running,
            lines: Vec::new(),
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
        self.status = TaskStatus::Running;
    }

    pub fn is_running(&self) -> bool {
        self.status == TaskStatus::Running
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

    /// Called when a Finished event arrives. Records the final status and
    /// drops the receiver and cancel handle.
    pub fn handle_finished(&mut self, success: bool) {
        self.status = if success {
            TaskStatus::Succeeded
        } else {
            TaskStatus::Failed
        };
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
        if self.is_running() {
            return false;
        }
        self.lines.clear();
        true
    }

    /// Append a line with a bounded cap so a very verbose build/push can't
    /// grow the buffer without limit.
    pub fn push_line(&mut self, line: String) {
        if line.trim().is_empty() {
            return;
        }

        self.lines.push(line);

        if self.lines.len() > 2_000 {
            let remove_count = self.lines.len() - 2_000;
            self.lines.drain(0..remove_count);
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
    pub editing_context: bool,
    context_draft: String,
    pub configs: Vec<config::Config>,
    pub tasks: Vec<Task>,
    pub selected_task: usize,
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
            editing_context: false,
            context_draft: String::new(),
            configs: vec![],
            tasks: Vec::new(),
            selected_task: 0,
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
            Screen::Tasks if !self.tasks.is_empty() => {
                self.selected_task = self.selected_task.saturating_sub(1);
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
            Screen::Tasks if !self.tasks.is_empty() => {
                self.selected_task =
                    (self.selected_task + 1).min(self.tasks.len().saturating_sub(1));
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

    pub fn effective_build_context(&self) -> &str {
        let manual = self.input.trim();
        if !manual.is_empty() {
            manual
        } else {
            self.selected_config()
                .map(config::Config::context_or_default)
                .unwrap_or(".")
        }
    }

    pub fn build_context_source(&self) -> BuildContextSource {
        if !self.input.trim().is_empty() {
            BuildContextSource::Manual
        } else if self
            .selected_config()
            .and_then(|config| config.context.as_deref())
            .is_some_and(|context| !context.trim().is_empty())
        {
            BuildContextSource::Configured
        } else {
            BuildContextSource::Default
        }
    }

    pub fn context_draft(&self) -> &str {
        &self.context_draft
    }

    pub fn begin_context_edit(&mut self) {
        self.context_draft = self.effective_build_context().to_string();
        self.editing_context = true;
        self.status = "editing build context".to_string();
    }

    pub fn push_context_char(&mut self, character: char) {
        self.context_draft.push(character);
    }

    pub fn pop_context_char(&mut self) {
        self.context_draft.pop();
    }

    pub fn clear_context_draft(&mut self) {
        self.context_draft.clear();
    }

    pub fn commit_context_edit(&mut self) {
        self.input = self.context_draft.trim().to_string();
        self.context_draft.clear();
        self.editing_context = false;
        self.status = if self.input.is_empty() {
            "build context override cleared"
        } else {
            "build context override saved"
        }
        .to_string();
    }

    pub fn cancel_context_edit(&mut self) {
        self.context_draft.clear();
        self.editing_context = false;
        self.status = "build context edit cancelled".to_string();
    }

    pub fn selected_task(&self) -> Option<&Task> {
        self.tasks.get(self.selected_task)
    }

    pub fn open_modal(&mut self, modal: Modal) {
        self.modal = Some(modal);
    }

    pub fn close_modal(&mut self) {
        self.modal = None;
    }

    /// Returns true if any task is still running in the background.
    pub fn has_running_tasks(&self) -> bool {
        self.tasks.iter().any(|task| task.is_running())
    }

    /// Kill any running tasks so they don't survive the TUI as orphaned
    /// `nerdctl` processes. Called on quit.
    pub fn cancel_running_tasks(&mut self) {
        for task in &mut self.tasks {
            task.cancel();
        }
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

    /// Start a build task and select it on the Tasks screen.
    pub fn start_build(
        &mut self,
        receiver: Receiver<docker::ProgressEvent>,
        cancel: docker::CancelHandle,
        title: String,
    ) {
        self.start_task(TaskKind::Build, receiver, cancel, title);
        self.status = "build started".to_string();
    }

    /// Start a push task and select it on the Tasks screen.
    pub fn start_push(
        &mut self,
        receiver: Receiver<docker::ProgressEvent>,
        cancel: docker::CancelHandle,
        title: String,
    ) {
        self.start_task(TaskKind::Push, receiver, cancel, title);
        self.status = "push started".to_string();
    }

    fn start_task(
        &mut self,
        kind: TaskKind,
        receiver: Receiver<docker::ProgressEvent>,
        cancel: docker::CancelHandle,
        title: String,
    ) {
        let mut task = Task::new(kind, title);
        task.start(receiver, cancel);
        self.tasks.push(task);
        // Select the newly started task so its output is visible immediately.
        self.selected_task = self.tasks.len() - 1;
    }

    /// Drain events from every task and apply results. Called each tick from
    /// the main loop so streaming output updates without blocking. Events are
    /// drained into per-task buffers first so applying a Finished result (which
    /// may call `self.refresh` / `self.set_error`) doesn't conflict with the
    /// borrow used to drain.
    pub fn poll_task_events(&mut self) {
        let mut drained: Vec<(usize, Vec<docker::ProgressEvent>)> = Vec::new();
        for (index, task) in self.tasks.iter_mut().enumerate() {
            let events = task.drain_events();
            if !events.is_empty() {
                drained.push((index, events));
            }
        }

        for (index, events) in drained {
            for event in events {
                match event {
                    docker::ProgressEvent::Line(line) => self.tasks[index].push_line(line),
                    docker::ProgressEvent::Finished { success, message } => {
                        self.tasks[index].push_line(message.clone());
                        self.tasks[index].handle_finished(success);
                        if success {
                            self.status = format!("{} completed", self.tasks[index].kind.label());
                            self.refresh();
                        } else {
                            self.set_error(message);
                        }
                    }
                }
            }
        }
    }

    /// Cancel the selected task if it is still running.
    pub fn cancel_selected_task(&mut self) {
        if let Some(task) = self.tasks.get_mut(self.selected_task)
            && task.is_running()
        {
            task.cancel();
            self.status = "cancelling task...".to_string();
        }
    }

    /// Remove the selected task from the list. A still-running task is
    /// cancelled first so its child process isn't orphaned - dropping a
    /// `CancelHandle` does nothing on its own, only `cancel` sends the signal.
    pub fn delete_selected_task(&mut self) {
        if self.tasks.is_empty() {
            return;
        }

        let index = self.selected_task.min(self.tasks.len() - 1);
        if self.tasks[index].is_running() {
            self.tasks[index].cancel();
        }
        self.tasks.remove(index);
        self.clamp_task_selection();
        self.status = "task removed".to_string();
    }

    /// Clear the selected task's output lines, if it has finished.
    pub fn clear_selected_task_lines(&mut self) {
        if let Some(task) = self.tasks.get_mut(self.selected_task)
            && task.clear_lines()
        {
            self.status = "task output cleared".to_string();
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

    fn clamp_task_selection(&mut self) {
        if self.tasks.is_empty() {
            self.selected_task = 0;
        } else {
            self.selected_task = self.selected_task.min(self.tasks.len() - 1);
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

#[cfg(test)]
mod tests {
    use super::{App, BuildContextSource};
    use crate::config::Config;

    fn config(context: Option<&str>) -> Config {
        Config {
            context: context.map(str::to_string),
            ..Config::default()
        }
    }

    #[test]
    fn build_context_uses_manual_configured_then_default_precedence() {
        let mut app = App::new();
        assert_eq!(app.effective_build_context(), ".");
        assert_eq!(app.build_context_source(), BuildContextSource::Default);

        app.configs = vec![config(Some(" ./first ")), config(Some("./second"))];
        assert_eq!(app.effective_build_context(), "./first");
        assert_eq!(app.build_context_source(), BuildContextSource::Configured);

        app.selected_build_tag = 1;
        assert_eq!(app.effective_build_context(), "./second");

        app.input = " ./manual ".to_string();
        assert_eq!(app.effective_build_context(), "./manual");
        assert_eq!(app.build_context_source(), BuildContextSource::Manual);
    }

    #[test]
    fn blank_contexts_fall_back_to_default() {
        let mut app = App::new();
        app.configs = vec![config(Some("   "))];
        app.input = "\t".to_string();

        assert_eq!(app.effective_build_context(), ".");
        assert_eq!(app.build_context_source(), BuildContextSource::Default);
    }

    #[test]
    fn context_edit_commit_trims_and_empty_commit_clears_override() {
        let mut app = App::new();
        app.configs = vec![config(Some("./configured"))];
        app.begin_context_edit();
        app.clear_context_draft();
        for character in "  ./manual path  ".chars() {
            app.push_context_char(character);
        }
        app.commit_context_edit();

        assert_eq!(app.input, "./manual path");
        assert_eq!(app.effective_build_context(), "./manual path");
        assert!(!app.editing_context);

        app.begin_context_edit();
        app.clear_context_draft();
        app.commit_context_edit();

        assert!(app.input.is_empty());
        assert_eq!(app.effective_build_context(), "./configured");
    }

    #[test]
    fn cancelling_context_edit_preserves_manual_override() {
        let mut app = App::new();
        app.input = "./original".to_string();
        app.begin_context_edit();
        app.push_context_char('x');
        app.cancel_context_edit();

        assert_eq!(app.input, "./original");
        assert_eq!(app.effective_build_context(), "./original");
        assert!(app.context_draft().is_empty());
        assert!(!app.editing_context);
    }
}
