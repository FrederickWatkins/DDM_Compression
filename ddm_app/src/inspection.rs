//! File metadata inspection.
use std::{
    collections::HashSet, ffi::OsString, fmt::Display, fs, ops::RangeInclusive, path::PathBuf,
};

use ddm_core::{error::Error, l0b::L0bPacket};
use human_format::Formatter;
use itertools::process_results;

use crate::{L0bFormat, encoding::decode, file::load};

/// Summary of an L0b file.
pub struct L0bFileInfo {
    pub(crate) num_packets: usize,
    pub(crate) num_frames: usize,
    pub(crate) formats: HashSet<L0bFormat>,
    pub(crate) channels: HashSet<u8>,
    pub(crate) gps_seconds: (u32, u32),
    pub(crate) file_name: OsString,
    pub(crate) file_metadata: fs::Metadata,
}

impl L0bFileInfo {
    /// Number of packets.
    pub fn num_packets(&self) -> usize {
        self.num_packets
    }
    /// Number of frames carrying DDM data.
    pub fn num_frames(&self) -> usize {
        self.num_frames
    }
    /// Data-carrying formats used in the file.
    pub fn formats(&self) -> &HashSet<L0bFormat> {
        &self.formats
    }
    /// Channels present in the file.
    pub fn channels(&self) -> &HashSet<u8> {
        &self.channels
    }
    /// Inclusive timestamp range.
    pub fn gps_seconds(&self) -> RangeInclusive<u32> {
        self.gps_seconds.0..=self.gps_seconds.1
    }
    /// General file metadata.
    pub fn file_metadata(&self) -> &fs::Metadata {
        &self.file_metadata
    }
}

impl Display for L0bFileInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use comfy_table::Table;
        let mut bytes = Formatter::new();
        bytes.with_decimals(2).with_units("bytes");
        let mut table = Table::new();
        table.set_header(vec!["Property", "Value"]);
        let mut formats: Vec<_> = self.formats.iter().map(ToString::to_string).collect();
        formats.sort_unstable();
        let mut channels: Vec<_> = self.channels.iter().copied().collect();
        channels.sort_unstable();
        table.add_row(vec![
            "File name".to_owned(),
            self.file_name.to_string_lossy().into_owned(),
        ]);
        table.add_row(vec![
            "File size".to_owned(),
            #[expect(
                clippy::cast_precision_loss,
                reason = "File size precision loss is fine"
            )]
            bytes.format(self.file_metadata.len() as f64),
        ]);
        table.add_row(vec!["Packets".to_owned(), self.num_packets.to_string()]);
        table.add_row(vec!["DDM frames".to_owned(), self.num_frames.to_string()]);
        table.add_row(vec![
            "Formats".to_owned(),
            if formats.is_empty() {
                "None".to_owned()
            } else {
                formats.join(", ")
            },
        ]);
        table.add_row(vec![
            "Channels".to_owned(),
            if channels.is_empty() {
                "None".to_owned()
            } else {
                channels
                    .iter()
                    .map(u8::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            },
        ]);
        table.add_row(vec![
            "GPS seconds (inclusive)".to_owned(),
            if self.num_frames == 0 {
                "None".to_owned()
            } else {
                format!("{}-{}", self.gps_seconds.0, self.gps_seconds.1)
            },
        ]);
        write!(f, "{table}")
    }
}

/// Get metadata about an L0b file.
///
/// # Errors
/// Returns an error if the file cannot be loaded or contains invalid data.
#[tracing::instrument(skip_all)]
pub fn inspect<const WIDTH: usize, const HEIGHT: usize>(
    input: PathBuf,
    progress: &mut impl prodash::NestedProgress,
) -> Result<L0bFileInfo, Error> {
    let mut num_packets = 0;
    let mut num_frames = 0;
    let mut formats = HashSet::new();
    let mut channels = HashSet::new();
    let mut gps_seconds = (u32::MAX, 0);
    let file_metadata = fs::metadata(&input)?;
    let file_name = input.file_name().unwrap_or(input.as_os_str()).into();
    let mut reading = progress.add_child("Read packets");
    let mut decoding = progress.add_child("Decode frames");
    let packets = load::<L0bPacket<WIDTH, HEIGHT>>(input, &mut reading)?;
    process_results(packets, |packets| {
        let packets = packets.inspect(|packet| {
            num_packets += 1;
            match packet {
                L0bPacket::MP00(..) => {
                    formats.insert(L0bFormat::MP00);
                }
                L0bPacket::MQ00(..) => {
                    formats.insert(L0bFormat::MQ00);
                }
                L0bPacket::MT00(..) => (),
            }
        });
        for frame in decode::<WIDTH, HEIGHT, _>(packets, &mut decoding) {
            num_frames += 1;
            let frame = frame?;
            channels.insert(frame.metadata().channel());
            gps_seconds.0 = gps_seconds.0.min(frame.metadata().gps_seconds());
            gps_seconds.1 = gps_seconds.1.max(frame.metadata().gps_seconds());
        }
        Ok::<_, Error>(())
    })??;
    Ok(L0bFileInfo {
        num_packets,
        num_frames,
        formats,
        channels,
        gps_seconds,
        file_name,
        file_metadata,
    })
}
