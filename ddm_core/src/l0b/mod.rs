//! L0b DDM Formats

use std::io::{Read, Seek};

use binrw::{BinRead, BinResult, BinWrite, Endian};
use ndarray::{Array2, ArrayView2};
use snafu::{Backtrace, Snafu};

use crate::{coding::{cabac, wavelet}};

pub mod mp00;
pub use mp00::{MP00Decoder, MP00Encoder, MP00EncoderConfig, MP00Header, MP00Packet};
pub mod mt00;
pub use mt00::MT00Packet;

/// DDM encoding error types
#[derive(Debug, Snafu)]
pub enum Error {
    /// Error during wavelet coding
    #[snafu(transparent)]
    Wavelet {
        /// Error source
        source: wavelet::Error,
    },
    /// Error during cabac compression
    #[snafu(transparent)]
    Cabac {
        /// Error source
        source: cabac::Error,
    },
    /// Error from binrw
    #[snafu(transparent)]
    Binrw {
        /// Error source
        source: binrw::Error,
        /// Error backtrace
        backtrace: Backtrace,
    },
    /// Tried to decode bits after decoder returned terminate
    #[snafu(display("calculated CRC {calculated_crc} didn't match actual crc {actual_crc}"))]
    CrcMismatch {
        /// CRC calculated from data
        calculated_crc: u32,
        /// CRC read from packet
        actual_crc: u32,
        /// Error backtrace
        backtrace: Backtrace,
    },

}

/// An encoder capable of turning [`DDMFrame`]s into a packets
pub trait DDMEncoder {
    /// Type of packet encoder outputs
    type Packet;

    /// Encode `DDMFrame` into packet
    ///
    /// # Errors
    /// - [`crate::error::Error::Io`] if an IO error occurs on the underlying stream during decoding
    fn encode(&mut self, frame: DDMFrame) -> Result<Self::Packet, Error>;
}

/// A decoder capable of turning a bitstream into a [`DDMFrame`]
pub trait DDMDecoder {
    /// Type of packet decoder decodes
    type Packet;

    /// Decode `DDMFrame` from packet
    ///
    /// # Errors
    /// - [`crate::error::Error::Io`] if an IO error occurs on the underlying stream during decoding
    fn decode(&mut self, packet: Self::Packet) -> Result<DDMFrame, Error>;
}

/// Metadata for a single DDM frame
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DDMFrameMetadata {
    channel: u8,
    status_flag: u8,
    gps_seconds: u32,
    sample_clocks: i32,
}

impl DDMFrameMetadata {
    /// Create new DDM metadata
    pub fn new(channel: u8, status_flag: u8, gps_seconds: u32, sample_clocks: i32) -> Self {
        Self {
            channel,
            status_flag,
            gps_seconds,
            sample_clocks,
        }
    }

    // Getters

    /// Get the channel number
    pub fn channel(&self) -> u8 {
        self.channel
    }

    /// Get the status flag
    pub fn status_flag(&self) -> u8 {
        self.status_flag
    }

    /// Get the GPS timestamp in seconds
    pub fn gps_seconds(&self) -> u32 {
        self.gps_seconds
    }

    /// Get the sample clock count
    pub fn sample_clocks(&self) -> i32 {
        self.sample_clocks
    }

    // Setters

    /// Set the channel number
    pub fn with_channel(mut self, channel: u8) -> Self {
        self.channel = channel;
        self
    }

    /// Set the status flag
    pub fn with_status_flag(mut self, status_flag: u8) -> Self {
        self.status_flag = status_flag;
        self
    }

    /// Set the GPS timestamp in seconds
    pub fn with_gps_seconds(mut self, gps_seconds: u32) -> Self {
        self.gps_seconds = gps_seconds;
        self
    }

    /// Set the sample clock count
    pub fn with_sample_clocks(mut self, sample_clocks: i32) -> Self {
        self.sample_clocks = sample_clocks;
        self
    }
}

#[cfg(test)]
impl proptest::arbitrary::Arbitrary for DDMFrameMetadata {
    type Parameters = ();
    type Strategy = proptest::strategy::BoxedStrategy<Self>;

