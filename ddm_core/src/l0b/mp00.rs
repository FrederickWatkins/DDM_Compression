//! MP00 legacy packet type

use binrw::{
    BinRead, BinResult, BinWrite,
    Endian::{Big, Little},
    io::{Read, Seek, Write},
    meta::{EndianKind, ReadEndian, WriteEndian, WriteMagic as _},
};
use ndarray::{Array2, ArrayView2, Dim, ShapeBuilder};
use snafu::Backtrace;

use crate::{
    coding::{CrcReader, CrcWriter},
    l0b::{DDMDecoder, DDMEncoder, DDMFrame, DDMFrameMetadata, Error},
};

/// Config for [`MP00Encoder`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MP00EncoderConfig {
    quantisation: u8,
}

impl MP00EncoderConfig {
    /// Create new encoder config
    pub fn new(quantisation: u8) -> Self {
        Self::default().with_quantisation(quantisation)
    }

    /// Quantisation in block floating point exponent
    pub fn quantisation(&self) -> u8 {
        self.quantisation
    }

    /// Quantisation in block floating point exponent
    pub fn with_quantisation(mut self, bits: u8) -> Self {
        self.quantisation = bits;
        self
    }
}

/// Encoder capable of converting raw DDM frame data into encoded MP00 packets
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MP00Encoder {
    config: MP00EncoderConfig,
}

impl MP00Encoder {
    /// Create a new MP00 encoder
    pub fn new(config: MP00EncoderConfig) -> Self {
        Self { config }
    }
}

impl DDMEncoder
    for MP00Encoder
{
    type Packet = MP00Packet;

    fn encode(&mut self, frame: DDMFrame) -> Result<Self::Packet, Error> {
        let header = MP00Header {
            channel: frame.metadata.channel,
            status_flag: frame.metadata.status_flag,
            quantisation: self.config.quantisation,
            gps_seconds: frame.metadata.gps_seconds,
            sample_clocks: frame.metadata.sample_clocks,
        };
        Ok(Self::Packet {
            header,
            pixel_data: frame.pixel_data.mapv(|x| x >> self.config.quantisation),
        })
    }
}

/// Encoder capable of converting encoded MP00 packets into raw DDM frame data
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MP00Decoder;

impl MP00Decoder {
    /// Create a new MP00 decoder
    pub fn new() -> Self {
        Self
    }
}

