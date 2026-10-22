use ddm_app::{AggregateComparisonResult, L0bFileInfo};
use human_format::Formatter;

/// Basic file metadata
pub struct FileInfoEntry {
    /// Filename
    pub name: String,
    /// Size in bytes
    pub size: u64,
    /// Encoding info
    pub info: L0bFileInfo,
}

/// Encoding error comparison
pub struct ComparisonInfoEntry {
    /// File info
    pub file: FileInfoEntry,
    /// Error info
    pub comparison: Option<AggregateComparisonResult>,
}

trait FileInfoRow {
    fn name(&self) -> &str;
    fn size(&self) -> u64;
    fn info(&self) -> &L0bFileInfo;
}

impl FileInfoRow for FileInfoEntry {
    fn name(&self) -> &str {
        &self.name
    }

    fn info(&self) -> &L0bFileInfo {
        &self.info
    }

    fn size(&self) -> u64 {
        self.size
    }
}

impl FileInfoRow for ComparisonInfoEntry {
    fn name(&self) -> &str {
        &self.file.name
    }

    fn info(&self) -> &L0bFileInfo {
        &self.file.info
    }

    fn size(&self) -> u64 {
        self.file.size
    }
}

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

fn format_size(size: u64) -> String {
    let mut formatter = Formatter::new();
    formatter
        .with_decimals(2)
        .with_separator("")
        .with_units("B");
    #[expect(clippy::cast_precision_loss, reason = "file metadata so don't care")]
    formatter.format(size as f64)
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

fn format_info_rows<R: FileInfoRow>(infos: &[R]) -> comfy_table::Table {
    let mut table = comfy_table::Table::new();

    table.set_header(
        std::iter::once("File".to_owned())
            .chain(infos.iter().map(|entry| entry.name().to_owned()))
            .collect::<Vec<_>>(),
    );

    add_property_row(
        &mut table,
        "Size",
        infos.iter().map(|entry| format_size(entry.size())),
    );

    add_property_row(
        &mut table,
        "Packets",
        infos
            .iter()
            .map(|entry| entry.info().num_packets.to_string()),
    );

    add_property_row(
        &mut table,
        "DDM frames",
        infos
            .iter()
            .map(|entry| entry.info().num_frames.to_string()),
    );

    add_property_row(
        &mut table,
        "Formats",
        infos.iter().map(|entry| format_formats(entry.info())),
    );

    add_property_row(
        &mut table,
        "Channels",
        infos.iter().map(|entry| format_channels(entry.info())),
    );

    add_property_row(
        &mut table,
        "GPS seconds (inclusive)",
        infos.iter().map(|entry| format_gps_seconds(entry.info())),
    );
    table
}

/// Format basic file info
pub fn format_file_info(infos: &[FileInfoEntry]) -> comfy_table::Table {
    format_info_rows(infos)
}

/// Format basic file info and error comparison
pub fn format_comparison_info(infos: &[ComparisonInfoEntry]) -> comfy_table::Table {
    let mut table = format_info_rows(infos);
    add_property_row(
        &mut table,
        "Average error",
        infos.iter().map(|entry| {
            if let Some(comparison) = entry.comparison {
                format!("{}", comparison.mean)
            } else {
                "-".to_owned()
            }
        }),
    );
    add_property_row(
        &mut table,
        "Worst case error",
        infos.iter().map(|entry| {
            if let Some(comparison) = entry.comparison {
                format!("{}", comparison.worst_case)
            } else {
                "-".to_owned()
            }
        }),
    );
    table
}
