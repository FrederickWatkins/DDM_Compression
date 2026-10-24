use std::{collections::HashSet, fmt::Display};

use dioxus::prelude::*;

use crate::components::select::{Select, SelectGroup, SelectGroupLabel, SelectOption};

const AVAILABLE_CHANNELS: [ChannelSelection; 17] = [
    ChannelSelection::Single(0),
    ChannelSelection::Single(1),
    ChannelSelection::Single(2),
    ChannelSelection::Single(3),
    ChannelSelection::Single(4),
    ChannelSelection::Single(5),
    ChannelSelection::Single(6),
    ChannelSelection::Single(7),
    ChannelSelection::Single(8),
    ChannelSelection::Single(9),
    ChannelSelection::Single(10),
    ChannelSelection::Single(11),
    ChannelSelection::Single(12),
    ChannelSelection::Single(13),
    ChannelSelection::Single(14),
    ChannelSelection::Single(15),
    ChannelSelection::All,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChannelSelection {
    Single(u8),
    All,
}

impl Display for ChannelSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChannelSelection::Single(channel) => write!(f, "Channel {channel}"),
            ChannelSelection::All => write!(f, "All channels"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PlaybackState {
    pub current_timestamp: u32,
    pub current_channel: ChannelSelection,
    pub playback_speed: u8,
    pub playing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackConfig {
    pub start_time: u32,
    pub end_time: u32,
}

#[component]
pub fn PlaybackControls(state: Signal<PlaybackState>, config: PlaybackConfig) -> Element {
    let state = state.read();
    let available_channels = AVAILABLE_CHANNELS.iter().enumerate().map(|(i, channel)| {
        rsx! {
            SelectOption::<ChannelSelection> { index: i, value: *channel, text_value: "{channel}", "{channel}" }
        }
    });

    rsx! {
        Select::<ChannelSelection> { default_value: ChannelSelection::All, width: "16rem",
            SelectGroup {
                SelectGroupLabel { "Channels" }
                {available_channels}
            }

        }
    }
}