    fn arbitrary_with((): Self::Parameters) -> Self::Strategy {
        use proptest::prelude::*;

        (0..15u8, any::<u8>(), any::<u32>(), any::<i32>())
            .prop_map(|(channel, status_flag, gps_seconds, sample_clocks)| Self {
                channel,
                status_flag,
                gps_seconds,
                sample_clocks,
            })
            .boxed()
    }
}

/// A single frame of a DDM, with coherent enclosed data
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DDMFrame {
    metadata: DDMFrameMetadata,
    pixel_data: Array2<u16>,
}

impl DDMFrame {
    /// Create new DDM frame
    pub fn new(metadata: DDMFrameMetadata, pixel_data: Array2<u16>) -> Self {
        Self {
            metadata,
            pixel_data,
        }
    }

    /// Get DDM metadata
    pub fn metadata(&self) -> DDMFrameMetadata {
        self.metadata
    }

    /// Get DDM pixel data
    pub fn pixel_data(&self) -> ArrayView2<'_, u16> {
        self.pixel_data.view()
    }
}

#[cfg(test)]
impl proptest::arbitrary::Arbitrary
    for DDMFrame
{
    type Parameters = (usize, usize);
    type Strategy = proptest::strategy::BoxedStrategy<Self>;

    fn arbitrary_with(parameters: Self::Parameters) -> Self::Strategy {
        use proptest::prelude::*;

        (any::<DDMFrameMetadata>())
            .prop_flat_map(move |metadata| {
                (
                    Just(metadata),
                    proptest::collection::vec(any::<u16>(), parameters.1.saturating_mul(parameters.0)),
                )
            })
            .prop_filter_map(
                "frame dimensions must be representable",
                move |(metadata, pixels)| {
                    Array2::from_shape_vec(parameters, pixels)
                        .ok()
                        .map(|pixel_data| Self::new(metadata, pixel_data))
                },
            )
            .boxed()
    }
}

/// A generic L0b packet of any kind
#[derive(BinWrite, Debug, Clone, PartialEq, Eq)]
#[brw(big, magic = 0xCA75_0200u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub enum L0bPacket {
    /// [`MP00Packet`]
    #[brw(magic(b"MP00"))]
    MP00(MP00Packet),
    // /// [`MQ00Packet`]
    // #[brw(magic(b"MQ00"))]
    // MQ00(MQ00Packet),
    /// [`MT00Packet`]
    #[brw(magic(b"MT00"))]
    MT00(MT00Packet),
}

impl From<MP00Packet>
    for L0bPacket
{
    fn from(value: MP00Packet) -> Self {
        Self::MP00(value)
    }
}

impl BinRead for L0bPacket {
    type Args<'a> = (usize, usize); // Adjust this to match whatever arguments your packets require

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: Endian,
        args: Self::Args<'_>,
    ) -> BinResult<Self> {
        // 1. Read and verify the top-level enum magic (0xCA75_0200u32)
        // Since your attributes specified `big`, you can pass Endian::Big or respect the passed `endian`.
        let magic = u32::read_options(reader, endian, ())?;
        if magic != 0xCA75_0200u32 {
            return Err(binrw::Error::BadMagic {
                pos: reader.stream_position()? - 4,
                found: Box::new(magic),
            });
        }

        // 2. Read the variant magic bytes (e.g., b"MP00", b"MT00")
        let variant_magic = <[u8; 4]>::read_options(reader, endian, ())?;

        // 3. Match the variant magic and parse the inner packet, forwarding `args`
        match &variant_magic {
            b"MP00" => {
                let packet = MP00Packet::read_options(reader, endian, args)?;
                Ok(Self::MP00(packet))
            }
            b"MT00" => {
                let packet = MT00Packet::read_options(reader, endian, ())?;
                Ok(Self::MT00(packet))
            }
            other => Err(binrw::Error::BadMagic {
                pos: reader.stream_position()? - 4,
                found: Box::new(*other),
            }),
        }
    }
}

// impl From<MQ00Packet>
//     for L0bPacket
// {
//     fn from(value: MQ00Packet) -> Self {
//         Self::MQ00(value)
//     }
// }