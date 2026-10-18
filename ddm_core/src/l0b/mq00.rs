//! Expertimental MQ00 packet type

use std::io::{Read, Seek, Write};

use binrw::{
    BinRead, BinWrite,
    Endian::Big,
    meta::{EndianKind, ReadEndian, WriteEndian},
};
use ndarray::Array2;

use crate::{
    error::{Error, Result},
    l0b::{DDMDecoder, DDMEncoder, DDMFrame},
};

#[derive(BinRead, BinWrite, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(any(test, feature = "serde"), derive(proptest_derive::Arbitrary))]
#[repr(u8)]
#[brw(repr = u8)]
pub enum MQ00PacketKind {
    Intra,
    Predictive,
}

/// Config for [`MQ00Encoder`]
pub struct MQ00EncoderConfig {
    quantisation: u8,
    dc_adjust: bool,
    wavelet_stages: u8,
    gop_length: u8,
}

impl MQ00EncoderConfig {
    /// Create new encoder config
    /// 
    /// # Errors
    /// - [`Error::ValueOutOfRange`] if `quantisation` doesn't fit in 1 byte
    /// - [`Error::ValueOutOfRange`] if `wavelet_stages` doesn't fit in 1 byte
    /// - [`Error::ValueOutOfRange`] if `gop_length` is 0
    pub fn new(
        quantisation: u8,
        dc_adjust: bool,
        wavelet_stages: u8,
        gop_length: u8,
    ) -> Result<Self> {
        Self::default()
            .with_quantisation(quantisation)?
            .with_dc_adjust(dc_adjust)
            .with_wavelet_stages(wavelet_stages)?
            .with_gop_length(gop_length)
    }

    /// Quantisation in block floating point exponent
    pub fn quantisation(&self) -> u8 {
        self.quantisation
    }

    /// Whether a DC adjustment is applied
    pub fn dc_adjust(&self) -> bool {
        self.dc_adjust
    }

    /// Number of wavelet coding stages
    pub fn wavelet_stages(&self) -> u8 {
        self.wavelet_stages
    }

    /// Length of group of frames
    pub fn gop_length(&self) -> u8 {
        self.gop_length
    }

    /// Quantisation in block floating point exponent
    /// 
    /// # Errors
    /// - [`Error::ValueOutOfRange`] if `bits` doesn't fit in 1 byte
    pub fn with_quantisation(mut self, bits: u8) -> Result<Self> {
        if bits & 0xF != 0 {
            return Err(Error::ValueOutOfRange {
                name: "bits",
                min: 0,
                max: 15,
                actual: u64::from(bits),
            });
        }
        self.quantisation = bits;
        Ok(self)
    }

    /// Whether a DC adjustment is applied
    pub fn with_dc_adjust(mut self, enabled: bool) -> Self {
        self.dc_adjust = enabled;
        self
    }

    /// Number of wavelet coding stages
    /// 
    /// # Errors
    /// - [`Error::ValueOutOfRange`] if `stages` doesn't fit in 1 byte
    pub fn with_wavelet_stages(mut self, stages: u8) -> Result<Self> {
        if stages & 0xF != 0 {
            return Err(Error::ValueOutOfRange {
                name: "stages",
                min: 0,
                max: 15,
                actual: u64::from(stages),
            });
        }
        self.wavelet_stages = stages;
        Ok(self)
    }

