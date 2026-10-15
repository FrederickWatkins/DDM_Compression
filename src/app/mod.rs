mod context;
mod file_loader;
mod message;
mod view;
mod workspace;

use context::AppContext;
use iced::{Element, Subscription, Task, Theme};
use message::Message;
use std::time::Duration;

/// Starts the GUI application. `main.rs` deliberately stays as a tiny entry point.
pub fn run() -> iced::Result {
    iced::application(App::default, App::update, App::view)
        .subscription(App::subscription)
        .title("DDM Workbench")
        .theme(|_: &App| Theme::Dark)
        .window_size(iced::Size::new(1200.0, 800.0))
        .run()
}

/// The top-level application state.
///
/// Keep persistent UI/document state here; view modules only build widgets.
#[derive(Debug, Default)]
struct App {
    context: AppContext,
    workspace: workspace::Workspace,
}

impl App {
    fn subscription(&self) -> Subscription<Message> {
        if self.workspace.is_playing() {
            iced::time::every(Duration::from_millis(100))
                .map(|_| Message::Workspace(workspace::Message::PlaybackTick))
        } else {
            Subscription::none()
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::None => Task::none(),
            Message::OpenL0bRequested => {
                self.context.status.message = "Open L0b file".to_owned();
                file_loader::pick_l0b()
            }
            Message::OpenArchiveRequested => {
                self.context.status.message =
                    "Open DDM frame archive selected (file picker not wired up yet)".to_owned();

                Task::none()
            }
            Message::L0bSelected(Some(path)) => {
                self.context.status.message = format!("Opening L0B file {}", path.display());
                self.context.document.path = Some(path.clone());
                file_loader::load_l0b(path)
            }

            Message::L0bSelected(None) => {
                self.context.status.message = format!("Cancelled opening L0b archive");
                Task::none()
            }
            Message::L0bLoaded(path, Ok(l0b_packets)) => {
                self.context.status.message =
                    format!("L0B file loaded ({} packets)", l0b_packets.len());
                self.context.document.path = Some(path.clone());
                self.workspace.open_l0b(path, l0b_packets);
                Task::none()
            }
            Message::Workspace(message) => self.workspace.update(message).map(Message::Workspace),
            Message::L0bLoaded(_, Err(err)) => Task::perform(
                async move {
                    rfd::AsyncMessageDialog::new()
                        .set_title("Error Loading L0B Archive")
                        .set_description(&format!("Failed to parse or read the file:\n{}", err))
                        .set_level(rfd::MessageLevel::Error)
                        .show()
                        .await;
                },
                |_| Message::None,
            ),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        view::app_shell(&self.context, &self.workspace)
    }
}
