//! Time and channel based viewer for decoded L0B DDM packets.

use std::{collections::HashMap, fmt, ops::RangeInclusive};

use crate::{DDMPixelData, l0b_packet::L0bPacket};
use iced::{
    Alignment, Color, Element, Fill, Theme,
    widget::{
        Space, button, column, container, pick_list, responsive, row, rule, scrollable, slider,
        text,
    },
};

const SIDEBAR_WIDTH: f32 = 240.0;
const PIXEL_WIDTH: f32 = 20.0;
const PIXEL_HEIGHT: f32 = 10.0;
const PIXEL_GAP: f32 = 1.0;
const NO_GAP_PIXEL_HEIGHT: f32 = 4.0;
const CHANNEL_PANEL_GAP: f32 = 2.0;
const DYNAMIC_RANGE_MAX: u16 = 3_000;

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
}

/// A decoded file indexed by channel and GPS second.
#[derive(Debug)]
pub(super) struct DdmViewer {
    frames: HashMap<(u32, u8), FrameData>,
    channels: Vec<u8>,
    start_time: u32,
    end_time: u32,
    current_offset: f32,
    channel: ChannelSelection,
    playback_speed: f32,
    playing: bool,
}

#[derive(Debug)]
struct FrameData {
    pixels: DDMPixelData,
    metadata: FrameMetadata,
}

#[derive(Debug)]
struct FrameMetadata {
    packet_index: usize,
    packet_type: &'static str,
    status_flag: u8,
    quantisation: String,
    gps_seconds: u32,
    sample_clocks: i32,
}