    /// Length of group of frames
    /// 
    /// # Errors
    /// - [`Error::ValueOutOfRange`] if `gop_length` is 0
    pub fn with_gop_length(mut self, frames: u8) -> Result<Self> {
        if frames == 0 {
            return Err(Error::ValueOutOfRange {
                name: "frames",
                min: 1,
                max: u64::from(u8::MAX),
                actual: u64::from(frames),
            });
        }
        self.gop_length = frames;
        Ok(self)
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

impl<const WIDTH: usize, const HEIGHT: usize> DDMEncoder<WIDTH, HEIGHT> for MQ00Encoder {
    type Packet = MQ00Packet<WIDTH, HEIGHT>;

    fn encode(&mut self, frame: &DDMFrame<WIDTH, HEIGHT>) -> Result<Self::Packet> {
        todo!()
    }
}

/// Decoder capable of converting encoded MQ00 packets into raw DDM frame data
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MQ00Decoder<const WIDTH: usize, const HEIGHT: usize> {}

impl<const WIDTH: usize, const HEIGHT: usize> MQ00Decoder<WIDTH, HEIGHT> {
    /// Create a new MQ00 decoder
    pub fn new() -> Self {
        todo!()
    }
}

impl<const WIDTH: usize, const HEIGHT: usize> DDMDecoder<WIDTH, HEIGHT>
    for MQ00Decoder<WIDTH, HEIGHT>
{
    type Packet = MQ00Packet<WIDTH, HEIGHT>;

    fn decode(&mut self, packet: Self::Packet) -> Result<DDMFrame<WIDTH, HEIGHT>> {
        todo!()
    }
}

/// Header for MQ00 packets
#[derive(BinRead, BinWrite, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(any(test, feature = "serde"), derive(proptest_derive::Arbitrary))]
#[brw(big)]
#[brw(magic = 0u8)] // Header version 0
pub struct MQ00Header {
    channel_num: u8,
    status_flag: u8,
    quantisation_wavelet_stages: u8,
    _reserved: u8, // For alignment, may be used later
    packet_kind: MQ00PacketKind,
    dc_offset: u16,
    timestamp_seconds: u32,
    timestamp_clocks: i32,
}

/// MQ00 packet type
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MQ00Packet<const WIDTH: usize, const HEIGHT: usize> {
    header: MQ00Header,
    pixel_data: Array2<i32>,
}

impl<const WIDTH: usize, const HEIGHT: usize> BinRead for MQ00Packet<WIDTH, HEIGHT> {
    type Args<'a> = ();

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::prelude::BinResult<Self> {
        todo!()
    }
}

impl<const WIDTH: usize, const HEIGHT: usize> BinWrite for MQ00Packet<WIDTH, HEIGHT> {
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

impl<const WIDTH: usize, const HEIGHT: usize> ReadEndian for MQ00Packet<WIDTH, HEIGHT> {
    const ENDIAN: EndianKind = EndianKind::Endian(Big);
}

impl<const WIDTH: usize, const HEIGHT: usize> WriteEndian for MQ00Packet<WIDTH, HEIGHT> {
    const ENDIAN: EndianKind = EndianKind::Endian(Big);
}

#[cfg(any(test, feature = "proptest"))]
impl<const WIDTH: usize, const HEIGHT: usize> proptest::arbitrary::Arbitrary
    for MQ00Packet<WIDTH, HEIGHT>
{
    type Parameters = ();
    type Strategy = proptest::strategy::BoxedStrategy<Self>;

    fn arbitrary_with((): Self::Parameters) -> Self::Strategy {
        use proptest::prelude::*;

        #[expect(clippy::expect_used, reason = "Calculation guarantees same size")]
        (any::<MQ00Header>())
            .prop_flat_map(|header| {
                (
                    Just(header),
                    proptest::collection::vec(any::<i32>(), WIDTH * HEIGHT),
                )
            })
            .prop_map(|(header, pixels)| Self {
                header,
                pixel_data: Array2::from_shape_vec((HEIGHT, WIDTH), pixels)
                    .expect("pixel count matches array dimensions"),
            })
            .boxed()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::io::Cursor;

    use proptest::prelude::*;

    use super::*;

    proptest! {
        #[test]
        fn full_round_trip(
            frames in proptest::collection::vec(any::<DDMFrame<20, 52>>(), 50..200),
            dc_adjust in any::<bool>(),
            wavelet_stages in 0..15u8,
            gop_length in 1..30u8
        ) {
            let config = MQ00EncoderConfig::new(0, dc_adjust, wavelet_stages, gop_length)?;
            let mut encoder = MQ00Encoder::new(&config);
            let mut buf = vec![];
            let mut cursor = Cursor::new(&mut buf);
            for frame in &frames {
                encoder.encode(frame)?.write(&mut cursor)?;
            }
            let mut decoder = MQ00Decoder::new();
            let mut decoded_frames = vec![];
            let mut cursor = Cursor::new(cursor.into_inner());
            loop {
                match MQ00Packet::read(&mut cursor) {
                    Ok(packet) => decoded_frames.push(decoder.decode(packet)?),
                    Err(binrw::Error::Io(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                    Err(e) => return Err(e.into())
                }
            }
            prop_assert_eq!(frames, decoded_frames);
        }

        #[test]
        fn encode_round_trip(
            frames in proptest::collection::vec(any::<DDMFrame<20, 52>>(), 50..200),
            dc_adjust in any::<bool>(),
            wavelet_stages in 0..15u8,
            gop_length in 1..30u8
        ) {
            let config = MQ00EncoderConfig::new(0, dc_adjust, wavelet_stages, gop_length)?;
            let mut encoder = MQ00Encoder::new(&config);
            let mut packets = vec![];
            for frame in &frames {
                packets.push(encoder.encode(frame)?);
            }
            let mut decoder = MQ00Decoder::new();
            let mut decoded_frames = vec![];
            for packet in packets {
                decoded_frames.push(decoder.decode(packet)?);
            }
            prop_assert_eq!(frames, decoded_frames);
        }

        #[test]
        fn bytestream_round_trip(packets in proptest::collection::vec(any::<MQ00Packet<20, 52>>(), 1..20)) {
            let mut buf = vec![];
            let mut cursor = Cursor::new(&mut buf);
            for packet in &packets {
                packet.write(&mut cursor)?;
            }
            let mut decoded_packets = vec![];
            let mut cursor = Cursor::new(cursor.into_inner());
            loop {
                match MQ00Packet::read(&mut cursor) {
                    Ok(packet) => decoded_packets.push(packet),
                    Err(binrw::Error::Io(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                    Err(e) => return Err(e.into())
                }
            }
            prop_assert_eq!(packets, decoded_packets);
        }
    }
}