impl DDMDecoder
    for MP00Decoder
{
    type Packet = MP00Packet;

    fn decode(&mut self, packet: Self::Packet) -> Result<DDMFrame, Error> {
        let metadata = packet.header.into();
        Ok(DDMFrame {
            metadata,
            pixel_data: packet.pixel_data.mapv(|x| x << packet.header.quantisation),
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
    quantisation: u8,
    gps_seconds: u32,
    sample_clocks: i32,
}

impl From<MP00Header> for DDMFrameMetadata {
    fn from(value: MP00Header) -> Self {
        Self {
            channel: value.channel,
            status_flag: value.status_flag,
            gps_seconds: value.gps_seconds,
            sample_clocks: value.sample_clocks,
        }
    }
}

/// MP00 packet type
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MP00Packet {
    header: MP00Header,
    pixel_data: Array2<u16>,
}

impl MP00Packet {
    /// Packet header
    pub fn header(&self) -> MP00Header {
        self.header
    }
}

impl BinRead for MP00Packet {
    type Args<'a> = (usize, usize);

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> BinResult<Self> {
        let mut crc_prefix = [0u8; 8];
        crc_prefix[0..4].copy_from_slice(&super::L0bPacket::MAGIC.to_be_bytes());
        crc_prefix[4..8].copy_from_slice(b"MP00");
        let mut crc_reader = CrcReader::new_with_prefix(reader, &crc_prefix);
        let header = MP00Header::read_options(&mut crc_reader, endian, ())?;

        let start_pos = crc_reader.stream_position()?;
        let pixel_data = decode_pixel_data(&mut crc_reader, args)?;
        let end_pos = crc_reader.stream_position()?;

        let calculated_crc = crc_reader.finalize();

        // The CRC immediately follows the encoded pixels and is stored little-endian.
        let crc = u32::read_options(reader, Little, ())?;
        if crc != calculated_crc {
            return Err(binrw::Error::Custom {
                pos: end_pos,
                err: Box::new(Error::CrcMismatch {
                    calculated_crc,
                    actual_crc: crc,
                    backtrace: Backtrace::new()
                }),
            });
        }

        // Align the complete packet after its CRC.
        let bytes_with_crc = end_pos
            .checked_sub(start_pos)
            .and_then(|bytes| bytes.checked_add(4))
            .ok_or_else(|| binrw::Error::Custom {
                pos: end_pos,
                err: Box::new(binrw::io::Error::new(
                    binrw::io::ErrorKind::InvalidData,
                    "invalid MP00 packet position",
                )),
            })?;
        if !bytes_with_crc.is_multiple_of(4) {
            let target_distance = match bytes_with_crc % 4 {
                1 => 3_i64,
                2 => 2,
                3 => 1,
                _ => 0,
            };
            reader.seek_relative(target_distance)?;
        }

        Ok(Self { header, pixel_data })
    }
}

impl BinWrite for MP00Packet {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> BinResult<()> {
        let mut crc_prefix = [0u8; 8];
        crc_prefix[0..4].copy_from_slice(&super::L0bPacket::MAGIC.to_be_bytes());
        crc_prefix[4..8].copy_from_slice(b"MP00");
        let mut crc_writer = CrcWriter::new_with_prefix(writer, &crc_prefix);
        self.header.write_options(&mut crc_writer, endian, args)?;

        let start_pos = crc_writer.stream_position()?;
        encode_pixel_data(&mut crc_writer, &self.pixel_data.view())?;
        let end_pos = crc_writer.stream_position()?;

        let crc = crc_writer.finalize();

        crc.write_options(writer, Little, args)?;

        // Align the complete packet after its CRC.
        let bytes_with_crc = end_pos
            .checked_sub(start_pos)
            .and_then(|bytes| bytes.checked_add(4))
            .ok_or_else(|| binrw::Error::Custom {
                pos: end_pos,
                err: Box::new(binrw::io::Error::new(
                    binrw::io::ErrorKind::InvalidData,
                    "invalid MP00 packet position",
                )),
            })?;
        if !bytes_with_crc.is_multiple_of(4) {
            let target_distance = match bytes_with_crc % 4 {
                1 => 3,
                2 => 2,
                3 => 1,
                _ => 0,
            };
            for _ in 0..target_distance {
                writer.write_all(&[0])?;
            }
        }
        Ok(())
    }
}

impl ReadEndian for MP00Packet {
    const ENDIAN: EndianKind = EndianKind::Endian(Big);
}

impl WriteEndian for MP00Packet {
    const ENDIAN: EndianKind = EndianKind::Endian(Big);
}

#[cfg(test)]
impl proptest::arbitrary::Arbitrary
    for MP00Packet
{
    type Parameters = (usize, usize);
    type Strategy = proptest::strategy::BoxedStrategy<Self>;

    fn arbitrary_with(parameters: Self::Parameters) -> Self::Strategy {
        use proptest::prelude::*;
        (any::<MP00Header>())
            .prop_flat_map(move |header| {
                (
                    Just(header),
                    proptest::collection::vec(any::<u16>(), parameters.0.saturating_mul(parameters.1)),
                )
            })
            .prop_filter_map(
                "frame dimensions must be representable",
                move |(header, pixels)| {
                    Array2::from_shape_vec(parameters, pixels)
                        .ok()
                        .map(|pixel_data| Self { header, pixel_data })
                },
            )
            .boxed()
    }
}

/// Decode pixel data using sign extended VRQ format
///
/// Returns pixel data and number of bytes read
pub fn decode_pixel_data<R: Read, Sh>(
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
        *pixel = prev_pixel.wrapping_add(delta_value.cast_unsigned());
        prev_pixel = *pixel;
    }
    Ok(output)
}

/// Encode pixel data using sign extended VRQ format
///
/// Returns number of bytes written
pub fn encode_pixel_data<W: Write>(
    writer: &mut W,
    pixel_data: &ArrayView2<u16>,
) -> Result<(), binrw::io::Error> {
    let mut prev_pixel = 0u16;
    for pixel in pixel_data {
        let delta_value: i16 = pixel.wrapping_sub(prev_pixel).cast_signed();
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
            sign_extend(bytes.get(..3).ok_or_else(invalid_vrq_width)?, 16)
        } else {
            sign_extend(bytes.get(..2).ok_or_else(invalid_vrq_width)?, 14)
        }
    } else {
        sign_extend(bytes.get(..1).ok_or_else(invalid_vrq_width)?, 7)
    }
}

/// Encode an i16 into 3 byte VRQ format
///
/// Returns number of bytes written
fn encode_vrq<W: Write>(writer: &mut W, val: i16) -> Result<(), binrw::io::Error> {
    if (-64..=63).contains(&val) {
        let b0 = encode_vrq_byte(u32::from(val.cast_unsigned() & 0x7F))?;
        writer.write_all(&[b0])?;
        Ok(())
    } else if (-8192..=8191).contains(&val) {
        let u = u32::from(val.cast_unsigned());
        let b0 = encode_vrq_byte(u & 0x7F)? | 0x80;
        let b1 = encode_vrq_byte((u >> 7) & 0x7F)?;
        writer.write_all(&[b0, b1])?;
        Ok(())
    } else {
        let u = u32::from(val.cast_unsigned());
        let b0 = encode_vrq_byte(u & 0x7F)? | 0x80;
        let b1 = encode_vrq_byte((u >> 7) & 0x7F)? | 0x80;
        let b2 = encode_vrq_byte(u >> 14)? & 0x7F;
        writer.write_all(&[b0, b1, b2])?;
        Ok(())
    }
}

fn encode_vrq_byte(value: u32) -> Result<u8, binrw::io::Error> {
    u8::try_from(value).map_err(|_| {
        binrw::io::Error::new(binrw::io::ErrorKind::InvalidData, "VRQ byte exceeds 7 bits")
    })
}

/// Sign extend a signed integer of arbitrary width, using only the lower seven bits in each byte
fn sign_extend(bytes: &[u8], width: usize) -> Result<i16, binrw::io::Error> {
    let width = u32::try_from(width).map_err(|_| invalid_vrq_width())?;
    if !(1..=16).contains(&width) || bytes.len() > 3 {
        return Err(invalid_vrq_width());
    }
    let mut output = 0i16;
    let num_bytes = bytes.len();
    for (i, byte) in bytes.iter().enumerate().take(num_bytes) {
        let shift = i
            .checked_mul(7)
            .and_then(|shift| u32::try_from(shift).ok())
            .ok_or_else(invalid_vrq_width)?;
        let bits = i16::from(byte & 0x7F)
            .checked_shl(shift)
            .ok_or_else(invalid_vrq_width)?;
        output |= bits;
    }
    // Only sign-extend if width is less than 16
    let sign_bit = width.checked_sub(1).ok_or_else(invalid_vrq_width)?;
    if width < 16 && output.checked_shr(sign_bit).ok_or_else(invalid_vrq_width)? & 1 == 1 {
        output |= (-1_i16).checked_shl(width).ok_or_else(invalid_vrq_width)?;
    }
    Ok(output)
}

fn invalid_vrq_width() -> binrw::io::Error {
    binrw::io::Error::new(binrw::io::ErrorKind::InvalidData, "invalid VRQ value width")
}

#[cfg(test)]
mod tests {
    use std::{io::Cursor, vec};

    use proptest::prelude::*;

    use super::*;

    proptest! {
        #[test]
        fn full_round_trip(frames in proptest::collection::vec(any::<DDMFrame>(), 1..64)) {
            let mut encoder = MP00Encoder::new(MP00EncoderConfig::new(0));
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
        fn encode_round_trip(frames in proptest::collection::vec(any::<DDMFrame>(), 1..64)) {
            let mut encoder = MP00Encoder::new(MP00EncoderConfig::new(0));
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
        fn bytestream_round_trip(packets in proptest::collection::vec(any::<MP00Packet>(), 1..64)) {
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