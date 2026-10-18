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
    export_dialog: Option<view::export_dialog::State>,
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
                self.menu_target = None;
                self.context.status.message = String::from("Open DDM frame archive");
                file_loader::pick_archive()
            }
            Message::L0bSelected(path) => self.load_selected_l0b(path),
            Message::ArchiveSelected(path) => self.load_selected_archive(path),
            Message::ArchiveLoaded(path, result) => self.finish_archive_load(&path, result),
            Message::L0bLoaded(path, result) => self.finish_l0b_load(&path, result),
            Message::Workspace(message) => self.workspace.update(message).map(Message::Workspace),
            Message::ExportL0bRequested => {
                self.open_export_dialog(view::export_dialog::Format::L0b)
            }
            Message::ExportArchiveRequested => {
                self.open_export_dialog(view::export_dialog::Format::Archive)
            }
            Message::ExportDialog(message) => self.update_export_dialog(message),
        }
    }

    fn window_task(&self, command: impl FnOnce(window::Id) -> Task<Message>) -> Task<Message> {
        self.window_id.map_or_else(Task::none, command)
    }

    fn open_l0b(&mut self) -> Task<Message> {
        self.menu_target = None;
        "Open L0B file".clone_into(&mut self.context.status.message);
        file_loader::pick_l0b()
    }

    fn load_selected_l0b(&mut self, path: Option<PathBuf>) -> Task<Message> {
        let Some(path) = path else {
            "Cancelled opening L0b archive".clone_into(&mut self.context.status.message);
            return Task::none();
        };

        tracing::info!(path = %path.display(), "loading selected L0B file");
        self.context.status.message = format!("Opening L0b file {}", path.display());
        file_loader::load_l0b(path)
    }

    fn load_selected_archive(&mut self, path: Option<PathBuf>) -> Task<Message> {
        let Some(path) = path else {
            self.context.status.message = String::from("Cancelled opening DDM frame archive");
            return Task::none();
        };
        tracing::info!(path = %path.display(), "loading selected DDM archive");
        self.context.status.message = format!("Opening DDM archive {}", path.display());
        file_loader::load_archive(path)
    }

    fn finish_l0b_load(
        &mut self,
        path: &Path,
        result: Result<Vec<crate::l0b_packet::L0bPacket>, String>,
    ) -> Task<Message> {
        match result {
            Ok(packets) => {
                let packet_count = packets.len();
                tracing::info!(path = %path.display(), packet_count, "L0B file loaded");
                match self.workspace.open_l0b(path, packets) {
                    Ok(()) => {
                        self.context.status.message =
                            format!("L0B file loaded ({packet_count} packets)");
                        self.context.document.path = Some(path.to_path_buf());
                        Task::none()
                    }
                    Err(error) => {
                        tracing::error!(path = %path.display(), %error, "failed to build L0B viewer");
                        self.context.status.message = format!("Failed to load L0B file: {error}");
                        show_load_error(error.to_string())
                    }
                }
            }
            Err(error) => {
                tracing::error!(path = %path.display(), %error, "failed to load L0B file");
                self.context.status.message = format!("Failed to load L0B file: {error}");
                show_load_error(error)
            }
        }
    }

    fn finish_archive_load(
        &mut self,
        path: &Path,
        result: Result<ciborium::Value, String>,
    ) -> Task<Message> {
        match result {
            Ok(archive) => match self.workspace.open_archive(path, &archive) {
                Ok(()) => {
                    self.context.status.message = String::from("DDM archive loaded");
                    self.context.document.path = Some(path.to_path_buf());
                    Task::none()
                }
                Err(error) => {
                    self.context.status.message = format!("Failed to load DDM archive: {error}");
                    show_load_error(error.to_string())
                }
            },
            Err(error) => {
                self.context.status.message = format!("Failed to load DDM archive: {error}");
                show_load_error(error)
            }
        }
    }

    fn open_export_dialog(&mut self, format: view::export_dialog::Format) -> Task<Message> {
        self.menu_target = None;
        self.export_dialog = Some(view::export_dialog::State::new(format, &self.workspace));
        Task::none()
    }

    fn update_export_dialog(&mut self, message: view::export_dialog::Message) -> Task<Message> {
        let action = self
            .export_dialog
            .as_mut()
            .and_then(|dialog| dialog.update(message, &self.workspace));
        self.handle_export_action(action)
    }

    fn handle_export_action(
        &mut self,
        action: Option<view::export_dialog::ExportAction>,
    ) -> Task<Message> {
        match action {
            Some(view::export_dialog::ExportAction::Close) => self.close_export_dialog(),
            Some(view::export_dialog::ExportAction::Save) => self.open_export_picker(),
            Some(view::export_dialog::ExportAction::Finished(path)) => {
                self.export_path_selected(path)
            }
            Some(view::export_dialog::ExportAction::ExportFinished(result)) => {
                self.export_completed(result)
            }
            None => Task::none(),
        }
    }

    fn close_export_dialog(&mut self) -> Task<Message> {
        self.export_dialog = None;
        Task::none()
    }

    fn open_export_picker(&self) -> Task<Message> {
        let Some(dialog) = &self.export_dialog else {
            return Task::none();
        };
        view::export_dialog::save_picker(dialog, &self.workspace).map(Message::ExportDialog)
    }

    fn export_path_selected(&mut self, path: Option<PathBuf>) -> Task<Message> {
        match path {
            Some(path) => self.write_export(path),
            None => self.cancel_export(),
        }
    }

    fn write_export(&self, path: PathBuf) -> Task<Message> {
        let Some(dialog) = &self.export_dialog else {
            return Task::none();
        };
        view::export_dialog::write_export_task(dialog, &self.workspace, path)
            .map(Message::ExportDialog)
    }

    fn cancel_export(&mut self) -> Task<Message> {
        "Export cancelled".clone_into(&mut self.context.status.message);
        self.close_export_dialog()
    }

    fn export_completed(&mut self, result: Result<usize, String>) -> Task<Message> {
        match result {
            Ok(packet_count) => self.export_succeeded(packet_count),
            Err(error) => self.export_failed(&error),
        }
    }

    fn export_succeeded(&mut self, packet_count: usize) -> Task<Message> {
        tracing::info!(packet_count, "export completed");
        self.context.status.message = format!("Export completed: {packet_count} packets written");
        self.close_export_dialog()
    }

    fn export_failed(&mut self, error: &str) -> Task<Message> {
        self.context.status.message = format!("Export failed: {error}");
        self.close_export_dialog()
    }

    fn view(&self) -> Element<'_, Message> {
        tracing::trace!("rendering application shell");
        view::app_shell(
            &self.context,
            &self.workspace,
            self.menu_target,
            self.export_dialog.as_ref(),
        )
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