impl DdmViewer {
    pub(super) fn new(packets: Vec<L0bPacket>) -> Self {
        let mut frames = HashMap::new();
        let mut channels = Vec::new();
        for (packet_index, packet) in packets.into_iter().enumerate() {
            let decoded = match packet {
                L0bPacket::MP00(packet) => Some((
                    packet.timestamp_seconds(),
                    packet.channel_num(),
                    packet.pixel_data(),
                    FrameMetadata {
                        packet_index: packet_index + 1,
                        packet_type: "MP00",
                        status_flag: packet.status_flag(),
                        quantisation: packet.pixel_quantisation().to_string(),
                        gps_seconds: packet.timestamp_seconds(),
                        sample_clocks: packet.timestamp_clocks(),
                    },
                )),
                L0bPacket::MQ00(packet) => packet.pixel_data().ok().map(|pixels| {
                    (
                        packet.timestamp_seconds(),
                        packet.channel_num(),
                        pixels,
                        FrameMetadata {
                            packet_index: packet_index + 1,
                            packet_type: "MQ00",
                            status_flag: packet.status_flag(),
                            quantisation: format!(
                                "LL {}, detail {}",
                                packet.ll_quantisation(),
                                packet.detail_quantisation()
                            ),
                            gps_seconds: packet.timestamp_seconds(),
                            sample_clocks: packet.timestamp_clocks(),
                        },
                    )
                }),
                L0bPacket::MT00(_) => None,
            };

            if let Some((gps_seconds, channel, pixels, metadata)) = decoded {
                if channel < 16 {
                    if !channels.contains(&channel) {
                        channels.push(channel);
                    }
                    frames.insert((gps_seconds, channel), FrameData { pixels, metadata });
                }
            }
        }
        channels.sort_unstable();

        let (start_time, end_time) = frames
            .keys()
            .map(|(gps_seconds, _)| *gps_seconds)
            .fold(None::<(u32, u32)>, |bounds, gps_seconds| match bounds {
                Some((start, end)) => Some((start.min(gps_seconds), end.max(gps_seconds))),
                None => Some((gps_seconds, gps_seconds)),
            })
            .unwrap_or((0, 0));

        Self {
            frames,
            channels,
            start_time,
            end_time,
            current_offset: 0.0,
            channel: ChannelSelection::All,
            playback_speed: 1.0,
            playing: false,
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
                self.playing = !self.playing && self.end_time > self.start_time;
            }
            Message::PlaybackTick => self.advance_playback(),
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

    fn duration_seconds(&self) -> u32 {
        self.end_time.saturating_sub(self.start_time)
    }

    fn gps_seconds(&self) -> u32 {
        self.start_time
            .saturating_add(self.current_offset.round() as u32)
            .min(self.end_time)
    }

    /// Returns whether this viewer needs the shared playback timer.
    pub(super) fn is_playing(&self) -> bool {
        self.playing
    }

    pub(super) fn tick(&mut self) {
        self.advance_playback();
    }

    /// Packet timeline and channel controls shown in the application's sidebar.
    pub(super) fn sidebar(&self) -> Element<'_, Message> {
        let mut options: Vec<ChannelSelection> = (0..16).map(ChannelSelection::Channel).collect();
        options.push(ChannelSelection::All);

        let slider_end = self.duration_seconds().max(1) as f32;
        let mut controls = column![
            text("Channel").size(12),
            pick_list(options, Some(self.channel), Message::ChannelChanged),
            rule::horizontal(1),
            text("GPS seconds").size(12),
            slider(
                RangeInclusive::new(0.0, slider_end),
                self.current_offset,
                Message::TimeChanged,
            )
            .step(1.0_f32),
            text(format!(
                "{:.0} s  ({}–{} s)",
                self.gps_seconds(),
                self.start_time,
                self.end_time
            ))
            .size(13),
            rule::horizontal(1),
            row![
                button(if self.playing { "Pause" } else { "Play" })
                    .on_press(Message::TogglePlayback)
                    .width(Fill),
                text(format!("{:.0} s/s", self.playback_speed)).size(13),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            slider(
                1.0_f32..=30.0_f32,
                self.playback_speed,
                Message::SpeedChanged
            )
            .step(1.0_f32),
            text("Playback speed (GPS seconds per second)").size(11),
            rule::horizontal(1),
            text(format!("{} channels with DDM data", self.channels.len())).size(13),
            text("Heatmap range: 0–5000").size(12),
        ]
        .spacing(10);

        if let ChannelSelection::Channel(channel) = self.channel {
            controls = controls.push(rule::horizontal(1));
            if let Some(frame) = self.frames.get(&(self.gps_seconds(), channel)) {
                controls = controls
                    .push(text("Packet metadata").size(16))
                    .push(metadata_row(
                        "Packet type",
                        frame.metadata.packet_type.to_owned(),
                    ))
                    .push(metadata_row(
                        "Packet index",
                        frame.metadata.packet_index.to_string(),
                    ))
                    .push(metadata_row("Channel", channel.to_string()))
                    .push(metadata_row(
                        "Status flag",
                        format!("0x{:02X}", frame.metadata.status_flag),
                    ))
                    .push(metadata_row(
                        "Quantisation",
                        frame.metadata.quantisation.clone(),
                    ))
                    .push(metadata_row(
                        "GPS seconds",
                        frame.metadata.gps_seconds.to_string(),
                    ))
                    .push(metadata_row(
                        "Sample clocks",
                        frame.metadata.sample_clocks.to_string(),
                    ))
                    .push(metadata_row("Pixel dimensions", "52 × 20".to_owned()));
            } else {
                controls = controls.push(text("Packet metadata").size(16)).push(
                    text(format!(
                        "No packet for channel {channel} at this GPS second."
                    ))
                    .size(13),
                );
            }
        }

        container(scrollable(controls).height(Fill))
            .width(SIDEBAR_WIDTH)
            .height(Fill)
            .padding(12)
            .into()
    }

    /// Displays the selected channel or a 4 × 4 channel overview.
    pub(super) fn view(&self) -> Element<'_, Message> {
        let gps_seconds = self.gps_seconds();
        match self.channel {
            ChannelSelection::Channel(channel) => {
                let frame = self.frames.get(&(gps_seconds, channel));
                container(heatmap_view(
                    frame.map(|frame| &frame.pixels),
                    channel,
                    PIXEL_WIDTH,
                    PIXEL_HEIGHT,
                    PIXEL_GAP,
                    None,
                ))
                .width(Fill)
                .height(Fill)
                .padding(16)
                .center_x(Fill)
                .center_y(Fill)
                .into()
            }
            ChannelSelection::All => container(responsive(move |size| {
                let cell_width = ((size.width - CHANNEL_PANEL_GAP * 3.0) / 4.0).max(0.0);
                let cell_height = ((size.height - CHANNEL_PANEL_GAP * 3.0) / 4.0).max(0.0);
                let pixel_height = ((cell_height - 24.0) / 52.0)
                    .min(cell_width / 40.0)
                    .floor()
                    .max(1.0);
                let pixel_width = pixel_height * 2.0;
                let pixel_gap = if pixel_height < NO_GAP_PIXEL_HEIGHT {
                    0.0
                } else {
                    PIXEL_GAP
                };

                let grid_rows: Vec<Element<'_, Message>> = (0..4)
                    .map(|grid_row| {
                        let channels: Vec<Element<'_, Message>> = (grid_row * 4..grid_row * 4 + 4)
                            .map(|channel| {
                                let frame = self.frames.get(&(gps_seconds, channel));
                                let panel = heatmap_view(
                                    frame.map(|frame| &frame.pixels),
                                    channel,
                                    pixel_width,
                                    pixel_height,
                                    pixel_gap,
                                    Some(cell_width),
                                );
                                container(panel)
                                    .width(cell_width)
                                    .height(cell_height)
                                    .center_x(cell_width)
                                    .center_y(cell_height)
                                    .into()
                            })
                            .collect();
                        row(channels).spacing(CHANNEL_PANEL_GAP).into()
                    })
                    .collect();
                column(grid_rows)
                    .spacing(CHANNEL_PANEL_GAP)
                    .width(Fill)
                    .height(Fill)
                    .into()
            }))
            .width(Fill)
            .height(Fill)
            .padding(12)
            .into(),
        }
    }
}

