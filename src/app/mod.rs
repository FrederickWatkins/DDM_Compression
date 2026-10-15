mod context;
mod file_loader;
mod message;
mod view;
mod workspace;

use context::AppContext;
use iced::{Element, Subscription, Task, Theme};
use message::{MenuTarget, Message};
use std::time::Duration;

/// Starts the GUI application. `main.rs` deliberately stays as a tiny entry point.
pub fn run() -> iced::Result {
    iced::application(
        || {
            (
                App::default(),
                iced::window::oldest().map(Message::WindowIdResolved),
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
    .default_font(iced::Font::DEFAULT)
    .window(iced::window::Settings {
        size: iced::Size::new(1200.0, 800.0),
        decorations: false,
        ..iced::window::Settings::default()
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
    window_id: Option<iced::window::Id>,
    menu_target: Option<MenuTarget>,
}

impl App {
    fn subscription(&self) -> Subscription<Message> {
        let window_opened = iced::window::events().filter_map(|(id, event)| {
            matches!(event, iced::window::Event::Opened { .. })
                .then_some(Message::WindowIdResolved(Some(id)))
        });

        let playback = if self.workspace.is_playing() {
            iced::time::every(Duration::from_millis(100))
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
            Message::WindowDrag => self.window_id.map_or_else(Task::none, iced::window::drag),
            Message::WindowMinimize => self
                .window_id
                .map_or_else(Task::none, |id| iced::window::minimize(id, true)),
            Message::WindowToggleMaximize => self
                .window_id
                .map_or_else(Task::none, iced::window::toggle_maximize),
            Message::WindowClose => self.window_id.map_or_else(Task::none, iced::window::close),
            Message::WindowResize(direction) => self
                .window_id
                .map_or_else(Task::none, |id| iced::window::drag_resize(id, direction)),
            Message::OpenL0bRequested => {
                self.menu_target = None;
                self.context.status.message = "Open L0b file".to_owned();
                file_loader::pick_l0b()
            }
            Message::OpenArchiveRequested => {
                self.menu_target = None;
                self.context.status.message =
                    "Open DDM frame archive selected (file picker not wired up yet)".to_owned();

                Task::none()
            }
            Message::L0bSelected(Some(path)) => {
                self.context.status.message = format!("Opening L0b file {}", path.display());
                self.context.document.path = Some(path.clone());
                file_loader::load_l0b(path)
            }

            Message::L0bSelected(None) => {
                self.context.status.message = format!("Cancelled opening L0b archive");
                Task::none()
            }
            Message::L0bLoaded(path, Ok(l0b_packets)) => {
                self.context.status.message =
                    format!("L0b file loaded ({} packets)", l0b_packets.len());
                self.context.document.path = Some(path.clone());
                self.workspace.open_l0b(path, l0b_packets);
                Task::none()
            }
            Message::Workspace(message) => self.workspace.update(message).map(Message::Workspace),
            Message::L0bLoaded(_, Err(err)) => Task::future(async move {
                rfd::AsyncMessageDialog::new()
                    .set_title("Error Loading L0b Archive")
                    .set_description(&format!("Failed to parse or read the file:\n{}", err))
                    .set_level(rfd::MessageLevel::Error)
                    .show()
                    .await;
            })
            .discard(),
            Message::ExportL0bRequested => {
                self.menu_target = None;
                self.context.status.message =
                    "Export L0b file selected (file picker not wired up yet)".to_owned();
                Task::none()
            }
            Message::ExportArchiveRequested => {
                self.menu_target = None;
                self.context.status.message =
                    "Export DDM frame archive selected (file picker not wired up yet)".to_owned();
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        view::app_shell(&self.context, &self.workspace, self.menu_target)
    }
}
