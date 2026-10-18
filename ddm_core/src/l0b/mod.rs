//! L0b format manipulation
use binrw::{BinRead, BinWrite};
use ndarray::{Array2, ArrayView2};

use crate::error::Result;

mod mp00;
pub use mp00::{MP00Decoder, MP00Encoder, MP00Header, MP00Packet};

mod mq00;
pub use mq00::{MQ00Decoder, MQ00Encoder, MQ00EncoderConfig, MQ00Header, MQ00Packet};

mod mt00;
pub use mt00::MT00Packet;

/// An encoder capable of turning [`DDMFrame`]s into a packets
pub trait DDMEncoder<const WIDTH: usize, const HEIGHT: usize> {
    /// Type of packet encoder outputs
    type Packet;

    /// Encode `DDMFrame` into packet
    ///
    /// # Errors
    /// - [`crate::error::Error::Io`] if an IO error occurs on the underlying stream during
    ///   decoding
    fn encode(&mut self, frame: &DDMFrame<WIDTH, HEIGHT>) -> Result<Self::Packet>;
}

/// A decoder capable of turning a bitstream into a [`DDMFrame`]
pub trait DDMDecoder<const WIDTH: usize, const HEIGHT: usize> {
    /// Type of packet decoder decodes
    type Packet;

    /// Decode `DDMFrame` from packet
    ///
    /// # Errors
    /// - [`crate::error::Error::Io`] if an IO error occurs on the underlying stream during
    ///   decoding
    fn decode(&mut self, packet: Self::Packet) -> Result<DDMFrame<WIDTH, HEIGHT>>;
}

/// Metadata for a single DDM frame
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
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

/// A single frame of a DDM, with coherent enclosed data
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DDMFrame<const WIDTH: usize, const HEIGHT: usize> {
    metadata: DDMFrameMetadata,
    pixel_data: Array2<u16>,
}

impl<const WIDTH: usize, const HEIGHT: usize> DDMFrame<WIDTH, HEIGHT> {
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

#[cfg(any(test, feature = "proptest"))]
impl<const WIDTH: usize, const HEIGHT: usize> proptest::arbitrary::Arbitrary
    for DDMFrame<WIDTH, HEIGHT>
{
    type Parameters = ();
    type Strategy = proptest::strategy::BoxedStrategy<Self>;

    fn arbitrary_with((): Self::Parameters) -> Self::Strategy {
        use proptest::prelude::*;

        #[expect(clippy::expect_used, reason = "Calculation guarantees same size")]
        (any::<DDMFrameMetadata>())
            .prop_flat_map(|metadata| {
                (
                    Just(metadata),
                    proptest::collection::vec(any::<u16>(), WIDTH * HEIGHT),
                )
            })
            .prop_map(|(metadata, pixels)| {
                Self::new(
                    metadata,
                    Array2::from_shape_vec((HEIGHT, WIDTH), pixels)
                        .expect("pixel count matches array dimensions"),
                )
            })
            .boxed()
    }
}

/// A generic L0b packet of any kind
#[derive(BinRead, BinWrite, Debug, Clone, PartialEq, Eq)]
#[brw(big, magic = 0xCA75_0200u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum L0bPacket<const WIDTH: usize, const HEIGHT: usize> {
    /// [`MP00Packet`]
    #[brw(magic(b"MP00"))]
    MP00(MP00Packet<WIDTH, HEIGHT>),
    /// [`MQ00Packet`]
    #[brw(magic(b"MQ00"))]
    MQ00(MQ00Packet<WIDTH, HEIGHT>),
    /// [`MT00Packet`]
    #[brw(magic(b"MT00"))]
    MT00(MT00Packet),
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use proptest::prelude::*;

    use super::*;

    proptest! {
        #[test]
        fn bytestream_round_trip(packets in proptest::collection::vec(any::<L0bPacket<20, 52>>(), 1..20)) {
            let mut buf = vec![];
            let mut cursor = Cursor::new(&mut buf);
            for packet in &packets {
                packet.write(&mut cursor)?;
            }
            let mut decoded_packets = vec![];
            let mut cursor = Cursor::new(cursor.into_inner());
            loop {
                match L0bPacket::read(&mut cursor) {
                    Ok(packet) => decoded_packets.push(packet),
                    Err(binrw::Error::Io(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                    Err(e) => return Err(e.into())
                }
            }
            prop_assert_eq!(packets, decoded_packets);
        }
    }
}
