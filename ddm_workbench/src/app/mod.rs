//! Main application logic
mod layout;
mod message;
mod style;

use std::sync::Arc;

use iced::{
    Element, Font, Length, Task,
    widget::{Button, button, container, pane_grid, row, space, text},
    window,
};
use iced_aw::{Menu, MenuBar, TabLabel, menu::Item, tabs::Tabs};
use layout::layout;
use message::Message;
use prodash::NestedProgress;
use rfd::{AsyncMessageDialog, MessageButtons, MessageLevel};
use slotmap::{DefaultKey, SlotMap};
use tracing::{debug, error, trace};

use crate::{
    L, M, XL, XS, app::style::sidebar_style, subsystem::file::{DataType, FileType, load_l0b}, ui::{component::workspace::Workspace, widget::progress_bar},
};

enum WorkspacePane {
    Main,
    Sidebar,
}

/// Top level app
pub struct App {
    window_id: Option<window::Id>,
    progress: Arc<prodash::tree::Root>,
    progress_tasks: Vec<(String, f32)>,
    workspaces: SlotMap<DefaultKey, Workspace>,
    active_workspace: Option<DefaultKey>,
    panes: pane_grid::State<WorkspacePane>,
}

impl App {
    /// Create new app tracking task progress in `progress`
    pub fn new(progress: Arc<prodash::tree::Root>) -> (Self, Task<Message>) {
        let progress_tasks = vec![];
        let workspaces = SlotMap::new();
        let active_workspace = None;
        let panes = pane_grid::State::with_configuration(pane_grid::Configuration::Split {
            axis: pane_grid::Axis::Vertical,
            ratio: 0.25,
            a: Box::new(pane_grid::Configuration::Pane(WorkspacePane::Sidebar)),
            b: Box::new(pane_grid::Configuration::Pane(WorkspacePane::Main)),
        });
        (
            Self {
                window_id: None,
                progress,
                progress_tasks,
                workspaces,
                active_workspace,
                panes,
            },
            window::latest().map(Message::WindowId),
        )
    }

