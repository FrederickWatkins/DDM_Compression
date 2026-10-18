//! L0b format manipulation
use binrw::{BinRead, BinWrite};
use ndarray::Array2;

use crate::error::Result;

mod mp00;
pub use mp00::{MP00Encoder, MP00Packet};

mod mq00;

pub use mq00::{MQ00Encoder, MQ00Packet};

mod mt00;

pub use mt00::MT00Packet;

/// An encoder capable of turning `DDMFrame`s into a packets
pub trait DDMEncoder {
    /// Type of packet encoder outputs
    type Packet;

    /// Encode `DDMFrame` into packet
    fn encode(&mut self, frame: DDMFrame) -> Result<Self::Packet>;
}

/// A decoder capable of turning a bitstream into a `DDMFrame`
pub trait DDMDecoder {
    /// Type of packet decoder decodes
    type Packet;

    /// Decode `DDMFrame` from packet
    fn decode(&mut self, packet: Self::Packet) -> Result<DDMFrame>;
}

/// Metadata for a single DDM frame
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DDMFrameMetadata {
    channel: u8,
    status_flag: u8,
    gps_seconds: u32,
    sample_clocks: i32,
}

/// A single frame of a DDM, with coherent enclosed data
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DDMFrame {
    metadata: DDMFrameMetadata,
    data: Array2<u16>,
}

/// A generic L0b packet of any kind
#[derive(BinRead, BinWrite, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[br(big, magic = 0xCA75_0200u32)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum L0bPacket {
    /// [`MP00Packet`]
    #[br(magic(b"MP00"))]
    MP00(MP00Packet),
    /// [`MQ00Packet`]
    #[br(magic(b"MQ00"))]
    MQ00(MQ00Packet),
    /// [`MT00Packet`]
    #[br(magic(b"MQ00"))]
    MT00(MT00Packet),
}
