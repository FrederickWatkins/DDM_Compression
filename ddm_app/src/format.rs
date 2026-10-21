//! File and packet format types.
use std::fmt::Display;

#[cfg(feature = "clap")]
use clap::ValueEnum;

/// Format used to serialize a selected DDM frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "clap", derive(ValueEnum))]
pub enum DDMFormat {
    /// Postcard representation of a [`ddm_core::l0b::DDMFrame`].
    Postcard,
}

/// Format used for L0b packets containing frame data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "clap", derive(ValueEnum))]
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
