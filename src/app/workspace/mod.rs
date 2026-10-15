//! Stateful, tabbed workspaces for open data documents.

pub(super) mod ddm_viewer;

use std::path::PathBuf;

use crate::l0b_packet::L0bPacket;
use ddm_viewer::DdmViewer;
use iced::{
    Element, Task,
    widget::{Space, button, column, container, row, text},
};

/// Messages emitted by the currently active workspace component.
#[derive(Debug, Clone)]
pub(super) enum Message {
    Select(usize),
    Close(usize),
    PlaybackTick,
    DdmViewer {
        tab: usize,
        message: ddm_viewer::Message,
    },
}

/// One independently open document and its workspace state.
#[derive(Debug)]
struct WorkspaceTab {
    title: String,
    kind: WorkspaceKind,
}

/// Workspace kinds live behind this enum so different document tools can
/// coexist in the same tab strip while retaining their own state.
#[derive(Debug)]
enum WorkspaceKind {
    DdmViewer(DdmViewer),
}

/// Collection of open workspaces and the selected tab.
#[derive(Debug, Default)]
pub(super) struct Workspace {
    tabs: Vec<WorkspaceTab>,
    active: Option<usize>,
}

impl Workspace {
    /// Opens a decoded L0B file in a new DDM viewer tab.
    pub(super) fn open_l0b(&mut self, path: PathBuf, packets: Vec<L0bPacket>) {
        let title = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("L0B data")
            .to_owned();
        self.tabs.push(WorkspaceTab {
            title,
            kind: WorkspaceKind::DdmViewer(DdmViewer::new(packets)),
        });
        self.active = Some(self.tabs.len() - 1);
    }

    /// Updates active-tab selection, closes a tab, or delegates to a workspace.
    pub(super) fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Select(index) => {
                if index < self.tabs.len() {
                    self.active = Some(index);
                }
                Task::none()
            }
            Message::Close(index) => {
                if index < self.tabs.len() {
                    self.tabs.remove(index);
                    self.active = match (self.active, self.tabs.len()) {
                        (_, 0) => None,
                        (Some(active), _) if active > index => Some(active - 1),
                        (Some(active), len) if active >= len => Some(len - 1),
                        (active, _) => active,
                    };
                }
                Task::none()
            }
            Message::PlaybackTick => {
                for tab in &mut self.tabs {
                    let WorkspaceKind::DdmViewer(viewer) = &mut tab.kind;
                    viewer.tick();
                }
                Task::none()
            }
            Message::DdmViewer { tab, message } => {
                let Some(WorkspaceTab {
                    kind: WorkspaceKind::DdmViewer(viewer),
                    ..
                }) = self.tabs.get_mut(tab)
                else {
                    return Task::none();
                };
                viewer
                    .update(message)
                    .map(move |message| Message::DdmViewer { tab, message })
            }
        }
    }

    /// Whether any open workspace is currently playing.
    pub(super) fn is_playing(&self) -> bool {
        self.tabs.iter().any(|tab| match &tab.kind {
            WorkspaceKind::DdmViewer(viewer) => viewer.is_playing(),
        })
    }

    /// Builds the tab strip shown above the workspace area.
    pub(super) fn tabs_view(&self) -> Element<'_, Message> {
        let tabs = self.tabs.iter().enumerate().map(|(index, tab)| {
            let title = if self.active == Some(index) {
                format!("▣ {}", tab.title)
            } else {
                tab.title.clone()
            };
            row![
                button(text(title))
                    .on_press(Message::Select(index))
                    .padding([7, 10]),
                button("×").on_press(Message::Close(index)).padding([7, 9]),
            ]
            .spacing(2)
            .into()
        });
        container(row(tabs).spacing(4))
            .width(iced::Fill)
            .padding([4, 8])
            .into()
    }

    /// Renders metadata belonging to the active workspace.
    pub(super) fn sidebar_view(&self) -> Element<'_, Message> {
        match self.active.and_then(|index| self.tabs.get(index)) {
            Some(WorkspaceTab {
                kind: WorkspaceKind::DdmViewer(viewer),
                ..
            }) => viewer.sidebar().map(|message| Message::DdmViewer {
                tab: self.active.unwrap_or_default(),
                message,
            }),
            None => container(Space::new())
                .width(240.0)
                .height(iced::Fill)
                .into(),
        }
    }

    /// Renders the active workspace or the initial welcome view.
    pub(super) fn view(&self) -> Element<'_, Message> {
        match self.active.and_then(|index| self.tabs.get(index)) {
            Some(WorkspaceTab {
                kind: WorkspaceKind::DdmViewer(viewer),
                ..
            }) => viewer.view().map(|message| Message::DdmViewer {
                tab: self.active.unwrap_or_default(),
                message,
            }),
            None => container(
                column![
                    text("DDM Workbench").size(26),
                    text(
                        "Open an L0B data file from File → Open to inspect its packets."
                    ),
                ]
                .spacing(12),
            )
            .width(iced::Fill)
            .height(iced::Fill)
            .padding(24)
            .into(),
        }
    }
}
