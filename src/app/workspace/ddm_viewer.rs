//! Time and channel based viewer for decoded L0B DDM packets.

use std::fmt;

use super::heatmap;
use crate::{
    convert::into_mq00_array,
    l0b_packet::{L0bPacket, mq00::MQ00},
};
use iced::{
    Alignment, Element, Length,
    widget::{
        Column, Container, Row, button, column, container, pick_list, responsive, rule, scrollable,
        slider, text,
    },
};
use ndarray::Array2;

const SIDEBAR_WIDTH: f32 = 240.0;
const PIXEL_GAP: f32 = 1.0;
const NO_GAP_PIXEL_HEIGHT: f32 = 4.0;
const CHANNEL_PANEL_GAP: f32 = 2.0;

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
#[derive(Debug, Clone)]
pub(in crate::app) enum Message {
    ChannelChanged(ChannelSelection),
    TimeChanged(f32),
    SpeedChanged(f32),
    TogglePlayback,
    PlaybackTick,
    DDMSelected { channel: u8, seconds: usize },
}

/// A decoded file indexed by channel and GPS second.
#[derive(Debug)]
pub(super) struct DdmViewer {
    packets: Array2<Option<MQ00>>,
    current_offset: f32,
    channel: ChannelSelection,
    playback_speed: f32,
    playing: bool,
}

impl DdmViewer {
    pub(super) fn new(packets: Vec<L0bPacket>) -> Self {
        let packets = into_mq00_array(packets);
        let current_offset = 0.0;
        let channel = ChannelSelection::All;
        let playback_speed = 30.0;
        let playing = true;
        Self {
            packets,
            current_offset,
            channel,
            playback_speed,
            playing,
        }
    }

    pub(super) fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::ChannelChanged(channel) => self.channel = channel,
            Message::TimeChanged(time) => {
                self.current_offset = time.round().clamp(0.0, self.duration_seconds() as f32)
            }
            Message::SpeedChanged(speed) => self.playback_speed = speed.clamp(1.0, 30.0),
            Message::TogglePlayback => {
                self.playing = !self.playing;
            }
            Message::PlaybackTick => self.advance_playback(),
            Message::DDMSelected { .. } => {}
        }
        iced::Task::none()
    }

    fn advance_playback(&mut self) {
        if !self.playing {
            return;
        }

        let next_offset = self.current_offset + self.playback_speed / 10.0;
        if next_offset >= self.duration_seconds() as f32 {
            self.current_offset = self.duration_seconds() as f32;
            self.playing = false;
        } else {
            self.current_offset = next_offset;
        }
    }

    fn duration_seconds(&self) -> usize {
        self.packets.dim().1
    }

    fn gps_seconds(&self) -> usize {
        self.current_offset as usize
    }

    /// Returns whether this viewer needs the shared playback timer.
    pub(super) fn is_playing(&self) -> bool {
        self.playing
    }

    pub(super) fn tick(&mut self) {
        self.advance_playback();
    }

    pub(super) fn packet(&self, channel: u8, seconds: usize) -> Option<MQ00> {
        self.packets
            .get((channel as usize, seconds))
            .and_then(Option::as_ref)
            .cloned()
    }

    /// Packet timeline and channel controls shown in the application's sidebar.
    pub(super) fn sidebar(&self) -> Element<'_, Message> {
        let mut options: Vec<ChannelSelection> = (0..16).map(ChannelSelection::Channel).collect();
        options.push(ChannelSelection::All);

        let mut channels = vec![];

        for channel in 0..16 {
            channels.push(ChannelSelection::Channel(channel));
        }
        channels.push(ChannelSelection::All);

        let channel_dropdown = pick_list(channels, Some(self.channel), |channel| {
            Message::ChannelChanged(channel)
        });
        let button_message = match self.playing {
            true => "Pause",
            false => "Play",
        };
        let play_button = button(button_message).on_press(Message::TogglePlayback);
        let time_slider = slider(
            0.0..=self.duration_seconds() as f32,
            self.current_offset,
            |time| Message::TimeChanged(time),
        );
        let speed_slider = slider(0.0..=30.0, self.playback_speed, |speed| {
            Message::SpeedChanged(speed)
        });

        let controls = column![
            "Controls",
            channel_dropdown,
            time_slider,
            speed_slider,
            play_button
        ]
        .spacing(10);

        container(scrollable(controls).height(Length::Fill))
            .width(SIDEBAR_WIDTH)
            .height(Length::Fill)
            .padding(12)
            .into()
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

    fn heatmap(&self, channel: u8, scale: f32) -> Element<'_, Message> {
        if let Some(ref packet) = self.packets[(channel as usize, self.gps_seconds())] {
            let message = text!("channel {channel}\n");
            let heatmap = heatmap::view(packet.raw_pixel_data(), 20.0 * scale, 10.0 * scale);
            button(column![message, heatmap])
                .on_press(Message::DDMSelected {
                    channel,
                    seconds: self.gps_seconds(),
                })
                .into()
        } else {
            let message = text!("channel {channel}\nunavailable");
            let heatmap = heatmap::view(
                Array2::from_elem((52, 20), 0).view(),
                20.0 * scale,
                10.0 * scale,
            );
            column![message, heatmap].into()
        }
    }

    fn heatmap_grid(&self) -> Element<'_, Message> {
        let mut rows = vec![];
        for row in 0..4 {
            let mut cells = vec![];
            for column in 0..4 {
                let channel = row * 4 + column;
                cells.push(self.heatmap(channel, 0.3).into());
            }
            rows.push(Row::with_children(cells).spacing(8).into())
        }
        Container::new(
            scrollable(Column::with_children(rows))
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into()
    }
}

fn metadata_row(label: &'static str, value: String) -> Element<'static, Message> {
    column![text(label).size(12), text(value).size(14)]
        .spacing(2)
        .into()
}
