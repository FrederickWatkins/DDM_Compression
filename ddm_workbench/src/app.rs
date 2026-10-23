use std::{
    rc::Rc,
    sync::{Arc, Mutex, MutexGuard},
    thread,
    time::Duration,
};

use slint::{ComponentHandle, ModelRc, SharedString, Timer, TimerMode, VecModel};
use tracing::{error, info};

use crate::{
    AppWindow, file_info, image,
    subsystem::file::{self, LoadedFile},
};

const DEFAULT_PLAYBACK_SPEED: f32 = 30.0;
const MIN_PLAYBACK_SPEED: f32 = 1.0;
const MAX_PLAYBACK_SPEED: f32 = 60.0;

#[derive(Default)]
struct AppState {
    files: Vec<LoadedFile>,
    active_file: Option<usize>,
    current_timestamp: u32,
    selected_channel: Option<u8>,
    playback_speed: f32,
    is_playing: bool,
    status_message: String,
}

impl AppState {
    fn active_file(&self) -> Option<&LoadedFile> {
        self.active_file.and_then(|index| self.files.get(index))
    }

    fn active_timestamp_range(&self) -> Option<(u32, u32)> {
        let frames = &self.active_file()?.data.frames;
        let count = frames.num_timestamps();
        (count > 0).then(|| {
            let start = frames.start_time();
            let end = start.saturating_add(count.saturating_sub(1));
            (start, end)
        })
    }
}

/// Start the workbench window and connect its callbacks to application state.
pub fn run() -> Result<(), slint::PlatformError> {
    let window = AppWindow::new()?;
    let state = Arc::new(Mutex::new(AppState {
        playback_speed: DEFAULT_PLAYBACK_SPEED,
        status_message: "Open an L0b file to begin.".to_owned(),
        ..AppState::default()
    }));
    let playback_timer = Rc::new(Timer::default());

    sync_window(&window, &lock_state(&state));
    connect_open_file(&window, &state);
    connect_file_selection(&window, &state, &playback_timer);
    connect_playback(&window, &state, &playback_timer);
    connect_timestamp(&window, &state);
    connect_channel_selection(&window, &state);

    window.run()
}

fn connect_open_file(window: &AppWindow, state: &Arc<Mutex<AppState>>) {
    let window_weak = window.as_weak();
    let state = Arc::clone(state);
    window.on_open_file_requested(move || {
        open_files(&window_weak, &state);
    });
}

fn connect_file_selection(
    window: &AppWindow,
    state: &Arc<Mutex<AppState>>,
    playback_timer: &Rc<Timer>,
) {
    let state = Arc::clone(state);
    let window_weak = window.as_weak();
    let playback_timer = Rc::clone(playback_timer);
    window.on_file_selected(move |index| {
        let Some(window) = window_weak.upgrade() else {
            return;
        };
        let mut state = lock_state(&state);
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        let Some(file) = state.files.get(index) else {
            return;
        };
        let name = file.name.clone();
        state.active_file = Some(index);
        state.current_timestamp = state.active_timestamp_range().map_or(0, |(start, _)| start);
        state.selected_channel = None;
        state.is_playing = false;
        playback_timer.stop();
        state.status_message = format!("Loaded {name}");
        sync_window(&window, &state);
    });
}

fn connect_playback(window: &AppWindow, state: &Arc<Mutex<AppState>>, playback_timer: &Rc<Timer>) {
    let state_arc = Arc::clone(state);
    let window_weak = window.as_weak();
    let toggle_timer = Rc::clone(playback_timer);
    window.on_playback_toggled(move || {
        let Some(window) = window_weak.upgrade() else {
            return;
        };
        let mut state = lock_state(&state_arc);
        let can_play = state
            .active_timestamp_range()
            .is_some_and(|(start, end)| start < end);
        state.is_playing = can_play && !state.is_playing;
        if state.is_playing {
            start_playback(
                &toggle_timer,
                Rc::downgrade(&toggle_timer),
                window.as_weak(),
                Arc::clone(&state_arc),
                state.playback_speed,
            );
        } else {
            toggle_timer.stop();
        }
        sync_playback_controls(&window, &state);
    });

    let state_arc = Arc::clone(state);
    let window_weak = window.as_weak();
    let speed_timer = Rc::clone(playback_timer);
    window.on_playback_speed_changed(move |speed| {
        let Some(window) = window_weak.upgrade() else {
            return;
        };
        let mut state = lock_state(&state_arc);
        state.playback_speed = safe_playback_speed(speed);
        if state.is_playing {
            start_playback(
                &speed_timer,
                Rc::downgrade(&speed_timer),
                window.as_weak(),
                Arc::clone(&state_arc),
                state.playback_speed,
            );
        }
        sync_playback_controls(&window, &state);
    });
}

