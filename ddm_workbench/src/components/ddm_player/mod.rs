mod playback_controls;
use std::sync::Arc;

use dioxus::prelude::*;
use playback_controls::{ChannelSelection, PlaybackControls, PlaybackState};

use crate::{components::ddm_player::playback_controls::PlaybackConfig, L0bFileData};

#[component]
pub fn DDMPlayer(file: Arc<L0bFileData>) -> Element {
    let config = PlaybackConfig {
        start_time: file.frames.start_time(),
        end_time: file.frames.start_time() + file.frames.num_timestamps(),
    };
    let state = Signal::new(PlaybackState {
        current_timestamp: config.start_time,
        current_channel: ChannelSelection::All,
        playback_speed: 30,
        playing: true,
    });
    rsx! {
        
        PlaybackControls { state, config }

        "{state:?}"
    }
}
