use std::{collections::HashSet, fmt::Display};

use ddm_core::l0b::{EncodingError, MP00EncoderConfig, MQ00EncoderConfig};

use crate::file::EncoderConfig;

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
    /// Inclusive range of timestamps present in the file.
    pub gps_seconds: Option<GpsSecondRange>,
}

/// Inclusive range of GPS seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GpsSecondRange {
    /// First GPS second in the range.
    pub start: u32,
    /// Last GPS second in the range.
    pub end: u32,
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

impl Display for L0bFormatConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MP00 { quantisation } => {
                write!(f, "-f mp00 ")?;
                write!(f, "--quantisation {quantisation}")?;
            }
            Self::MQ00 {
                quantisation,
                wavelet_stages,
                gop_length,
                dc_adjust,
            } => {
                write!(f, "-f mq00 ")?;
                write!(f, "--quantisation {quantisation} ")?;
                write!(f, "--wavelet-stages {wavelet_stages} ")?;
                write!(f, "--gop-length {gop_length} ")?;
                if !dc_adjust {
                    write!(f, "--no-dc-adjust")?;
                }
            }
        }
        Ok(())
    }
}

impl TryInto<EncoderConfig> for L0bFormatConfig {
    type Error = EncodingError;

    fn try_into(self) -> Result<EncoderConfig, Self::Error> {
        match self {
            Self::MP00 { quantisation } => {
                Ok(EncoderConfig::Mp00(MP00EncoderConfig::new(quantisation)))
            }
            Self::MQ00 {
                quantisation,
                dc_adjust,
                wavelet_stages,
                gop_length,
            } => Ok(EncoderConfig::Mq00(MQ00EncoderConfig::new(
                quantisation,
                dc_adjust,
                wavelet_stages,
                gop_length,
            )?)),
        }
    }
}
