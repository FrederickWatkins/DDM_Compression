//! Stateful, tabbed workspaces for open data documents.

pub(super) mod ddm_analyzer;
pub(super) mod ddm_viewer;
mod heatmap;

use crate::l0b_packet::{L0bPacket, mq00::MQ00};
use ddm_analyzer::DdmAnalyzer;
use ddm_viewer::DdmViewer;
use iced::{
    Element, Fill, Task,
    widget::{Space, column, container, text},
};
use std::path::Path;

/// Messages emitted by the currently active workspace component.
#[derive(Debug, Clone, Copy)]
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
    pub(super) fn open_l0b(&mut self, path: &Path, packets: Vec<L0bPacket>) {
        let title = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("L0B data")
            .to_owned();
        tracing::debug!(tab = %title, packet_count = packets.len(), "opening L0B viewer tab");
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
            Message::DdmViewer { tab, message } => self.update_viewer(tab, message),
            Message::DdmAnalyzer { tab, message } => self.update_analyzer(tab, message),
        }
    }

    fn update_viewer(&mut self, index: usize, message: ddm_viewer::Message) -> Task<Message> {
        if let ddm_viewer::Message::DDMSelected { channel, seconds } = message {
            self.open_analyzer(index, channel, seconds);
            return Task::none();
        }

        let Some(WorkspaceTab {
            kind: WorkspaceKind::DdmViewer(viewer),
            ..
        }) = self.tabs.get_mut(index)
        else {
            return Task::none();
        };

        viewer
            .update(message)
            .map(move |message| Message::DdmViewer {
                tab: index,
                message,
            })
    }

    fn open_analyzer(&mut self, index: usize, channel: u8, seconds: usize) {
        let Some(packet) = self.viewer_packet(index, channel, seconds) else {
            tracing::warn!(
                tab = index,
                channel,
                seconds,
                "could not open analyzer for selected DDM"
            );
            return;
        };

        tracing::debug!(tab = index, channel, seconds, "opening DDM analyzer tab");
        self.tabs.push(WorkspaceTab {
            title: format!("Channel {channel} · {seconds}s"),
            kind: WorkspaceKind::DdmAnalyzer(DdmAnalyzer::new(packet)),
        });
        self.active = Some(self.tabs.len() - 1);
    }

    fn viewer_packet(&self, index: usize, channel: u8, seconds: usize) -> Option<MQ00> {
        match &self.tabs.get(index)?.kind {
            WorkspaceKind::DdmViewer(viewer) => viewer.packet(channel, seconds),
            WorkspaceKind::DdmAnalyzer(_) => None,
        }
    }

    fn update_analyzer(&mut self, index: usize, message: ddm_analyzer::Message) -> Task<Message> {
        let Some(WorkspaceTab {
            kind: WorkspaceKind::DdmAnalyzer(analyzer),
            ..
        }) = self.tabs.get_mut(index)
        else {
            return Task::none();
        };

        analyzer.update(message);
        Task::none()
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
        let Some((index, tab)) = self.active_tab() else {
            return empty_sidebar();
        };

        match &tab.kind {
            WorkspaceKind::DdmViewer(viewer) => {
                viewer.sidebar().map(move |message| Message::DdmViewer {
                    tab: index,
                    message,
                })
            }
            WorkspaceKind::DdmAnalyzer(analyzer) => {
                analyzer.sidebar().map(move |message| Message::DdmAnalyzer {
                    tab: index,
                    message,
                })
            }
        }
    }

    /// Renders the active workspace or the initial welcome view.
    pub(super) fn view(&self) -> Element<'_, Message> {
        tracing::trace!(active_tab = ?self.active, tab_count = self.tabs.len(), "rendering workspace");
        let Some((index, tab)) = self.active_tab() else {
            return welcome_view();
        };

        match &tab.kind {
            WorkspaceKind::DdmViewer(viewer) => {
                viewer.view().map(move |message| Message::DdmViewer {
                    tab: index,
                    message,
                })
            }
            WorkspaceKind::DdmAnalyzer(analyzer) => {
                analyzer.view().map(move |message| Message::DdmAnalyzer {
                    tab: index,
                    message,
                })
            }
        }
    }

    fn active_tab(&self) -> Option<(usize, &WorkspaceTab)> {
        let index = self.active?;
        Some((index, self.tabs.get(index)?))
    }

    fn select_tab(&mut self, index: usize) {
        if self.tabs.get(index).is_some() {
            tracing::debug!(tab = index, "selected workspace tab");
            self.active = Some(index);
        }
    }

    fn close_tab(&mut self, index: usize) {
        if self.tabs.get(index).is_none() {
            return;
        }

        tracing::debug!(tab = index, "closed workspace tab");
        self.tabs.remove(index);
        self.active = self.next_active_tab(index);
    }

    fn next_active_tab(&self, closed_index: usize) -> Option<usize> {
        let active = self.active?;
        let last_index = self.tabs.len().checked_sub(1)?;
        Some(if active > closed_index {
            active - 1
        } else if active >= last_index {
            last_index
        } else {
            active
        })
    }

    fn playback_tick(&mut self) {
        for tab in &mut self.tabs {
            if let WorkspaceKind::DdmViewer(viewer) = &mut tab.kind {
                viewer.tick();
            }
        }
    }
}

fn empty_sidebar() -> Element<'static, Message> {
    container(Space::new()).width(240.0).height(Fill).into()
}

fn welcome_view() -> Element<'static, Message> {
    container(
        column![
            text("DDM Workbench").size(20),
            text("Open a file to get started.").size(14),
        ]
        .spacing(12),
    )
    .width(Fill)
    .height(Fill)
    .padding(24)
    .into()
}
