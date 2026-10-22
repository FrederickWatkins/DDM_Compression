use std::{collections::HashSet, fmt::Display};

/// Format used for L0b packets containing frame data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
pub enum L0bFormat {
    /// MP00 format.
    MP00,
    /// MQ00 format.
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

#[derive(Debug, Clone, PartialEq, Eq)]
/// Summary of an L0b file.
pub struct L0bFileInfo {
    /// Number of L0b packets
    pub num_packets: usize,
    /// Number containing pixel data
    pub num_frames: usize,
    /// Formats present in file
    pub formats: HashSet<L0bFormat>,
    /// Channels present in file
    pub channels: HashSet<u8>,
    /// Range of timestamps
    pub gps_seconds: (u32, u32),
}

/// Configuration for L0b output encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum L0bFormatConfig {
    /// MP00 format.
    MP00 {
        /// Pixel quantisation.
        quantisation: u8,
    },
    /// MQ00 format.
    MQ00 {
        /// Wavelet coefficient quantisation.
        quantisation: u8,
        /// Whether to apply DC adjustment.
        dc_adjust: bool,
        /// Number of wavelet transform stages.
        wavelet_stages: u8,
        /// Length of a group of pictures.
        gop_length: u8,
    },
}
