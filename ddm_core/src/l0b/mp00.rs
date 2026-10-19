//! MP00 legacy packet type

#[cfg(not(feature = "std"))]
use alloc::boxed::Box;

use binrw::{
    BinRead, BinResult, BinWrite,
    Endian::{Big, Little},
    io::{Read, Seek, Write},
    meta::{EndianKind, ReadEndian, WriteEndian, WriteMagic as _},
};
use ndarray::{Array2, ArrayView2, Dim, ShapeBuilder};

use crate::{
    coding::crc::{CrcReader, CrcWriter},
    l0b::{DDMDecoder, DDMEncoder, DDMFrame, DDMFrameMetadata, EncodingError},
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
        Self { pixel_quantisation }
    }
}

impl<const WIDTH: usize, const HEIGHT: usize> DDMEncoder<WIDTH, HEIGHT>
    for MP00Encoder<WIDTH, HEIGHT>
{
    type Packet = MP00Packet<WIDTH, HEIGHT>;

    fn encode(&mut self, frame: DDMFrame<WIDTH, HEIGHT>) -> Result<Self::Packet, EncodingError> {
        let header = MP00Header {
            channel: frame.metadata.channel,
            status_flag: frame.metadata.status_flag,
            pixel_quantisation: self.pixel_quantisation,
            gps_seconds: frame.metadata.gps_seconds,
            sample_clocks: frame.metadata.sample_clocks,
        };
        Ok(Self::Packet {
            header,
            pixel_data: frame.pixel_data.mapv(|x| x >> self.pixel_quantisation),
        })
    }
}

/// Encoder capable of converting encoded MP00 packets into raw DDM frame data
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MP00Decoder<const WIDTH: usize, const HEIGHT: usize>;

impl<const WIDTH: usize, const HEIGHT: usize> MP00Decoder<WIDTH, HEIGHT> {
    /// Create a new MP00 decoder
    pub fn new() -> Self {
        Self
    }
}

impl<const WIDTH: usize, const HEIGHT: usize> DDMDecoder<WIDTH, HEIGHT>
    for MP00Decoder<WIDTH, HEIGHT>
{
    type Packet = MP00Packet<WIDTH, HEIGHT>;

    fn decode(&mut self, packet: Self::Packet) -> Result<DDMFrame<WIDTH, HEIGHT>, EncodingError> {
        let metadata = DDMFrameMetadata {
            channel: packet.header.channel,
            status_flag: packet.header.status_flag,
            gps_seconds: packet.header.gps_seconds,
            sample_clocks: packet.header.sample_clocks,
        };
        Ok(DDMFrame {
            metadata,
            pixel_data: packet
                .pixel_data
                .mapv(|x| x << packet.header.pixel_quantisation),
        })
    }
}

/// Header for MP00 packets
#[derive(BinRead, BinWrite, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[brw(big)]
#[brw(magic = 2u8)] // Header version 2
pub struct MP00Header {
    channel: u8,
    status_flag: u8,
    pixel_quantisation: u8,
    gps_seconds: u32,
    sample_clocks: i32,
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

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> BinResult<Self> {
        let mut crc_prefix = [0u8; 8];
        crc_prefix[0..4].copy_from_slice(&super::L0bPacket::<WIDTH, HEIGHT>::MAGIC.to_be_bytes());
        crc_prefix[4..8].copy_from_slice(b"MP00");
        let mut crc_reader = CrcReader::new_with_prefix(reader, &crc_prefix);
        let header = MP00Header::read_options(&mut crc_reader, endian, args)?;

        let start_pos = crc_reader.stream_position()?;
        let pixel_data = decode_pixel_data(&mut crc_reader, [HEIGHT, WIDTH])?;
        let end_pos = crc_reader.stream_position()?;

        let calculated_crc = crc_reader.finalize();

        // The CRC immediately follows the encoded pixels and is stored little-endian.
        let crc = u32::read_options(reader, Little, args)?;
        if crc != calculated_crc {
            return Err(binrw::Error::Custom {
                pos: end_pos,
                err: Box::new(EncodingError::CrcMismatch {
                    encoded_crc: crc,
                    calculated_crc,
                }),
            });
        }

        // Align the complete packet after its CRC.
        let bytes_with_crc = (end_pos - start_pos) + 4;
        if !bytes_with_crc.is_multiple_of(4) {
            let target_distance = 4 - (bytes_with_crc % 4);
            #[expect(clippy::cast_possible_wrap, reason = "Target distance max value is 3")]
            reader.seek_relative(target_distance as i64)?;
        }

        Ok(Self { header, pixel_data })
    }
}

impl<const WIDTH: usize, const HEIGHT: usize> BinWrite for MP00Packet<WIDTH, HEIGHT> {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> BinResult<()> {
        let mut crc_prefix = [0u8; 8];
        crc_prefix[0..4].copy_from_slice(&super::L0bPacket::<WIDTH, HEIGHT>::MAGIC.to_be_bytes());
        crc_prefix[4..8].copy_from_slice(b"MP00");
        let mut crc_writer = CrcWriter::new_with_prefix(writer, &crc_prefix);
        self.header.write_options(&mut crc_writer, endian, args)?;

        let start_pos = crc_writer.stream_position()?;
        encode_pixel_data(&mut crc_writer, &self.pixel_data.view())?;
        let end_pos = crc_writer.stream_position()?;

        let crc = crc_writer.finalize();

        crc.write_options(writer, Little, args)?;