fn connect_timestamp(window: &AppWindow, state: &Arc<Mutex<AppState>>) {
    let state = Arc::clone(state);
    let window_weak = window.as_weak();
    window.on_timestamp_changed(move |value| {
        let Some(window) = window_weak.upgrade() else {
            return;
        };
        let mut state = lock_state(&state);
        let timestamp_index = slider_index(value);
        if let Some((start, end)) = state.active_timestamp_range() {
            state.current_timestamp = start.saturating_add(timestamp_index).min(end);
        }
        sync_active_file(&window, &state);
    });
}

fn connect_channel_selection(window: &AppWindow, state: &Arc<Mutex<AppState>>) {
    let state = Arc::clone(state);
    let window_weak = window.as_weak();
    window.on_channel_changed(move |channel| {
        let Some(window) = window_weak.upgrade() else {
            return;
        };
        let mut state = lock_state(&state);
        state.selected_channel = u8::try_from(channel).ok();
        sync_active_file(&window, &state);
    });
}

fn open_files(window: &slint::Weak<AppWindow>, state: &Arc<Mutex<AppState>>) {
    let selection_state = Arc::clone(state);
    let selection_window = window.clone();
    let selection = async move {
        let Some(paths) = file::choose_l0b_files().await else {
            return;
        };
        load_files_in_background(paths, selection_window, &selection_state);
    };

    if let Err(error) = slint::spawn_local(selection) {
        error!(error = %error, "Could not start the file picker");
        report_status(
            window,
            state,
            format!("Could not start file picker: {error}"),
        );
    }
}

fn load_files_in_background(
    paths: Vec<std::path::PathBuf>,
    window: slint::Weak<AppWindow>,
    state: &Arc<Mutex<AppState>>,
) {
    let error_window = window.clone();
    let worker_state = Arc::clone(state);
    let worker = thread::Builder::new()
        .name("ddm-file-loader".to_owned())
        .spawn(move || {
            for path in paths {
                let result = file::load_l0b_file(&path);
                let window = window.clone();
                let state = Arc::clone(&worker_state);
                if let Err(error) = slint::invoke_from_event_loop(move || {
                    let Some(window) = window.upgrade() else {
                        return;
                    };
                    match result {
                        Ok(file) => {
                            let mut state = lock_state(&state);
                            let name = file.name.clone();
                            state.files.push(file);
                            state.active_file = state.files.len().checked_sub(1);
                            state.current_timestamp =
                                state.active_timestamp_range().map_or(0, |(start, _)| start);
                            state.selected_channel = None;
                            state.is_playing = false;
                            state.status_message = format!("Loaded {name}");
                            info!(file = %name, "Loaded L0b file");
                            sync_window(&window, &state);
                        }
                        Err(error) => {
                            error!(error = %error, "Failed to load L0b file");
                            let mut state = lock_state(&state);
                            state.status_message = error.to_string();
                            sync_status(&window, &state);
                        }
                    }
                }) {
                    error!(error = %error, "Could not notify the UI about a loaded file");
                }
            }
        });

    if let Err(error) = worker {
        error!(error = %error, "Could not start the file picker thread");
        report_status(
            &error_window,
            state,
            format!("Could not start file loader: {error}"),
        );
    }
}

fn report_status(window: &slint::Weak<AppWindow>, state: &Arc<Mutex<AppState>>, message: String) {
    if let Some(window) = window.upgrade() {
        let mut state = lock_state(state);
        state.status_message = message;
        sync_status(&window, &state);
    }
}

fn start_playback(
    timer: &Timer,
    timer_weak: std::rc::Weak<Timer>,
    window: slint::Weak<AppWindow>,
    state: Arc<Mutex<AppState>>,
    speed: f32,
) {
    let speed = safe_playback_speed(speed);
    let interval = Duration::from_secs_f64(1.0 / f64::from(speed));
    timer.start(TimerMode::Repeated, interval, move || {
        if !advance_playback(&window, &state)
            && let Some(timer) = timer_weak.upgrade()
        {
            timer.stop();
        }
    });
}

