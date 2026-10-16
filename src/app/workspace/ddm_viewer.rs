//! Time and channel based viewer for decoded L0B DDM packets.

use crate::{
    DDM_CHANNEL_COUNT, DDM_HEIGHT, DDM_WIDTH,
    convert::into_mq00_array,
    l0b_packet::{L0bPacket, mq00::MQ00},
};
use anyhow::Result;
use iced::{
    Alignment, Element, Length, Task,
    widget::{
        Column, Container, Row, button, column, container, pick_list, scrollable, slider, text,
    },
};
use ndarray::Array2;
use std::fmt;

use super::heatmap;

const SIDEBAR_WIDTH: f32 = 240.0;
const GRID_SIDE: u8 = 4;
const PIXEL_WIDTH: f32 = 20.0;
const PIXEL_HEIGHT: f32 = 10.0;

/// Channel selection offered by the viewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum ChannelSelection {
    Channel(u8),
    All,
}

impl fmt::Display for ChannelSelection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Channel(channel) => write!(formatter, "Channel {channel}"),
            Self::All => formatter.write_str("All 16 channels"),
        }
    }
}

/// Events emitted by the DDM viewer.
#[derive(Debug, Clone, Copy)]
pub(in crate::app) enum Message {
    ChannelChanged(ChannelSelection),
    TimeChanged(f32),
    SpeedChanged(f32),
    TogglePlayback,
    DDMSelected { channel: u8, time_index: usize },
}

/// A decoded file indexed by channel and second offset from its first DDM packet.
#[derive(Debug)]
pub(super) struct DdmViewer {
    packets: Array2<Option<MQ00>>,
    current_offset: f32,
    channel: ChannelSelection,
    playback_speed: f32,
    playing: bool,
}

impl DdmViewer {
    pub(super) fn new(packets: Vec<L0bPacket>) -> Result<Self> {
        Ok(Self {
            packets: into_mq00_array(packets)?,
            current_offset: 0.0,
            channel: ChannelSelection::All,
            playback_speed: 30.0,
            playing: true,
        })
    }

    pub(super) fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ChannelChanged(channel) => self.channel = channel,
            Message::TimeChanged(time) => self.set_time(time),
            Message::SpeedChanged(speed) => self.playback_speed = speed.clamp(1.0, 30.0),
            Message::TogglePlayback => self.playing = !self.playing,
            Message::DDMSelected { .. } => {}
        }
        Task::none()
    }

    fn set_time(&mut self, time: f32) {
        self.current_offset = time.round().clamp(0.0, self.last_offset());
    }

    fn advance_playback(&mut self) {
        if !self.playing {
            return;
        }

        let end = self.last_offset();
        let next_offset = self.current_offset + self.playback_speed / 10.0;
        self.current_offset = next_offset.min(end);
        self.playing = next_offset < end;
    }

    fn duration_seconds(&self) -> usize {
        self.packets.dim().1
    }

    fn last_offset(&self) -> f32 {
        slider_seconds(self.duration_seconds().saturating_sub(1))
    }

    fn time_index(&self) -> usize {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the nonnegative playback offset is truncated to its containing GPS second"
        )]
        {
            self.current_offset as usize
        }
    }

    fn packet_at(&self, channel: u8) -> Option<&MQ00> {
        self.packets
            .get((usize::from(channel), self.time_index()))
            .and_then(Option::as_ref)
    }

    /// Returns whether this viewer needs the shared playback timer.
    pub(super) fn is_playing(&self) -> bool {
        self.playing
    }

    pub(super) fn tick(&mut self) {
        self.advance_playback();
    }

    pub(super) fn packet(&self, channel: u8, time_index: usize) -> Option<MQ00> {
        self.packets
            .get((usize::from(channel), time_index))
            .and_then(Option::as_ref)
            .cloned()
    }

    /// Packet timeline and channel controls shown in the application's sidebar.
    pub(super) fn sidebar(&self) -> Element<'_, Message> {
        let controls = column![
            text("Controls"),
            self.channel_picker(),
            self.time_control(),
            self.speed_control(),
            self.playback_button(),
        ]
        .spacing(10);

        container(scrollable(controls).height(Length::Fill))
            .width(SIDEBAR_WIDTH)
            .height(Length::Fill)
            .padding(12)
            .into()
    }

    fn channel_picker(&self) -> Element<'static, Message> {
        let channels: Vec<_> = (0..DDM_CHANNEL_COUNT)
            .map(ChannelSelection::Channel)
            .chain(std::iter::once(ChannelSelection::All))
            .collect();

        pick_list(channels, Some(self.channel), Message::ChannelChanged).into()
    }

    fn time_control(&self) -> Element<'static, Message> {
        slider(
            0.0..=self.last_offset(),
            self.current_offset,
            Message::TimeChanged,
        )
        .style(super::super::style::slider_style)
        .into()
    }

    fn speed_control(&self) -> Element<'static, Message> {
        slider(1.0..=30.0, self.playback_speed, Message::SpeedChanged)
            .style(super::super::style::slider_style)
            .into()
    }

    fn playback_button(&self) -> Element<'static, Message> {
        let label = if self.playing { "Pause" } else { "Play" };
        button(label).on_press(Message::TogglePlayback).into()
    }

    /// Displays the selected channel or a 4 × 4 channel overview.
    pub(super) fn view(&self) -> Element<'_, Message> {
        match self.channel {
            ChannelSelection::Channel(channel) => Container::new(self.heatmap(channel, 1.0))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .into(),
            ChannelSelection::All => self.heatmap_grid(),
        }
    }

    fn heatmap(&self, channel: u8, scale: f32) -> Element<'static, Message> {
        match self.packet_at(channel) {
            Some(packet) => self.packet_heatmap(channel, scale, packet),
            None => unavailable_heatmap(channel, scale),
        }
    }

    fn packet_heatmap(&self, channel: u8, scale: f32, packet: &MQ00) -> Element<'static, Message> {
        button(column![
            text!("channel {channel}\n"),
            heatmap::view(
                &packet.raw_pixel_data(),
                PIXEL_WIDTH * scale,
                PIXEL_HEIGHT * scale,
            ),
        ])
        .on_press(Message::DDMSelected {
            channel,
            time_index: self.time_index(),
        })
        .into()
    }

    fn heatmap_grid(&self) -> Element<'_, Message> {
        let rows = (0..GRID_SIDE)
            .map(|row| self.heatmap_row(row).into())
            .collect::<Vec<_>>();

        Container::new(
            scrollable(Column::with_children(rows))
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into()
    }

    fn heatmap_row(&self, row: u8) -> Row<'static, Message> {
        let cells = (0..GRID_SIDE)
            .map(|column| {
                let channel = row * GRID_SIDE + column;
                self.heatmap(channel, 0.3)
            })
            .collect::<Vec<_>>();

        Row::with_children(cells).spacing(8)
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "Iced sliders use f32 and file durations are well within its integer precision"
)]
fn slider_seconds(seconds: usize) -> f32 {
    seconds as f32
}

fn unavailable_heatmap(channel: u8, scale: f32) -> Element<'static, Message> {
    let empty_pixels = Array2::from_elem((DDM_HEIGHT, DDM_WIDTH), 0);
    column![
        text!("channel {channel}\nunavailable"),
        heatmap::view(
            &empty_pixels.view(),
            PIXEL_WIDTH * scale,
            PIXEL_HEIGHT * scale,
        ),
    ]
    .into()
}