        // Align the complete packet after its CRC.
        let bytes_encoded = end_pos - start_pos;
        let bytes_with_crc = bytes_encoded + 4;
        if !bytes_with_crc.is_multiple_of(4) {
            let target_distance = 4 - (bytes_with_crc % 4);
            #[expect(
                clippy::cast_possible_truncation,
                reason = "Target distance max value is 3"
            )]
            writer.write_all(&[0u8; 3][..target_distance as usize])?;
        }
        Ok(())
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

/// Decode pixel data using sign extended VRQ format
///
/// Returns pixel data and number of bytes read
fn decode_pixel_data<R: Read, Sh>(
    reader: &mut R,
    shape: Sh,
) -> Result<Array2<u16>, binrw::io::Error>
where
    Sh: ShapeBuilder<Dim = Dim<[usize; 2]>>,
{
    let mut output = Array2::from_elem(shape, 0);
    let mut prev_pixel = 0u16;
    for pixel in &mut output {
        let delta_value = decode_vrq(reader)?;
        *pixel = prev_pixel.overflowing_add(delta_value.cast_unsigned()).0;
        prev_pixel = *pixel;
    }
    Ok(output)
}

/// Encode pixel data using sign extended VRQ format
///
/// Returns number of bytes written
fn encode_pixel_data<W: Write>(
    writer: &mut W,
    pixel_data: &ArrayView2<u16>,
) -> Result<(), binrw::io::Error> {
    let mut prev_pixel = 0u16;
    for pixel in pixel_data {
        let delta_value: i16 = pixel.overflowing_sub(prev_pixel).0.cast_signed();
        encode_vrq(writer, delta_value)?;
        prev_pixel = *pixel;
    }

    Ok(())
}

/// Decode an i16 from 3 byte VRQ format
///
/// Returns result and number of bytes read
fn decode_vrq<R: Read>(reader: &mut R) -> Result<i16, binrw::io::Error> {
    let mut bytes = [0u8; 3];
    reader.read_exact(&mut bytes[0..1])?;
    if bytes[0] >> 7 == 1 {
        reader.read_exact(&mut bytes[1..2])?;
        if bytes[1] >> 7 == 1 {
            reader.read_exact(&mut bytes[2..3])?;
            // Pass width 16 for 3-byte i16 values
            Ok(sign_extend(&bytes[..3], 16))
        } else {
            Ok(sign_extend(&bytes[..2], 14))
        }
    } else {
        Ok(sign_extend(&bytes[..1], 7))
    }
}

/// Encode an i16 into 3 byte VRQ format
///
/// Returns number of bytes written
fn encode_vrq<W: Write>(writer: &mut W, val: i16) -> Result<(), binrw::io::Error> {
    #![allow(clippy::unwrap_used, reason = "Guaranteed to fit by &")]
    if (-64..=63).contains(&val) {
        let b0 = u8::try_from(val.cast_unsigned() & 0x7F).unwrap();
        writer.write_all(&[b0])?;
        Ok(())
    } else if (-8192..=8191).contains(&val) {
        let u = val.cast_unsigned();
        let b0 = u8::try_from(u & 0x7F).unwrap() | 0x80;
        let b1 = u8::try_from((u >> 7) & 0x7F).unwrap();
        writer.write_all(&[b0, b1])?;
        Ok(())
    } else {
        let u = u32::from(val.cast_unsigned());
        let b0 = u8::try_from(u & 0x7F).unwrap() | 0x80;
        let b1 = u8::try_from((u >> 7) & 0x7F).unwrap() | 0x80;
        let b2 = u8::try_from(u >> 14).unwrap() & 0x7F;
        writer.write_all(&[b0, b1, b2])?;
        Ok(())
    }
}

/// Sign extend a signed integer of arbitrary width, using only the lower seven bits in each byte
fn sign_extend(bytes: &[u8], width: usize) -> i16 {
    let mut output = 0i16;
    let num_bytes = bytes.len();
    for (i, byte) in bytes.iter().enumerate().take(num_bytes) {
        output |= i16::from(byte & 0x7F) << (7 * i);
    }
    // Only sign-extend if width is less than 16
    if width < 16 && (output >> (width - 1)) & 1 == 1 {
        output |= -1 << width;
    }
    output
}

#[cfg(test)]
mod tests {
    use std::{io::Cursor, vec};

    use proptest::prelude::*;

    use super::*;

    proptest! {
        #[test]
        fn full_round_trip(frames in proptest::collection::vec(any::<DDMFrame<20, 52>>(), 1..64)) {
            let mut encoder = MP00Encoder::new(0);
            let mut buf = vec![];
            let mut cursor = Cursor::new(&mut buf);
            for frame in frames.clone() {
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
        fn encode_round_trip(frames in proptest::collection::vec(any::<DDMFrame<20, 52>>(), 1..64)) {
            let mut encoder = MP00Encoder::new(0);
            let mut packets = vec![];
            for frame in frames.clone() {
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
        fn bytestream_round_trip(packets in proptest::collection::vec(any::<MP00Packet<20, 52>>(), 1..64)) {
            let mut buf = vec![];
            let mut cursor = Cursor::new(&mut buf);
            for packet in &packets {
                packet.write(&mut cursor)?;
            }
            let mut decoded_packets = vec![];
            let mut cursor = Cursor::new(cursor.into_inner());
            for _ in 0..packets.len() {
                decoded_packets.push(MP00Packet::read(&mut cursor)?);
            }
            prop_assert_eq!(packets, decoded_packets);
        }
    }
}