fn advance_playback(window: &slint::Weak<AppWindow>, state: &Arc<Mutex<AppState>>) -> bool {
    let Some(window) = window.upgrade() else {
        return false;
    };
    let mut state = lock_state(state);
    if !state.is_playing {
        return false;
    }

    let Some((_, end)) = state.active_timestamp_range() else {
        state.is_playing = false;
        sync_playback_controls(&window, &state);
        return false;
    };

    if state.current_timestamp >= end {
        state.is_playing = false;
    } else if let Some(timestamp) = state.current_timestamp.checked_add(1) {
        state.current_timestamp = timestamp;
    } else {
        state.is_playing = false;
    }
    sync_active_file(&window, &state);
    state.is_playing
}

fn sync_window(window: &AppWindow, state: &AppState) {
    let names = state
        .files
        .iter()
        .map(|file| SharedString::from(file.name.as_str()))
        .collect::<Vec<_>>();
    window.set_files(ModelRc::new(VecModel::from(names)));
    window.set_active_file_index(
        state
            .active_file
            .and_then(|index| i32::try_from(index).ok())
            .unwrap_or(-1),
    );
    sync_status(window, state);
    sync_active_file(window, state);
}

fn sync_status(window: &AppWindow, state: &AppState) {
    window.set_status_message(SharedString::from(state.status_message.as_str()));
}

fn sync_active_file(window: &AppWindow, state: &AppState) {
    let Some(file) = state.active_file() else {
        window.set_has_frames(false);
        window.set_file_name("No file loaded".into());
        window.set_packet_summary("".into());
        window.set_format_summary("".into());
        window.set_channel_summary("".into());
        window.set_timestamp_summary("".into());
        window.set_timestamp_count(1);
        window.set_timestamp_index(0);
        window.set_timestamp_label("".into());
        window.set_frame_images(image::empty_model());
        window.set_selected_channel(-1);
        sync_playback_controls(window, state);
        return;
    };

    let info = &file.data.info;
    let frames = &file.data.frames;
    let timestamp_count = frames.num_timestamps();
    let timestamp_index = state.current_timestamp.saturating_sub(frames.start_time());

    window.set_has_frames(timestamp_count > 0);
    window.set_file_name(SharedString::from(file.name.as_str()));
    window.set_packet_summary(SharedString::from(format!(
        "{} packets, {} frames",
        info.num_packets, info.num_frames
    )));
    window.set_format_summary(SharedString::from(file_info::format_formats(info)));
    window.set_channel_summary(SharedString::from(file_info::format_channels(info)));
    window.set_timestamp_summary(SharedString::from(file_info::format_gps_seconds(info)));
    window.set_timestamp_count(slint_integer(timestamp_count.max(1)));
    window.set_timestamp_index(slint_integer(timestamp_index));
    window.set_timestamp_label(SharedString::from(format!(
        "GPS second: {}",
        state.current_timestamp
    )));
    window.set_selected_channel(state.selected_channel.map_or(-1, i32::from));
    window.set_frame_images(image::model(image::images_at(
        &file.data,
        state.current_timestamp,
    )));
    sync_playback_controls(window, state);
}

fn sync_playback_controls(window: &AppWindow, state: &AppState) {
    window.set_is_playing(state.is_playing);
    window.set_playback_speed(state.playback_speed);
    window.set_playback_speed_label(SharedString::from(format!(
        "Playback speed: {:.0} FPS",
        state.playback_speed
    )));
}

fn lock_state(state: &Mutex<AppState>) -> MutexGuard<'_, AppState> {
    state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn slider_index(value: f32) -> u32 {
    if !value.is_finite() || value <= 0.0 {
        0
    } else {
        #[expect(
            clippy::cast_precision_loss,
            reason = "The conversion supplies the u32 upper bound to the f32 slider value"
        )]
        let clamped = value.round().min(u32::MAX as f32);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "The UI slider is clamped to a non-negative frame index"
        )]
        let index = clamped as u32;
        index
    }
}

fn slint_integer(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn safe_playback_speed(speed: f32) -> f32 {
    if speed.is_finite() {
        speed.clamp(MIN_PLAYBACK_SPEED, MAX_PLAYBACK_SPEED)
    } else {
        DEFAULT_PLAYBACK_SPEED
    }
}
