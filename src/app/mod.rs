//! Iced-based graphical application for browsing and analyzing DDM data.

mod context;
mod file_loader;
mod message;
mod style;
mod view;
mod workspace;

use context::AppContext;
use iced::{
    Element, Font, Result as IcedResult, Size, Subscription, Task, Theme, application, time, window,
};
use message::{MenuTarget, Message};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

/// Starts the GUI application. `main.rs` deliberately stays as a tiny entry point.
pub fn run() -> IcedResult {
    tracing::debug!("building Iced application");
    application(
        || {
            (
                App::default(),
                window::oldest().map(Message::WindowIdResolved),
            )
        },
        App::update,
        App::view,
    )
    .subscription(App::subscription)
    .title("DDM Workbench")
    .theme(|_: &App| {
        let mut palette = Theme::Dark.palette();
        palette.primary = palette.background;
        Theme::custom("DDM Workbench", palette)
    })
    .default_font(Font::DEFAULT)
    .window(window::Settings {
        size: Size::new(1200.0, 800.0),
        decorations: false,
        ..window::Settings::default()
    })
    .run()
}

/// The top-level application state.
///
/// Keep persistent UI/document state here; view modules only build widgets.
#[derive(Debug, Default)]
struct App {
    context: AppContext,
    workspace: workspace::Workspace,
    window_id: Option<window::Id>,
    menu_target: Option<MenuTarget>,
}

impl App {
    fn subscription(&self) -> Subscription<Message> {
        let window_opened = window::events().filter_map(|(id, event)| {
            matches!(event, window::Event::Opened { .. })
                .then_some(Message::WindowIdResolved(Some(id)))
        });

        let playback = if self.workspace.is_playing() {
            time::every(Duration::from_millis(100))
                .map(|_| Message::Workspace(workspace::Message::PlaybackTick))
        } else {
            Subscription::none()
        };

        Subscription::batch([window_opened, playback])
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::MenuHover(target) => {
                self.menu_target = target;
                Task::none()
            }
            Message::WindowIdResolved(window_id) => {
                self.window_id = window_id;
                Task::none()
            }
            Message::WindowDrag => self.window_task(window::drag),
            Message::WindowMinimize => self.window_task(|id| window::minimize(id, true)),
            Message::WindowToggleMaximize => self.window_task(window::toggle_maximize),
            Message::WindowClose => self.window_task(window::close),
            Message::WindowResize(direction) => {
                self.window_task(|id| window::drag_resize(id, direction))
            }
            Message::OpenL0bRequested => {
                tracing::info!("user requested an L0B file");
                self.open_l0b()
            }
            Message::OpenArchiveRequested => {
                tracing::warn!("DDM frame archive opening is not available yet");
                self.set_unavailable_action("Open DDM frame archive");
                Task::none()
            }
            Message::L0bSelected(path) => self.load_selected_l0b(path),
            Message::L0bLoaded(path, result) => self.finish_l0b_load(&path, result),
            Message::Workspace(message) => self.workspace.update(message).map(Message::Workspace),
            Message::ExportL0bRequested => {
                tracing::warn!("L0B export is not available yet");
                self.set_unavailable_action("Export L0b file");
                Task::none()
            }
            Message::ExportArchiveRequested => {
                tracing::warn!("DDM frame archive export is not available yet");
                self.set_unavailable_action("Export DDM frame archive");
                Task::none()
            }
        }
    }

    fn window_task(&self, command: impl FnOnce(window::Id) -> Task<Message>) -> Task<Message> {
        self.window_id.map_or_else(Task::none, command)
    }

    fn open_l0b(&mut self) -> Task<Message> {
        self.menu_target = None;
        "Open L0b file".clone_into(&mut self.context.status.message);
        file_loader::pick_l0b()
    }

    fn load_selected_l0b(&mut self, path: Option<PathBuf>) -> Task<Message> {
        let Some(path) = path else {
            "Cancelled opening L0b archive".clone_into(&mut self.context.status.message);
            return Task::none();
        };

        tracing::info!(path = %path.display(), "loading selected L0B file");
        self.context.status.message = format!("Opening L0b file {}", path.display());
        self.context.document.path = Some(path.clone());
        file_loader::load_l0b(path)
    }

    fn finish_l0b_load(
        &mut self,
        path: &Path,
        result: Result<Vec<crate::l0b_packet::L0bPacket>, String>,
    ) -> Task<Message> {
        match result {
            Ok(packets) => {
                tracing::info!(path = %path.display(), packet_count = packets.len(), "L0B file loaded");
                self.context.status.message =
                    format!("L0b file loaded ({} packets)", packets.len());
                self.context.document.path = Some(path.to_path_buf());
                self.workspace.open_l0b(path, packets);
                Task::none()
            }
            Err(error) => {
                tracing::error!(path = %path.display(), %error, "failed to load L0B file");
                show_load_error(error)
            }
        }
    }

    fn set_unavailable_action(&mut self, action: &str) {
        self.menu_target = None;
        self.context.status.message = format!("{action} selected (file picker not wired up yet)");
    }

    fn view(&self) -> Element<'_, Message> {
        tracing::trace!("rendering application shell");
        view::app_shell(&self.context, &self.workspace, self.menu_target)
    }
}

fn show_load_error(error: String) -> Task<Message> {
    Task::future(async move {
        rfd::AsyncMessageDialog::new()
            .set_title("Error Loading L0b Archive")
            .set_description(format!("Failed to parse or read the file:\n{error}"))
            .set_level(rfd::MessageLevel::Error)
            .show()
            .await;
    })
    .discard()
}
