//! Stateful, tabbed workspaces for open data documents.

pub(super) mod ddm_viewer;
mod heatmap;
pub(super) mod ddm_analyzer;

use std::path::PathBuf;

use crate::l0b_packet::L0bPacket;
use ddm_analyzer::DdmAnalyzer;
use ddm_viewer::DdmViewer;
use iced::{
    Element, Task,
    widget::{Space, column, container, text},
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
    DdmAnalyzer {
        tab: usize,
        message: ddm_analyzer::Message,
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
    DdmAnalyzer(DdmAnalyzer),
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
                self.select_tab(index);
                Task::none()
            }
            Message::Close(index) => {
                self.close_tab(index);
                Task::none()
            }
            Message::PlaybackTick => {
                self.playback_tick();
                Task::none()
            }
            Message::DdmViewer { tab, message } => {
                let message = match message {
                    ddm_viewer::Message::DDMSelected { channel, seconds } => {
                        let packet = self.tabs.get(tab).and_then(|tab| match &tab.kind {
                            WorkspaceKind::DdmViewer(viewer) => viewer.packet(channel, seconds),
                            WorkspaceKind::DdmAnalyzer(_) => None,
                        });
                        if let Some(packet) = packet {
                            self.tabs.push(WorkspaceTab {
                                title: format!("Channel {channel} · {seconds}s"),
                                kind: WorkspaceKind::DdmAnalyzer(DdmAnalyzer::new(packet)),
                            });
                            self.active = Some(self.tabs.len() - 1);
                        }
                        return Task::none();
                    }
                    message => message,
                };
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
            Message::DdmAnalyzer { tab, message } => {
                let Some(WorkspaceTab {
                    kind: WorkspaceKind::DdmAnalyzer(analyzer),
                    ..
                }) = self.tabs.get_mut(tab)
                else {
                    return Task::none();
                };
                analyzer.update(message);
                Task::none()
            }
        }
    }

    /// Whether any open workspace is currently playing.
    pub(super) fn is_playing(&self) -> bool {
        self.tabs.iter().any(|tab| match &tab.kind {
            WorkspaceKind::DdmViewer(viewer) => viewer.is_playing(),
            WorkspaceKind::DdmAnalyzer(_) => false,
        })
    }

    /// Returns the display data needed by the tab strip view.
    pub(super) fn tab_titles(&self) -> Vec<(String, bool)> {
        self.tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| (tab.title.clone(), self.active == Some(index)))
            .collect()
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
            Some(WorkspaceTab {
                kind: WorkspaceKind::DdmAnalyzer(analyzer),
                ..
            }) => analyzer.sidebar().map(|message| Message::DdmAnalyzer {
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
            Some(WorkspaceTab {
                kind: WorkspaceKind::DdmAnalyzer(analyzer),
                ..
            }) => analyzer.view().map(|message| Message::DdmAnalyzer {
                tab: self.active.unwrap_or_default(),
                message,
            }),
            None => container(
                column![
                    text("DDM Workbench").size(20),
                    text("Open a file to get started.").size(14),
                ]
                .spacing(12),
            )
            .width(iced::Fill)
            .height(iced::Fill)
            .padding(24)
            .into(),
        }
    }

    fn select_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active = Some(index);
        }
    }

    fn close_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.tabs.remove(index);
            self.active = match (self.active, self.tabs.len()) {
                (_, 0) => None,
                (Some(active), _) if active > index => Some(active - 1),
                (Some(active), len) if active >= len => Some(len - 1),
                (active, _) => active,
            };
        }
    }

    fn playback_tick(&mut self) {
        for tab in &mut self.tabs {
            if let WorkspaceKind::DdmViewer(viewer) = &mut tab.kind {
                viewer.tick();
            }
        }
    }
}