fn heatmap_view<'a>(
    pixel_data: Option<&'a DDMPixelData>,
    channel: u8,
    pixel_width: f32,
    pixel_height: f32,
    pixel_gap: f32,
    caption_width: Option<f32>,
) -> Element<'a, Message> {
    let pixel_rows: Vec<Element<'a, Message>> = (0..52)
        .map(|row_index| {
            let cells: Vec<Element<'a, Message>> = (0..20)
                .map(|column_index| {
                    let color = pixel_data
                        .map(|pixels| heat_color(pixels[row_index][column_index]))
                        .unwrap_or_else(|| Color::from_rgb(0.32, 0.32, 0.32));
                    container(Space::new())
                        .width(pixel_width)
                        .height(pixel_height)
                        .style(move |_theme: &Theme| container::Style {
                            background: Some(color.into()),
                            ..container::Style::default()
                        })
                        .into()
                })
                .collect();
            row(cells).spacing(pixel_gap).into()
        })
        .collect();

    let pixel_grid = column(pixel_rows).spacing(pixel_gap);
    let title = if pixel_data.is_some() {
        format!("Channel {channel}")
    } else {
        format!("Channel {channel} · unavailable")
    };
    let caption = container(text(title).size(if pixel_width > 10.0 { 16 } else { 11 }))
        .width(caption_width.map_or(iced::Length::Shrink, iced::Length::Fixed))
        .height(18.0)
        .center_x(Fill)
        .center_y(Fill);
    column![caption, pixel_grid,]
        .spacing(2.0)
        .align_x(Alignment::Center)
        .into()
}

fn metadata_row(label: &'static str, value: String) -> Element<'static, Message> {
    column![text(label).size(12), text(value).size(14)]
        .spacing(2)
        .into()
}

fn heat_color(value: u16) -> Color {
    let amount = f32::from(value.min(DYNAMIC_RANGE_MAX)) / f32::from(DYNAMIC_RANGE_MAX);

    // Dark blue → cyan → orange → yellow.
    let stops = [
        (0.0, Color::from_rgb(0.025, 0.035, 0.16)),
        (0.35, Color::from_rgb(0.0, 0.48, 0.82)),
        (0.70, Color::from_rgb(0.95, 0.23, 0.08)),
        (1.0, Color::from_rgb(1.0, 0.92, 0.22)),
    ];

    for pair in stops.windows(2) {
        let (start, first) = pair[0];
        let (end, second) = pair[1];
        if amount <= end {
            let local = ((amount - start) / (end - start)).clamp(0.0, 1.0);
            return Color::from_rgb(
                first.r + (second.r - first.r) * local,
                first.g + (second.g - first.g) * local,
                first.b + (second.b - first.b) * local,
            );
        }
    }

    Color::from_rgb(1.0, 0.92, 0.22)
}
