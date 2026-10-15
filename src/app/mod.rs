mod context;
mod file_loader;
mod message;
mod view;

use context::AppContext;
use iced::{Element, Task, Theme};
use message::Message;
use rfd::FileDialog;

/// Starts the GUI application. `main.rs` deliberately stays as a tiny entry point.
pub fn run() -> iced::Result {
    iced::application(App::default, App::update, App::view)
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
}

impl App {
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
                self.context.status.message = format!("Open L0b file {}", path.display());
                file_loader::load_l0b(path)
            }

            Message::L0bSelected(None) => {
                self.context.status.message = format!("Cancelled opening L0b archive");
                Task::none()
            }
            Message::L0bLoaded(Ok(l0b_packets)) => {
                self.context.status.message =
                    format!("L0b file loaded ({} packets)", l0b_packets.len());

                Task::none()
            }
            Message::L0bLoaded(Err(err)) => Task::perform(
                async move {
                    rfd::AsyncMessageDialog::new()
                        .set_title("Error Loading L0B Archive")
                        .set_description(&format!("Failed to parse or read the file:\n{}", err))
                        .set_level(rfd::MessageLevel::Error)
                        .show().await;
                },
                |_| Message::None,
            ),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        view::app_shell(&self.context)
    }
}
