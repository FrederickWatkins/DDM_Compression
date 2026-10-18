//! Expertimental MQ00 packet type

use std::io::{Read, Seek, Write};

use binrw::{BinRead, BinWrite};

use crate::{
    error::{ErrorKind, Result},
    l0b::{DDMEncoder, DDMFrame},
};

pub struct MQ00EncoderConfig {
    quantisation: u8,
    dc_adjust: bool,
    wavelet_stages: u8,
    gop_length: u8,
}

impl MQ00EncoderConfig {
    pub fn new(
        quantisation: u8,
        dc_adjust: bool,
        wavelet_stages: u8,
        gop_length: u8,
    ) -> Result<Self> {
        Ok(Self::default()
            .with_quantisation(quantisation)?
            .with_dc_adjust(dc_adjust)
            .with_wavelet_stages(wavelet_stages)?
            .with_gop_length(gop_length))
    }

    pub fn quantisation(&self) -> u8 {
        self.quantisation
    }

    pub fn dc_adjust(&self) -> bool {
        self.dc_adjust
    }

    pub fn wavelet_stages(&self) -> u8 {
        self.wavelet_stages
    }

    pub fn gop_length(&self) -> u8 {
        self.gop_length
    }

    pub fn with_quantisation(mut self, bits: u8) -> Result<Self> {
        if bits & 0xF != 0 {
            return Err(ErrorKind::ValueOutOfRange {
                name: "bits",
                min: 0,
                max: 15,
                actual: u64::from(bits),
            });
        }
        self.quantisation = bits;
        Ok(self)
    }

    pub fn with_dc_adjust(mut self, enabled: bool) -> Self {
        self.dc_adjust = enabled;
        self
    }

    pub fn with_wavelet_stages(mut self, stages: u8) -> Result<Self> {
        if stages & 0xF != 0 {
            return Err(ErrorKind::ValueOutOfRange {
                name: "stages",
                min: 0,
                max: 15,
                actual: u64::from(stages),
            });
        }
        self.wavelet_stages = stages;
        Ok(self)
    }

    pub fn with_gop_length(mut self, frames: u8) -> Self {
        self.gop_length = frames;
        self
    }
}

impl Default for MQ00EncoderConfig {
    fn default() -> Self {
        Self {
            quantisation: 0,
            dc_adjust: false,
            wavelet_stages: 0,
            gop_length: 1,
        }
    }
}

/// Encoder capable of converting raw DDM frame data into encoded MQ00 packets
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MQ00Encoder {}

impl MQ00Encoder {
    /// Create a new MQ00 encoder
    pub fn new(config: &MQ00EncoderConfig) -> Self {
        todo!()
    }
}

impl DDMEncoder for MQ00Encoder {
    type Packet = MQ00Packet;

    fn encode(&mut self, frame: DDMFrame) -> Result<Self::Packet> {
        todo!()
    }
}

/// Decoder capable of converting encoded MQ00 packets into raw DDM frame data
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MQ00Decoder {}

impl MQ00Decoder {
    pub fn new() -> Self {
        todo!()
    }
}

/// Header for MQ00 packets
#[derive(BinRead, BinWrite, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[bw(big)]
pub struct MQ00Header {}

/// MQ00 packet type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MQ00Packet {
    header: MQ00Header,
}

impl BinRead for MQ00Packet {
    type Args<'a> = ();

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::prelude::BinResult<Self> {
        todo!()
    }
}

impl BinWrite for MQ00Packet {
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
