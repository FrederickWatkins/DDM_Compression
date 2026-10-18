//! MP00 legacy packet type

use std::io::{Seek, Write};

use binrw::{BinRead, BinWrite};

use crate::{
    error::Result,
    l0b::{DDMDecoder, DDMEncoder, DDMFrame},
};

/// Encoder capable of converting raw DDM frame data into encoded MP00 packets
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MP00Encoder {}

impl MP00Encoder {
    /// Create a new MP00 encoder
    pub fn new() -> Self {
        todo!()
    }
}

impl DDMEncoder for MP00Encoder {
    type Packet = MP00Packet;

    fn encode(&mut self, frame: DDMFrame) -> Result<Self::Packet> {
        todo!()
    }
}

/// Encoder capable of converting encoded MP00 packets into raw DDM frame data
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MP00Decoder {}

impl MP00Decoder {
    pub fn new() -> Self {
        todo!()
    }
}

impl DDMDecoder for MP00Decoder {
    type Packet = MP00Packet;

    fn decode(&mut self, packet: Self::Packet) -> Result<DDMFrame> {
        todo!()
    }
}

/// Header for MP00 packets
#[derive(BinRead, BinWrite, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[bw(big)]
pub struct MP00Header {}

/// MP00 packet type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MP00Packet {}

impl BinRead for MP00Packet {
    type Args<'a> = ();

    fn read_options<R: std::io::prelude::Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::prelude::BinResult<Self> {
        todo!()
    }
}

impl BinWrite for MP00Packet {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::prelude::BinResult<()> {
        todo!()
    }
}
