pub mod layout;
pub mod message;

use std::sync::Arc;

use iced::{
    Element, Length, Task,
    widget::{Button, button, container, row, space, text},
    window,
};
use iced_aw::{
    Menu, MenuBar,
    menu::{self, Item},
    menu_bar, menu_items,
    tabs::Tabs,
};
use layout::layout;
use message::Message;
use prodash::NestedProgress;
use rfd::{AsyncMessageDialog, MessageButtons, MessageLevel};
use tracing::{debug, error, trace};

use crate::{
    L, M, S, XL, XS,
    subsystem::file::{DataType, FileType, load_l0b},
    ui::{
        component::workspace::{self, Workspace},
        widget::progress_bar::progress_bar,
    },
};

pub struct App {
    window_id: Option<window::Id>,
    workspace: Workspace,
    progress: Arc<prodash::tree::Root>,
    progress_tasks: Vec<(String, f32)>,
}

impl App {
    pub fn new(progress: Arc<prodash::tree::Root>) -> (Self, Task<Message>) {
        let workspace = Workspace::new();
        (
            Self {
                window_id: None,
                workspace,
                progress,
                progress_tasks: vec![],
            },
            window::latest().map(Message::WindowId),
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::CloseWindow => match self.window_id {
                Some(id) => window::close(id),
                None => Task::none(),
            },

            Message::Minimize => match self.window_id {
                Some(id) => window::minimize(id, true),
                None => Task::none(),
            },

            Message::Maximize => match self.window_id {
                Some(id) => window::toggle_maximize(id),
                None => Task::none(),
            },

            Message::DragWindow => match self.window_id {
                Some(id) => window::drag(id),
                None => Task::none(),
            },
            Message::WindowId(id) => {
                self.window_id = id;
                Task::none()
            }
            Message::Workspace(message) => self.workspace.update(message).map(Message::Workspace),
            Message::Noop => Task::none(),
            Message::OpenFile(file_type) => {
                let mut progress = self.progress.add_child("Open files");
                Task::future(async move { open_file(file_type, &mut progress).await })
                    .then(std::convert::identity)
            }
            Message::DataLoaded(data_type) => Task::none(),
            Message::Error(error) => Task::discard(Task::future(
                AsyncMessageDialog::new()
                    .set_buttons(MessageButtons::Ok)
                    .set_level(MessageLevel::Error)
                    .set_title("DDM Workbench has encountered an error")
                    .set_description(format!("{error}"))
                    .show(),
            )),
            Message::ProgressTick => {
                self.progress_tasks = get_progress_tasks_with_percentage(self.progress.clone()).collect();
                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let window_title = "DDM Workbench".to_owned();
        let menu_bar = menu();
        let main_content = self.workspace.view().map(Message::Workspace);
        let status_bar = row(self
            .progress_tasks
            .iter()
            .map(|(name, fraction)| progress_bar(name.clone(), *fraction)));
        layout(window_title, menu_bar, main_content, status_bar)
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        iced::time::every(std::time::Duration::from_millis(30)).map(|_| Message::ProgressTick)
    }
}

async fn open_file<P>(file_type: FileType, progress: &mut P) -> Task<Message>
where
    P: NestedProgress,
    P::SubProgress: 'static,
{
    match file_type {
        FileType::L0b => {
            let handles = match load_l0b(progress).await {
                Ok(handles) => handles,
                Err(e) => {
                    debug!("Error selecting files {e}");
                    return Task::none();
                }
            };
            let tasks = handles.map(|handle| {
                let handle = match handle {
                    Ok(handle) => handle,
                    Err(e) => {
                        error!("Error opening file {e}");
                        return Task::done(Message::Error(e.to_string()));
                    }
                };
                Task::perform(handle, |data| match data {
                    Ok(Ok(data)) => Message::DataLoaded(DataType::L0b(data)),
                    Ok(Err(e)) => {
                        error!("Error during L0b load: {e}");
                        Message::Error(e.to_string())
                    }
                    Err(e) => {
                        error!("Error during L0b load: {e}");
                        Message::Error(e.to_string())
                    }
                })
            });
            Task::batch(tasks)
        }
    }
}

fn get_progress_tasks_with_percentage(
    progress: Arc<prodash::tree::Root>,
) -> impl Iterator<Item = (String, f32)> {
    let mut out = vec![];
    trace!("Updating progress tasks");
    progress.sorted_snapshot(&mut out);

    out.iter()
        .filter_map(|(key, task)| {
            let percentage = task
                .progress
                .as_ref()
                .and_then(|value| value.fraction())?;

            let name = out
                .iter()
                .find(|(parent_key, _)| {
                    parent_key.level().checked_add(2) == Some(key.level())
                        && key.shares_parent_with(parent_key, parent_key.level())
                })
                .map(|(_, parent)| parent.name.as_str())
                .filter(|name| !name.is_empty())
                .unwrap_or(task.name.as_str())
                .to_owned();
            trace!(name=name, percentage=percentage, "Found progress task");
            Some((name, percentage))
        })
        .collect::<Vec<_>>()
        .into_iter()
}

fn menu_button<'a>(content: impl Into<Element<'a, Message>>) -> Button<'a, Message> {
    button(content).width(Length::Fill)
}

fn menu<'a>() -> Element<'a, Message> {
    menu_bar!((
        button("File").on_press(Message::Noop),
        Menu::new(menu_items!(
            (
                menu_button("Open").on_press(Message::Noop),
                Menu::new(menu_items!(Item::new(
                    menu_button("L0b data").on_press(Message::OpenFile(FileType::L0b))
                ),))
                .width(180.0)
            ),
            Item::new(menu_button("File").on_press(Message::Noop)),
            Item::new(menu_button("File").on_press(Message::Noop)),
        ))
        .offset(XS)
        .width(180.0)
        .offset(XS)
    ))
    .into()
}