    /// Update app state
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::CloseWindow => {
                debug!(window_id = ?self.window_id, "Close window requested");
                if let Some(id) = self.window_id {
                    debug!(window_id = ?id, "Closing window");
                    window::close(id)
                } else {
                    debug!("Cannot close window; window ID is unavailable");
                    Task::none()
                }
            }
            Message::MinimizeWindow => {
                debug!(window_id = ?self.window_id, "Minimize window requested");
                if let Some(id) = self.window_id {
                    debug!(window_id = ?id, "Minimizing window");
                    window::minimize(id, true)
                } else {
                    debug!("Cannot minimize window; window ID is unavailable");
                    Task::none()
                }
            }
            Message::MaximizeWindow => {
                debug!(window_id = ?self.window_id, "Toggle window maximize requested");
                if let Some(id) = self.window_id {
                    debug!(window_id = ?id, "Toggling window maximize state");
                    window::toggle_maximize(id)
                } else {
                    debug!("Cannot toggle window maximize state; window ID is unavailable");
                    Task::none()
                }
            }
            Message::DragWindow => {
                debug!(window_id = ?self.window_id, "Drag window requested");
                if let Some(id) = self.window_id {
                    debug!(window_id = ?id, "Dragging window");
                    window::drag(id)
                } else {
                    debug!("Cannot drag window; window ID is unavailable");
                    Task::none()
                }
            }
            Message::WindowId(id) => {
                debug!(window_id = ?id, "Window ID updated");
                self.window_id = id;
                Task::none()
            }
            Message::Workspace { tab, message } => {
                if let Some(workspace) = self.workspaces.get_mut(tab) {
                    workspace
                        .update(message)
                        .map(move |m| Message::Workspace { tab, message: m })
                } else {
                    error!(tab = ?tab, message = ?message, "Cannot update workspace; tab not found");
                    Task::none()
                }
            }
            Message::Noop => {
                debug!("No-op message received");
                Task::none()
            }
            Message::OpenFile(file_type) => {
                debug!(file_type = ?file_type, "Open file requested");
                let mut progress = self.progress.add_child("Open files");
                Task::future(async move { open_file(file_type, &mut progress).await })
                    .then(std::convert::identity)
            }
            Message::DataLoaded { name, data } => {
                debug!(name = %name, "Data loaded; creating workspace");
                self.active_workspace = Some(self.workspaces.insert(Workspace::new(name, data)));
                Task::none()
            }
            Message::Error(error) => {
                error!(error = %error, "Application error received");
                Task::discard(Task::future(
                    AsyncMessageDialog::new()
                        .set_buttons(MessageButtons::Ok)
                        .set_level(MessageLevel::Error)
                        .set_title("DDM Workbench has encountered an error")
                        .set_description(error)
                        .show(),
                ))
            }
            Message::ProgressTick => {
                self.progress_tasks = get_progress_tasks_with_percentage(&self.progress).collect();
                Task::none()
            }
            Message::SelectTab(tab) => {
                debug!(tab = ?tab, "Workspace tab selected");
                self.active_workspace = Some(tab);
                Task::none()
            }
            Message::CloseTab(tab) => {
                debug!(tab = ?tab, "Workspace tab closed");
                self.workspaces.remove(tab);
                Task::none()
            }
            Message::ResizeSidebar(event) => {
                trace!(event = ?event, "Sidebar resize event");
                self.panes.resize(event.split, event.ratio);
                Task::none()
            }
        }
    }

    /// View app
    pub fn view(&self) -> Element<'_, Message> {
        let window_title = "DDM Workbench".to_owned();
        let menu_bar = menu();

        let main_content = pane_grid::PaneGrid::new(&self.panes, |_, pane, _| {
            let content = match pane {
                WorkspacePane::Sidebar => self.sidebar_content(),
                WorkspacePane::Main => self.main_content(),
            };

            pane_grid::Content::new(content)
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .spacing(5.0)
        .on_resize(5, Message::ResizeSidebar);
        let status_bar = row(self
            .progress_tasks
            .iter()
            .map(|(name, fraction)| progress_bar(name.clone(), *fraction)));
        layout(window_title, menu_bar, main_content, status_bar)
    }

    fn main_content(&self) -> Element<'_, Message> {
        let mut tab_manager = Tabs::new(Message::SelectTab)
            .width(Length::Fill)
            .height(Length::Fill)
            .tab_bar_height(Length::Fixed(L))
            .close_size(L)
            .tab_bar_width(Length::Fixed(200.0 * self.workspaces.len() as f32))
            .text_font(Font::MONOSPACE)
            .text_size(M)
            .on_close(Message::CloseTab);
        for tab in &self.workspaces {
            tab_manager = tab_manager.push(
                tab.0,
                TabLabel::Text(tab.1.name().clone()),
                tab.1.view().map(move |m| Message::Workspace {
                    tab: tab.0,
                    message: m,
                }),
            );
        }
        if let Some(active_tab) = self.active_workspace {
            tab_manager = tab_manager.set_active_tab(&active_tab);
        }
        container(tab_manager)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn sidebar_content(&self) -> Element<'_, Message> {
        container(if let Some(active_tab) = self.active_workspace {
            if let Some(workspace) = self.workspaces.get(active_tab) {
                workspace
                    .sidebar_content()
                    .map(move |message| Message::Workspace {
                        tab: active_tab,
                        message,
                    })
            } else {
                space().into()
            }
        } else {
            space().into()
        }).style(sidebar_style).into()
    }

    /// Required runtime subscriptions
    pub fn subscription(&self) -> iced::Subscription<Message> {
        let mut subs = vec![
            iced::time::every(std::time::Duration::from_millis(30)).map(|_| Message::ProgressTick),
        ];
        if let Some(active_tab) = self.active_workspace
            && let Some(active_workspace) = self.workspaces.get(active_tab) {
                subs.push(
                    active_workspace
                        .subscription()
                        .with(active_tab)
                        .map(|(tab, message)| Message::Workspace { tab, message }),
                );
            }

        iced::Subscription::batch(subs)
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
                    Ok(Ok(data)) => Message::DataLoaded {
                        name: data.0,
                        data: DataType::L0b(Arc::new(data.1)),
                    },
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
    progress: &Arc<prodash::tree::Root>,
) -> impl Iterator<Item = (String, f32)> {
    let mut out = vec![];
    trace!("Updating progress tasks");
    progress.sorted_snapshot(&mut out);

    out.iter()
        .filter_map(|(key, task)| {
            let percentage = task
                .progress
                .as_ref()
                .and_then(prodash::progress::Value::fraction)?;

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
            trace!(name = name, percentage = percentage, "Found progress task");
            Some((name, percentage))
        })
        .collect::<Vec<_>>()
        .into_iter()
}

fn menu_button<'a>(content: String) -> Button<'a, Message> {
    button(text(content).size(M).font(Font::MONOSPACE)).width(Length::Fill)
}

fn open_menu<'a>() -> Menu<'a, Message, iced::Theme, iced::Renderer> {
    Menu::new(vec![Item::new(
        menu_button("L0b data".to_owned()).on_press(Message::OpenFile(FileType::L0b)),
    )])
    .width(180.0)
}

fn file_menu<'a>() -> Menu<'a, Message, iced::Theme, iced::Renderer> {
    Menu::new(vec![
        // Use Item::with_menu for items that open a nested submenu
        Item::with_menu(
            menu_button("Open".to_owned()).on_press(Message::Noop),
            open_menu(),
        ),
        // Regular leaf items use Item::new
        Item::new(menu_button("File".to_owned()).on_press(Message::Noop)),
        Item::new(menu_button("File".to_owned()).on_press(Message::Noop)),
    ])
    .offset(XS)
    .width(4.0 * XL)
}

fn menu<'a>() -> Element<'a, Message> {
    MenuBar::new(vec![Item::with_menu(
        button(text("File").size(M).font(Font::MONOSPACE)).on_press(Message::Noop),
        file_menu(),
    )])
    .into()
}
