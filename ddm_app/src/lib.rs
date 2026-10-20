//! DDM app functions

use std::{
    collections::HashSet, ffi::OsString, fmt::Display, fs, ops::RangeInclusive, path::PathBuf,
    str::FromStr,
};

#[cfg(feature = "clap")]
use clap::{Args, ValueEnum};
use ddm_core::{error::Error, l0b::L0bPacket};
use humansize::{DECIMAL, format_size};
use itertools::process_results;
use ranges::{Domain, Ranges};

pub mod encoding;
pub mod file;
use file::load;

use crate::{encoding::decode, file::save};

/// Format to use for L0b packets containing frame data
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "clap", derive(ValueEnum))]
pub enum L0bFormat {
    /// MP00 format (see [`ddm_core::l0b::MP00Packet`])
    MP00,
    /// MQ00 format (see [`ddm_core::l0b::MQ00Packet`])
    MQ00,
}

impl Display for L0bFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MP00 => write!(f, "MP00"),
            Self::MQ00 => write!(f, "MQ00"),
        }
    }
}

/// Config for L0b format
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum L0bFormatConfig {
    /// MP00 format (see [`ddm_core::l0b::MP00EncoderConfig`])
    MP00 {
        /// Pixel quantisation
        quantisation: u8,
    },
    /// MQ00 format (see [`ddm_core::l0b::MQ00EncoderConfig`])
    MQ00 {
        /// Wavelet coefficient quantisation
        quantisation: u8,
        /// Whether to apply dc adjustment
        dc_adjust: bool,
        /// Number of wavelet transform stages to apply
        wavelet_stages: u8,
        /// Length of a group of pictures (1 more than number of P frames)
        gop_length: u8,
    },
}

/// Parse comma-separated values and inclusive ranges, e.g. `1,2-4,5`.
///
/// # Errors
/// Errors if `input` doesn't contain a valid list and range formatted string
pub fn parse_ranges<T>(input: &str) -> Result<Ranges<T>, String>
where
    T: Domain + FromStr + Copy,
    <T as FromStr>::Err: Display,
{
    if input.trim().is_empty() {
        return Ok(Ranges::full());
    }

    let mut ranges = Ranges::new();

    for part in input.split(',') {
        let part = part.trim();

        if part.is_empty() {
            return Err("empty entry in range list".to_owned());
        }

        if let Some((start, end)) = part.split_once('-') {
            let start = start.trim();
            let end = end.trim();

            if start.is_empty() || end.is_empty() {
                return Err(format!("invalid range `{part}`; expected START-END"));
            }

            let start = start
                .parse::<T>()
                .map_err(|e| format!("invalid range start in `{part}`: {e}"))?;

            let end = end
                .parse::<T>()
                .map_err(|e| format!("invalid range end in `{part}`: {e}"))?;

            if start > end {
                return Err(format!("range start exceeds end in `{part}`"));
            }

            ranges.insert(start..=end);
        } else {
            let value = part
                .parse::<T>()
                .map_err(|e| format!("invalid value `{part}`: {e}"))?;

            ranges.insert(value..=value);
        }
    }

    Ok(ranges)
}

/// Filters for DDM frames
#[cfg_attr(feature = "clap", derive(Debug, Args))]
#[cfg_attr(feature = "clap", command(next_help_heading = "DDM filters"))]
pub struct DDMFilters {
    /// Channel filter
    #[cfg_attr(
        feature = "clap", 
        arg(long, short, default_value = "", hide_default_value = true, value_parser = parse_ranges::<u8>))
    ]
    pub channel: Ranges<u8>,
    /// Timestamp filter
    #[cfg_attr(
        feature = "clap",
        arg(long("seconds"), short('s'), default_value = "", hide_default_value = true, value_parser = parse_ranges::<u32>))
    ]
    pub gps_seconds: Ranges<u32>,
}

/// Info about L0b file
pub struct L0bFileInfo {
    num_packets: usize,
    num_frames: usize,
    formats: HashSet<L0bFormat>,
    channels: HashSet<u8>,
    gps_seconds: (u32, u32),
    file_name: OsString,
    file_metadata: fs::Metadata,
}

impl L0bFileInfo {
    /// Number of packets
    pub fn num_packets(&self) -> usize {
        self.num_packets
    }

    /// Number of frames (packets containing DDM data)
    pub fn num_frames(&self) -> usize {
        self.num_frames
    }

    /// Data carrying formats used in file
    pub fn formats(&self) -> &HashSet<L0bFormat> {
        &self.formats
    }

    /// Channels present in file
    pub fn channels(&self) -> &HashSet<u8> {
        &self.channels
    }

    /// Range of timestamps found in file
    pub fn gps_seconds(&self) -> RangeInclusive<u32> {
        self.gps_seconds.0..=self.gps_seconds.1
    }

    /// General file metadata
    pub fn file_metadata(&self) -> &fs::Metadata {
        &self.file_metadata
    }
}

impl Display for L0bFileInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use comfy_table::Table;

        let mut table = Table::new();
        table.set_header(vec!["Property", "Value"]);

        let mut formats: Vec<_> = self.formats.iter().map(ToString::to_string).collect();
        formats.sort_unstable();

        let mut channels: Vec<_> = self.channels.iter().copied().collect();
        channels.sort_unstable();

        table.add_row(vec![
            "File name".to_owned(),
            format!("{}", self.file_name.display()),
        ]);
        table.add_row(vec![
            "File size".to_owned(),
            format_size(self.file_metadata.len(), DECIMAL),
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

/// Get info about L0b file
///
/// # Errors
/// Errors if the file isn't able to be loaded, or if it contains invalid data
#[tracing::instrument(skip_all)]
pub fn inspect<const WIDTH: usize, const HEIGHT: usize>(
    input: PathBuf,
) -> Result<L0bFileInfo, Error> {
    let mut num_packets = 0;
    let mut num_frames = 0;
    let mut formats = HashSet::new();
    let mut channels = HashSet::new();
    let mut gps_seconds = (u32::MAX, 0);
    let file_metadata = fs::metadata(&input)?;
    let file_name = input.file_name().unwrap_or(input.as_os_str()).into();
    let packets = load::<L0bPacket<WIDTH, HEIGHT>>(input)?;
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
        let frames = decode::<WIDTH, HEIGHT, _>(packets);
        for frame in frames {
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

/// Convert frames that meet filters and write in selected format
///
/// # Errors
/// Errors if the file isn't able to be loaded, or if it contains invalid data
#[tracing::instrument(skip_all)]
pub fn convert<const WIDTH: usize, const HEIGHT: usize>(
    input: PathBuf,
    output: PathBuf,
    filters: &DDMFilters,
    format: L0bFormatConfig,
) -> Result<(), Error> {
    let packets = load::<L0bPacket<WIDTH, HEIGHT>>(input)?;

    process_results(packets, |packets| {
        let frames = decode::<WIDTH, HEIGHT, _>(packets);

        process_results(frames, |frames| {
            let frames = frames.filter(|frame| {
                filters.channel.contains(&frame.metadata().channel())
                    && filters
                        .gps_seconds
                        .contains(&frame.metadata().gps_seconds())
            });

            let packets = encoding::encode(frames, format)?;

            process_results(packets, |packets| save(output, packets))?
        })?
    })??;

    Ok(())
}
