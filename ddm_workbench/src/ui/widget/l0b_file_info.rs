use ddm_app::L0bFileInfo;
use iced::{
    Element,
    widget::{column, text},
};

/// Render L0b file info
pub fn l0b_file_info<'a, Message: 'a>(info: &L0bFileInfo) -> Element<'a, Message> {
    column![
        text(format!(
            "{} frames of {} packets",
            info.num_frames, info.num_packets
        )),
        text(format!("Formats present: {}", format_formats(info))),
        text(format!("Channels present: {}", format_channels(info))),
        text(format!("Timestamp range: {}", format_gps_seconds(info))),
    ]
    .into()
}

fn format_formats(info: &L0bFileInfo) -> String {
    let mut formats: Vec<_> = info.formats.iter().map(ToString::to_string).collect();
    formats.sort_unstable();

    if formats.is_empty() {
        "None".to_owned()
    } else {
        formats.join(", ")
    }
}

fn format_channels(info: &L0bFileInfo) -> String {
    let mut channels: Vec<_> = info.channels.iter().copied().collect();
    channels.sort_unstable();

    if channels.is_empty() {
        "None".to_owned()
    } else if channels.len() == 16 {
        "All".to_owned()
    } else {
        channels
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn format_gps_seconds(info: &L0bFileInfo) -> String {
    match info.gps_seconds {
        Some(range) => format!("{}-{}", range.start, range.end),
        None => "None".to_owned(),
    }
}
