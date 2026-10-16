//! Stateful, tabbed workspaces for open data documents.

pub(super) mod ddm_analyzer;
pub(super) mod ddm_viewer;
mod heatmap;

use crate::{
    DDM_CHANNEL_COUNT, DDM_HEIGHT, DDM_WIDTH, DDMPixelData,
    l0b_packet::{L0bPacket, mq00::MQ00},
};
use anyhow::Result;
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

pub(super) type ExportFrame = (usize, u8, MQ00);

pub(super) struct ExportFrames {
    pub viewer: bool,
    pub frames: Vec<ExportFrame>,
}

impl Workspace {
    /// Returns tab names and whether each tab is a DDM viewer.
    pub(super) fn export_tabs(&self) -> Vec<(usize, String, bool)> {
        self.tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| {
                (
                    index,
                    tab.title.clone(),
                    matches!(&tab.kind, WorkspaceKind::DdmViewer(_)),
                )
            })
            .collect()
    }

    pub(super) fn active_index(&self) -> Option<usize> {
        self.active
    }

    pub(super) fn export_frames(&self, index: usize) -> Option<ExportFrames> {
        let tab = self.tabs.get(index)?;
        Some(match &tab.kind {
            WorkspaceKind::DdmViewer(viewer) => ExportFrames {
                viewer: true,
                frames: viewer.export_frames(),
            },
            WorkspaceKind::DdmAnalyzer(analyzer) => {
                let packet = analyzer.packet();
                ExportFrames {
                    viewer: false,
                    frames: vec![(0, packet.channel_num(), packet)],
                }
            }
        })
    }

    /// Opens a decoded L0B file in a new DDM viewer tab.
    pub(super) fn open_l0b(&mut self, path: &Path, packets: Vec<L0bPacket>) -> Result<()> {
        let title = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("L0B data")
            .to_owned();
        tracing::debug!(tab = %title, packet_count = packets.len(), "opening L0B viewer tab");
        let viewer = DdmViewer::new(packets)?;
        self.tabs.push(WorkspaceTab {
            title,
            kind: WorkspaceKind::DdmViewer(viewer),
        });
        self.active = Some(self.tabs.len() - 1);
        Ok(())
    }

    /// Opens a CBOR DDM array: `(height, width)`, `(time, height, width)`, or
    /// `(channel, time, height, width)`.
    pub(super) fn open_archive(&mut self, path: &Path, archive: &ciborium::Value) -> Result<()> {
        let title = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("DDM archive")
            .to_owned();
        let rank = array_rank(archive)
            .ok_or_else(|| anyhow::anyhow!("archive root must be a non-empty nested array"))?;
        match rank {
            2 => {
                let packet = packet_from_pixels(archive, 0, 0)?;
                self.tabs.push(WorkspaceTab {
                    title,
                    kind: WorkspaceKind::DdmAnalyzer(DdmAnalyzer::new(packet)),
                });
            }
            3 | 4 => {
                let packets = archive_packets(archive, rank)?;
                let viewer = DdmViewer::new(packets)?;
                self.tabs.push(WorkspaceTab {
                    title,
                    kind: WorkspaceKind::DdmViewer(viewer),
                });
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "archive array must have 2, 3, or 4 dimensions; found {rank}"
                ));
            }
        }
        self.active = Some(self.tabs.len() - 1);
        Ok(())
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
        if let ddm_viewer::Message::DDMSelected {
            channel,
            time_index,
        } = message
        {
            self.open_analyzer(index, channel, time_index);
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

    fn open_analyzer(&mut self, index: usize, channel: u8, time_index: usize) {
        let Some(packet) = self.viewer_packet(index, channel, time_index) else {
            tracing::warn!(
                tab = index,
                channel,
                time_index,
                "could not open analyzer for selected DDM"
            );
            return;
        };

        let timestamp = packet.timestamp_seconds();
        tracing::debug!(tab = index, channel, timestamp, "opening DDM analyzer tab");
        self.tabs.push(WorkspaceTab {
            title: format!("Channel {channel} · {timestamp}s"),
            kind: WorkspaceKind::DdmAnalyzer(DdmAnalyzer::new(packet)),
        });
        self.active = Some(self.tabs.len() - 1);
    }

    fn viewer_packet(&self, index: usize, channel: u8, time_index: usize) -> Option<MQ00> {
        match &self.tabs.get(index)?.kind {
            WorkspaceKind::DdmViewer(viewer) => viewer.packet(channel, time_index),
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

fn array_rank(value: &ciborium::Value) -> Option<usize> {
    let values = value.as_array()?;
    let nested_rank = values.iter().filter_map(array_rank).max().unwrap_or(0);
    Some(1 + nested_rank)
}

fn archive_packets(value: &ciborium::Value, rank: usize) -> Result<Vec<L0bPacket>> {
    let outer = value
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("archive data must be an array"))?;
    let mut packets = Vec::new();
    match rank {
        3 => {
            for (time, pixels) in outer.iter().enumerate() {
                if matches!(pixels, ciborium::Value::Null) {
                    continue;
                }
                packets.push(L0bPacket::MQ00(Box::new(packet_from_pixels(
                    pixels,
                    0,
                    u32::try_from(time)?,
                )?)));
            }
        }
        4 => {
            if outer.len() > usize::from(DDM_CHANNEL_COUNT) {
                return Err(anyhow::anyhow!(
                    "archive contains more than {DDM_CHANNEL_COUNT} channels"
                ));
            }
            for (channel, times) in outer.iter().enumerate() {
                let Some(times) = times.as_array() else {
                    if matches!(times, ciborium::Value::Null) {
                        continue;
                    }
                    return Err(anyhow::anyhow!(
                        "each archive channel must contain a time array"
                    ));
                };
                for (time, pixels) in times.iter().enumerate() {
                    if matches!(pixels, ciborium::Value::Null) {
                        continue;
                    }
                    packets.push(L0bPacket::MQ00(Box::new(packet_from_pixels(
                        pixels,
                        u8::try_from(channel)?,
                        u32::try_from(time)?,
                    )?)));
                }
            }
        }
        _ => return Err(anyhow::anyhow!("unsupported archive array rank {rank}")),
    }
    Ok(packets)
}

fn packet_from_pixels(value: &ciborium::Value, channel: u8, timestamp: u32) -> Result<MQ00> {
    let rows = value
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("DDM frame must be a two-dimensional array"))?;
    if rows.len() != DDM_HEIGHT {
        return Err(anyhow::anyhow!(
            "DDM frame must have {DDM_HEIGHT} rows, found {}",
            rows.len()
        ));
    }
    let mut pixels: DDMPixelData = [[0; DDM_WIDTH]; DDM_HEIGHT];
    for (row_index, row) in rows.iter().enumerate() {
        let row = row
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("DDM row {row_index} must be an array"))?;
        if row.len() != DDM_WIDTH {
            return Err(anyhow::anyhow!(
                "DDM row {row_index} must have {DDM_WIDTH} pixels, found {}",
                row.len()
            ));
        }
        for (column, value) in row.iter().enumerate() {
            let integer = value.as_integer().ok_or_else(|| {
                anyhow::anyhow!("DDM pixel ({row_index}, {column}) must be an integer")
            })?;
            pixels[row_index][column] = integer
                .try_into()
                .map_err(|_| anyhow::anyhow!("DDM pixel ({row_index}, {column}) is outside u16"))?;
        }
    }
    MQ00::new(channel, 0, 0, 0, timestamp, 0, pixels)
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
