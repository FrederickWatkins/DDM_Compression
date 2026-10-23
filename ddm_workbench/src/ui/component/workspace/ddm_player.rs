use std::{fmt::Display, sync::Arc};

use ddm_core::l0b::DDMFrameMetadata;
use iced::{
    Alignment, ContentFit, Element, Font, Length, Task,
    widget::{
        Grid, button, column, container, image::FilterMethod, pick_list, row, rule, slider, text,
    },
};
use ndarray::Array2;
use tracing::debug;

use crate::{
    DDMFrame, L0bFileData, M, S, XS, ui::widget::{ddm_to_image, image_handle, l0b_file_info},
};

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum Message {
    Tick,
    TogglePaused,
    TimeChanged(u32),
    SpeedChanged(f32),
    ChannelChanged(ChannelOption),
}

const fn channel_options() -> [ChannelOption; 17] {
    let mut options = [ChannelOption::All; 17];
    options[0] = ChannelOption::All;
    let mut i = 0;
    while i < 16 {
        options[i + 1] = ChannelOption::Single(i as u8);
        i += 1;
    }
    options
}

const CHANNEL_OPTIONS: [ChannelOption; 17] = channel_options();

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChannelOption {
    Single(u8),
    All,
}

impl Display for ChannelOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Single(channel) => write!(f, "Channel {channel}"),
            Self::All => write!(f, "All channels"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
struct PlayerControls {
    playing: bool,
    current_timestamp: u32,
    start_time: u32,
    end_time: u32,
    playback_speed: f32,
    channel: ChannelOption,
}

impl PlayerControls {
    fn view(&self) -> Element<'_, Message> {
        let button_label = if self.playing { "Pause" } else { "Play" };
        let play_button = button(text(button_label).size(M).font(Font::MONOSPACE))
            .on_press(Message::TogglePaused);
        let channel_select =
            pick_list(CHANNEL_OPTIONS, Some(self.channel), Message::ChannelChanged);
        let top_row = row![play_button, channel_select];
        let time_label = text(format!("Timestamp: {} seconds", self.current_timestamp))
            .size(M)
            .font(Font::MONOSPACE);
        let time_slider = slider(
            self.start_time..=self.end_time,
            self.current_timestamp,
            Message::TimeChanged,
        )
        .width(Length::Fill)
        .default(self.start_time);
        let time_slider = row![time_label, time_slider]
            .align_y(Alignment::Center)
            .width(Length::Fill);
        let speed_label = text(format!("Playback speed: {} FPS", self.playback_speed))
            .size(M)
            .font(Font::MONOSPACE);
        let speed_slider = slider(1.0..=60.0, self.playback_speed, Message::SpeedChanged)
            .default(30.0f32)
            .width(Length::Fill);
        let speed_slider = row![speed_label, speed_slider]
            .align_y(Alignment::Center)
            .width(Length::Fill);
        column![top_row, time_slider, speed_slider,].into()
    }

    fn tick(&mut self) {
        if self.current_timestamp < self.end_time {
            if self.playing {
                self.current_timestamp += 1;
            }
        } else {
            self.playing = false;
        }
    }

    fn toggle_playing(&mut self) {
        if self.current_timestamp < self.end_time {
            self.playing = !self.playing;
        } else {
            self.playing = false;
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DDMPlayer {
    name: String,
    frames: Arc<L0bFileData>,
    controls: PlayerControls,
}

impl DDMPlayer {
    /// Create new ddm player
    pub fn new(mut name: String, frames: Arc<L0bFileData>) -> Self {
        name.truncate(10);
        let playing = true;
        let current_timestamp = frames.info.gps_seconds.unwrap().start;
        let playback_speed = 30.0;
        let channel = ChannelOption::All;
        let start_time = frames.frames.start_time();
        let end_time = start_time + frames.frames.num_timestamps();
        let controls = PlayerControls {
            playing,
            current_timestamp,
            start_time,
            end_time,
            playback_speed,
            channel,
        };
        Self {
            name,
            frames,
            controls,
        }
    }

    /// Player name
    pub fn name(&self) -> String {
        format!("Player: {}", self.name)
    }

    /// Update workspace with message
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {
                self.controls.tick();
                Task::none()
            }
            Message::TogglePaused => {
                debug!(
                    playing = self.controls.playing,
                    timestamp = self.controls.current_timestamp,
                    "Playback toggle requested"
                );
                self.controls.toggle_playing();
                Task::none()
            }
            Message::TimeChanged(timestamp) => {
                debug!(timestamp, "Playback timestamp changed");
                self.controls.current_timestamp = timestamp;
                Task::none()
            }
            Message::SpeedChanged(playback_speed) => {
                debug!(playback_speed, "Playback speed changed");
                self.controls.playback_speed = playback_speed;
                Task::none()
            }
            Message::ChannelChanged(channel) => {
                debug!(channel = ?channel, "Playback channel changed");
                self.controls.channel = channel;
                Task::none()
            }
        }
    }

    /// View workspace
    pub fn view(&self) -> Element<'_, Message> {
        match self.controls.channel {
            ChannelOption::Single(channel) => {
                self.ddm_image(channel, self.controls.current_timestamp)
            }
            ChannelOption::All => Grid::with_children(
                (0..16).map(|channel| self.ddm_image(channel, self.controls.current_timestamp)),
            )
            .columns(4)
            .spacing(M)
            .height(Length::Fill)
            .into(),
        }
    }

    /// Sidebar content
    pub fn sidebar_content(&self) -> Element<'_, Message> {
        column![
            self.controls.view(),
            container(rule::horizontal(XS)).width(Length::Fill).padding(S),
            l0b_file_info(&self.frames.info),
        ].into()
    }

    fn ddm_image(&self, channel: u8, timestamp: u32) -> Element<'_, Message> {
        let default_frame = DDMFrame::new(
            DDMFrameMetadata::new(0, 0, 0, 0),
            Array2::from_elem([52, 20], 0),
        );
        let frame = self
            .frames
            .frames
            .get((channel, timestamp))
            .unwrap_or(&None)
            .as_ref()
            .unwrap_or(&default_frame)
            .pixel_data();
        iced::widget::image(image_handle(&ddm_to_image(&frame).view()).unwrap())
            .width(Length::Fill)
            .height(Length::Fill)
            .content_fit(ContentFit::Contain)
            .filter_method(FilterMethod::Nearest)
            .into()
    }

    /// Required runtime subscriptions
    pub fn subscription(&self) -> iced::Subscription<Message> {
        let time_delta = (1.0 / self.controls.playback_speed) * 1000.0;
        #[expect(clippy::cast_sign_loss, clippy::cast_possible_truncation, reason="Don't care about precision")]
        iced::time::every(std::time::Duration::from_millis(time_delta as u64))
            .map(|_| Message::Tick)
    }
}
