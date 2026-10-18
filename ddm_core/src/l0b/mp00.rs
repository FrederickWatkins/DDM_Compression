//! MP00 legacy packet type

use std::io::{Seek, Write};

use binrw::{
    BinRead, BinWrite,
    Endian::Big,
    meta::{EndianKind, ReadEndian, WriteEndian},
};
use ndarray::Array2;

use crate::{
    error::Result,
    l0b::{DDMDecoder, DDMEncoder, DDMFrame},
};

/// Encoder capable of converting raw DDM frame data into encoded MP00 packets
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MP00Encoder<const WIDTH: usize, const HEIGHT: usize> {
    pixel_quantisation: u8,
}

impl<const WIDTH: usize, const HEIGHT: usize> MP00Encoder<WIDTH, HEIGHT> {
    /// Create a new MP00 encoder
    pub fn new(pixel_quantisation: u8) -> Self {
        todo!()
    }
}

impl<const WIDTH: usize, const HEIGHT: usize> DDMEncoder<WIDTH, HEIGHT>
    for MP00Encoder<WIDTH, HEIGHT>
{
    type Packet = MP00Packet<WIDTH, HEIGHT>;

    fn encode(&mut self, frame: &DDMFrame<WIDTH, HEIGHT>) -> Result<Self::Packet> {
        todo!()
    }
}

/// Encoder capable of converting encoded MP00 packets into raw DDM frame data
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MP00Decoder<const WIDTH: usize, const HEIGHT: usize> {}

impl<const WIDTH: usize, const HEIGHT: usize> MP00Decoder<WIDTH, HEIGHT> {
    /// Create a new MP00 decoder
    pub fn new() -> Self {
        todo!()
    }
}

impl<const WIDTH: usize, const HEIGHT: usize> DDMDecoder<WIDTH, HEIGHT>
    for MP00Decoder<WIDTH, HEIGHT>
{
    type Packet = MP00Packet<WIDTH, HEIGHT>;

    fn decode(&mut self, packet: Self::Packet) -> Result<DDMFrame<WIDTH, HEIGHT>> {
        todo!()
    }
}

/// Header for MP00 packets
#[derive(BinRead, BinWrite, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(any(test, feature = "serde"), derive(proptest_derive::Arbitrary))]
#[brw(big)]
#[brw(magic = 2u8)] // Header version 2
pub struct MP00Header {
    channel_num: u8,
    status_flag: u8,
    pixel_quantisation: u8,
    timestamp_seconds: u32,
    timestamp_clocks: i32,
}

/// MP00 packet type
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MP00Packet<const WIDTH: usize, const HEIGHT: usize> {
    header: MP00Header,
    pixel_data: Array2<u16>,
}

impl<const WIDTH: usize, const HEIGHT: usize> BinRead for MP00Packet<WIDTH, HEIGHT> {
    type Args<'a> = ();

    fn read_options<R: std::io::prelude::Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::prelude::BinResult<Self> {
        todo!()
    }
}

impl<const WIDTH: usize, const HEIGHT: usize> BinWrite for MP00Packet<WIDTH, HEIGHT> {
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

impl<const WIDTH: usize, const HEIGHT: usize> ReadEndian for MP00Packet<WIDTH, HEIGHT> {
    const ENDIAN: EndianKind = EndianKind::Endian(Big);
}

impl<const WIDTH: usize, const HEIGHT: usize> WriteEndian for MP00Packet<WIDTH, HEIGHT> {
    const ENDIAN: EndianKind = EndianKind::Endian(Big);
}

#[cfg(any(test, feature = "proptest"))]
impl<const WIDTH: usize, const HEIGHT: usize> proptest::arbitrary::Arbitrary
    for MP00Packet<WIDTH, HEIGHT>
{
    type Parameters = ();
    type Strategy = proptest::strategy::BoxedStrategy<Self>;

    fn arbitrary_with((): Self::Parameters) -> Self::Strategy {
        use proptest::prelude::*;
        #[expect(clippy::expect_used, reason = "Calculation guarantees same size")]
        (any::<MP00Header>())
            .prop_flat_map(|header| {
                (
                    Just(header),
                    proptest::collection::vec(any::<u16>(), WIDTH * HEIGHT),
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
        fn full_round_trip(frames in proptest::collection::vec(any::<DDMFrame<20, 52>>(), 1..20)) {
            let mut encoder = MP00Encoder::new(0);
            let mut buf = vec![];
            let mut cursor = Cursor::new(&mut buf);
            for frame in &frames {
                encoder.encode(frame)?.write(&mut cursor)?;
            }
            let mut decoder = MP00Decoder::new();
            let mut decoded_frames = vec![];
            let mut cursor = Cursor::new(cursor.into_inner());
            loop {
                match MP00Packet::read(&mut cursor) {
                    Ok(packet) => decoded_frames.push(decoder.decode(packet)?),
                    Err(binrw::Error::Io(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                    Err(e) => return Err(e.into())
                }
            }
            prop_assert_eq!(frames, decoded_frames);
        }

        #[test]
        fn encode_round_trip(frames in proptest::collection::vec(any::<DDMFrame<20, 52>>(), 1..20)) {
            let mut encoder = MP00Encoder::new(0);
            let mut packets = vec![];
            for frame in &frames {
                packets.push(encoder.encode(frame)?);
            }
            let mut decoder = MP00Decoder::new();
            let mut decoded_frames = vec![];
            for packet in packets {
                decoded_frames.push(decoder.decode(packet)?);
            }
            prop_assert_eq!(frames, decoded_frames);
        }

        #[test]
        fn bytestream_round_trip(packets in proptest::collection::vec(any::<MP00Packet<20, 52>>(), 1..20)) {
            let mut buf = vec![];
            let mut cursor = Cursor::new(&mut buf);
            for packet in &packets {
                packet.write(&mut cursor)?;
            }
            let mut decoded_packets = vec![];
            let mut cursor = Cursor::new(cursor.into_inner());
            loop {
                match MP00Packet::read(&mut cursor) {
                    Ok(packet) => decoded_packets.push(packet),
                    Err(binrw::Error::Io(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                    Err(e) => return Err(e.into())
                }
            }
            prop_assert_eq!(packets, decoded_packets);
        }
    }
}
