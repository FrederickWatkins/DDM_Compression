use ddm_app::L0bFileInfo;

pub(crate) fn format_formats(info: &L0bFileInfo) -> String {
    let mut formats = info
        .formats
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    formats.sort_unstable();

    if formats.is_empty() {
        "Formats: None".to_owned()
    } else {
        format!("Formats: {}", formats.join(", "))
    }
}

pub(crate) fn format_channels(info: &L0bFileInfo) -> String {
    let mut channels = info.channels.iter().copied().collect::<Vec<_>>();
    channels.sort_unstable();

    if channels.is_empty() {
        "Channels: None".to_owned()
    } else if channels.len() == 16 {
        "Channels: All".to_owned()
    } else {
        let channels = channels
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        format!("Channels: {channels}")
    }
}

pub(crate) fn format_gps_seconds(info: &L0bFileInfo) -> String {
    match info.gps_seconds {
        Some(range) => format!("Timestamp range: {}-{}", range.start, range.end),
        None => "Timestamp range: None".to_owned(),
    }
}
