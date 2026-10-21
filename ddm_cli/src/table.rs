use ddm_app::L0bFileInfo;


fn add_property_row(
    table: &mut comfy_table::Table,
    property: &str,
    values: impl IntoIterator<Item = String>,
) {
    table.add_row(
        std::iter::once(property.to_owned())
            .chain(values)
            .collect::<Vec<_>>(),
    );
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
    if info.num_frames == 0 {
        "None".to_owned()
    } else {
        format!("{}-{}", info.gps_seconds.0, info.gps_seconds.1)
    }
}

pub fn format_file_info(infos: &[(String, L0bFileInfo)]) -> comfy_table::Table {
    let mut table = comfy_table::Table::new();

    table.set_header(
        std::iter::once("File".to_owned())
            .chain(infos.iter().map(|(name, _)| name.clone()))
            .collect::<Vec<_>>(),
    );

    add_property_row(
        &mut table,
        "Packets",
        infos.iter().map(|(_, info)| info.num_packets.to_string()),
    );

    add_property_row(
        &mut table,
        "DDM frames",
        infos.iter().map(|(_, info)| info.num_frames.to_string()),
    );

    add_property_row(
        &mut table,
        "Formats",
        infos.iter().map(|(_, info)| format_formats(info)),
    );

    add_property_row(
        &mut table,
        "Channels",
        infos.iter().map(|(_, info)| format_channels(info)),
    );

    add_property_row(
        &mut table,
        "GPS seconds (inclusive)",
        infos.iter().map(|(_, info)| format_gps_seconds(info)),
    );
    table
}
