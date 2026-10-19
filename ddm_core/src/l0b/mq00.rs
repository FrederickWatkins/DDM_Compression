//! Expertimental MQ00 packet type

#[cfg(not(feature = "std"))]
use alloc::boxed::Box;
use core::iter;

use binrw::{
    BinRead, BinResult, BinWrite, Endian,
    io::{Read, Seek, Write},
    meta::{EndianKind, ReadEndian, WriteEndian, WriteMagic as _},
};
use bitstream_io::{BigEndian, BitReader, BitWrite, BitWriter};
use ndarray::{Array2, ArrayView2, ArrayViewMut2, Axis};

use crate::{
    coding::{
        cabac::{CabacContext, CabacDecoder, CabacEncoder},
        crc::{CrcReader, CrcWriter},
        wavelet::{dwt_53_nd, idwt_53_nd},
    },
    l0b::{DDMDecoder, DDMEncoder, DDMFrame, DDMFrameMetadata, EncodingError},
};

#[derive(BinRead, BinWrite, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[repr(u8)]
#[brw(repr = u8)]
pub enum MQ00PacketKind {
    Intra,
    Predictive,
}

/// Config for [`MQ00Encoder`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    /// - [`EncodingError::ConfigValueOutOfRange`] if `quantisation` is greater than 15
    /// - [`EncodingError::ConfigValueOutOfRange`] if `wavelet_stages` doesn't fit in 1 byte
    /// - [`EncodingError::ConfigValueOutOfRange`] if `gop_length` is 0
    pub fn new(
        quantisation: u8,
        dc_adjust: bool,
        wavelet_stages: u8,
        gop_length: u8,
    ) -> Result<Self, EncodingError> {
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
    /// - [`EncodingError::ConfigValueOutOfRange`] if `bits` is greater than 15
    pub fn with_quantisation(mut self, bits: u8) -> Result<Self, EncodingError> {
        if bits & 0xF0 != 0 {
            return Err(EncodingError::ConfigValueOutOfRange {
                name: "quantisation",
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
    /// - [`EncodingError::ConfigValueOutOfRange`] if `stages` doesn't fit in 1 byte
    pub fn with_wavelet_stages(mut self, stages: u8) -> Result<Self, EncodingError> {
        if stages & 0xF0 != 0 {
            return Err(EncodingError::ConfigValueOutOfRange {
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
    /// - [`EncodingError::ConfigValueOutOfRange`] if `frames` is 0
    pub fn with_gop_length(mut self, frames: u8) -> Result<Self, EncodingError> {
        if frames == 0 {
            return Err(EncodingError::ConfigValueOutOfRange {
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
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MQ00Encoder<const WIDTH: usize, const HEIGHT: usize> {
    config: MQ00EncoderConfig,
    prev_frame: [Array2<i32>; 16],
    packets_since_intra: [usize; 16],
}

impl<const WIDTH: usize, const HEIGHT: usize> MQ00Encoder<WIDTH, HEIGHT> {
    /// Create a new MQ00 encoder
    pub fn new(config: MQ00EncoderConfig) -> Self {
        let prev_frame = core::array::from_fn(|_| Array2::from_elem([HEIGHT, WIDTH], 0));
        Self {
            config,
            prev_frame,
            packets_since_intra: [0; 16],
        }
    }
}

impl<const WIDTH: usize, const HEIGHT: usize> DDMEncoder<WIDTH, HEIGHT>
    for MQ00Encoder<WIDTH, HEIGHT>
{
    type Packet = MQ00Packet<WIDTH, HEIGHT>;

    fn encode(
        &mut self,
        frame: DDMFrame<WIDTH, HEIGHT>,
    ) -> Result<MQ00Packet<WIDTH, HEIGHT>, EncodingError> {
        let packet_kind = if self.packets_since_intra[frame.metadata.channel as usize]
            < self.config.gop_length as usize - 1
        {
            self.packets_since_intra[frame.metadata.channel as usize] += 1;
            MQ00PacketKind::Predictive
        } else {
            self.packets_since_intra[frame.metadata.channel as usize] = 0;
            MQ00PacketKind::Intra
        };

        let mut pixel_data = match packet_kind {
            MQ00PacketKind::Intra => frame.pixel_data.mapv(i32::from),
            MQ00PacketKind::Predictive => {
                let previous = &self.prev_frame[frame.metadata.channel as usize];

                &frame.pixel_data.mapv(i32::from) - previous
            }
        };

        let dc_offset = if self.config.dc_adjust {
            let dc_offset_raw = dc_adjust(pixel_data.view_mut());
            dc_offset_raw
                .try_into()
                .map_err(|_| EncodingError::InvalidInput {
                    name: "dc_offset",
                    var_type: "u16",
                    value: i64::from(dc_offset_raw),
                })?
        } else {
            0
        };

        dwt(pixel_data.view_mut(), self.config.wavelet_stages);

        for pixel in &mut pixel_data {
            *pixel >>= self.config.quantisation;
        }

        let header = MQ00Header {
            channel: frame.metadata.channel,
            status_flag: frame.metadata.status_flag,
            quantisation_wavelet_stages: self.config.quantisation << 4 | self.config.wavelet_stages,
            bits_per_coefficient: 32 - self.config.quantisation,
            packet_kind,
            dc_offset,
            gps_seconds: frame.metadata.gps_seconds,
            sample_clocks: frame.metadata.sample_clocks,
        };

        Ok(Self::Packet { header, pixel_data })
    }
}

/// Decoder capable of converting encoded MQ00 packets into raw DDM frame data
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MQ00Decoder<const WIDTH: usize, const HEIGHT: usize> {
    prev_frame: [Array2<i32>; 16],
}

impl<const WIDTH: usize, const HEIGHT: usize> MQ00Decoder<WIDTH, HEIGHT> {
    /// Create a new MQ00 decoder
    pub fn new() -> Self {
        let prev_frame = core::array::from_fn(|_| Array2::from_elem([HEIGHT, WIDTH], 0));
        Self { prev_frame }
    }
}

impl<const WIDTH: usize, const HEIGHT: usize> DDMDecoder<WIDTH, HEIGHT>
    for MQ00Decoder<WIDTH, HEIGHT>
{
    type Packet = MQ00Packet<WIDTH, HEIGHT>;

    fn decode(&mut self, packet: Self::Packet) -> Result<DDMFrame<WIDTH, HEIGHT>, EncodingError> {
        let metadata = DDMFrameMetadata {
            channel: packet.header.channel,
            status_flag: packet.header.status_flag,
            gps_seconds: packet.header.gps_seconds,
            sample_clocks: packet.header.sample_clocks,
        };

        let stages = packet.header.quantisation_wavelet_stages & 0xF;
        let quantisation = (packet.header.quantisation_wavelet_stages >> 4) & 0xF;

        let mut pixel_data_i32 = packet.pixel_data;

        for pixel in &mut pixel_data_i32 {
            *pixel <<= quantisation;
        }

        idwt(pixel_data_i32.view_mut(), stages);

        inverse_dc_adjust(
            pixel_data_i32.view_mut(),
            i32::from(packet.header.dc_offset),
        );

        match packet.header.packet_kind {
            MQ00PacketKind::Intra => (),
            MQ00PacketKind::Predictive => {
                let previous = &self.prev_frame[packet.header.channel as usize];

                pixel_data_i32 += previous;
            }
        }

        let mut pixel_data = Array2::from_elem([HEIGHT, WIDTH], 0);

        for (pixel, old_pixel) in pixel_data.iter_mut().zip(pixel_data_i32) {
            let old_pixel = old_pixel.clamp(0, i32::from(u16::MAX));
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "The value is clamped to the u16 range"
            )]
            let old_pixel = old_pixel as u16;
            *pixel = old_pixel;
        }

        Ok(DDMFrame {
            metadata,
            pixel_data,
        })
    }
}

/// Header for MQ00 packets
#[derive(BinRead, BinWrite, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[brw(big)]
#[brw(magic = 0u8)] // Header version 0
pub struct MQ00Header {
    channel: u8,
    status_flag: u8,
    quantisation_wavelet_stages: u8,
    bits_per_coefficient: u8,
    packet_kind: MQ00PacketKind,
    dc_offset: u16,
    gps_seconds: u32,
    sample_clocks: i32,
}

#[cfg(any(test, feature = "proptest"))]
impl proptest::arbitrary::Arbitrary for MQ00Header {
    type Parameters = ();
    type Strategy = proptest::strategy::BoxedStrategy<Self>;

    fn arbitrary_with((): Self::Parameters) -> Self::Strategy {
        use proptest::prelude::*;

        (
            0..15u8,
            any::<u8>(),
            any::<u8>(),
            any::<MQ00PacketKind>(),
            any::<u16>(),
            any::<u32>(),
            any::<i32>(),
        )
            .prop_map(
                |(
                    channel,
                    status_flag,
                    quantisation_wavelet_stages,
                    packet_kind,
                    dc_offset,
                    gps_seconds,
                    sample_clocks,
                )| {
                    Self {
                        channel,
                        status_flag,
                        quantisation_wavelet_stages,
                        bits_per_coefficient: 32,
                        packet_kind,
                        dc_offset,
                        gps_seconds,
                        sample_clocks,
                    }
                },
            )
            .boxed()
    }
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
        endian: Endian,
        args: Self::Args<'_>,
    ) -> BinResult<Self> {
        let mut crc_prefix = [0u8; 8];
        crc_prefix[0..4].copy_from_slice(&super::L0bPacket::<WIDTH, HEIGHT>::MAGIC.to_be_bytes());
        crc_prefix[4..8].copy_from_slice(b"MQ00");
        let mut crc_reader = CrcReader::new_with_prefix(reader, &crc_prefix);
        let header = MQ00Header::read_options(&mut crc_reader, endian, args)?;

        let start_pos = crc_reader.stream_position()?;

        let mut ctx = CabacContext::new();
        let mut decoder = CabacDecoder::new(BitReader::<_, BigEndian>::new(crc_reader))?;

        let pixel_data = from_bitplanes::<WIDTH, HEIGHT, _>(
            iter::from_fn(|| Some(decoder.decode_decision(&mut ctx))),
            header.bits_per_coefficient,
        )?;
        let mut crc_reader = decoder.into_inner().into_reader();
        let end_pos = crc_reader.stream_position()?;

        let calculated_crc = crc_reader.finalize();

        // Align the complete packet after its CRC.
        let bytes_decoded = (end_pos - start_pos) + 4;

        // The CRC immediately follows the encoded pixels and is stored little-endian.
        let crc = u32::read_options(reader, Endian::Little, args)?;
        if crc != calculated_crc {
            return Err(binrw::Error::Custom {
                pos: end_pos,
                err: Box::new(EncodingError::CrcMismatch {
                    encoded_crc: crc,
                    calculated_crc,
                }),
            });
        }
        if !bytes_decoded.is_multiple_of(4) {
            let target_distance = 4 - (bytes_decoded % 4);
            #[expect(clippy::cast_possible_wrap, reason = "Target distance max value is 3")]
            reader.seek_relative(target_distance as i64)?;
        }

        Ok(Self { header, pixel_data })
    }
}

impl<const WIDTH: usize, const HEIGHT: usize> BinWrite for MQ00Packet<WIDTH, HEIGHT> {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        endian: Endian,
        args: Self::Args<'_>,
    ) -> BinResult<()> {
        let mut crc_prefix = [0u8; 8];
        crc_prefix[0..4].copy_from_slice(&super::L0bPacket::<WIDTH, HEIGHT>::MAGIC.to_be_bytes());
        crc_prefix[4..8].copy_from_slice(b"MQ00");
        let mut crc_writer = CrcWriter::new_with_prefix(writer, &crc_prefix);
        self.header.write_options(&mut crc_writer, endian, args)?;

        let start_pos = crc_writer.stream_position()?;

        let mut ctx = CabacContext::new();
        let mut encoder = CabacEncoder::new(BitWriter::<_, BigEndian>::new(crc_writer));

        for bit in to_bitplanes(&self.pixel_data.view(), self.header.bits_per_coefficient) {
            encoder.encode_decision(bit, &mut ctx)?;
        }

        encoder.encode_flush()?;
        let mut bitwriter = encoder.into_inner();
        bitwriter.byte_align()?;
        bitwriter.flush()?;

        let mut crc_writer = bitwriter.into_writer();
        let end_pos = crc_writer.stream_position()?;
        let crc = crc_writer.finalize();

        crc.write_options(writer, Endian::Little, args)?;

        // Align the complete packet after its CRC.
        let bytes_encoded = end_pos - start_pos + 4;
        if !bytes_encoded.is_multiple_of(4) {
            let target_distance = 4 - (bytes_encoded % 4);
            #[expect(
                clippy::cast_possible_truncation,
                reason = "Target distance max value is 3"
            )]
            writer.write_all(&[0u8; 3][..target_distance as usize])?;
        }

        Ok(())
    }
}

impl<const WIDTH: usize, const HEIGHT: usize> ReadEndian for MQ00Packet<WIDTH, HEIGHT> {
    const ENDIAN: EndianKind = EndianKind::Endian(Endian::Big);
}

impl<const WIDTH: usize, const HEIGHT: usize> WriteEndian for MQ00Packet<WIDTH, HEIGHT> {
    const ENDIAN: EndianKind = EndianKind::Endian(Endian::Big);
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

fn dc_adjust(mut pixel_data: ArrayViewMut2<i32>) -> i32 {
    let mean = pixel_data.mean().unwrap_or(0);
    pixel_data -= mean;
    mean
}

fn inverse_dc_adjust(mut pixel_data: ArrayViewMut2<i32>, offset: i32) {
    pixel_data += offset;
}

fn dwt(mut pixel_data: ArrayViewMut2<i32>, stages: u8) {
    let dim = pixel_data.dim();
    for i in 0..stages {
        let chunk_sizes = [dim.0 / (1 << i), dim.1 / (1 << i)];
        if chunk_sizes.contains(&0) {
            return;
        }
        if let Some(mut subband) = pixel_data
            .axis_chunks_iter_mut(Axis(0), chunk_sizes[0])
            .next()
        {
            if let Some(subband) = subband.axis_chunks_iter_mut(Axis(1), chunk_sizes[1]).next() {
                dwt_53_nd(subband, &[Axis(0), Axis(1)]);
            } else {
                return;
            }
        } else {
            return;
        }
    }
}

fn idwt(mut pixel_data: ArrayViewMut2<i32>, stages: u8) {
    let dim = pixel_data.dim();
    for i in (0..stages).rev() {
        let chunk_sizes = [dim.0 / (1 << i), dim.1 / (1 << i)];
        if chunk_sizes.contains(&0) {
            continue;
        }
        if let Some(mut subband) = pixel_data
            .axis_chunks_iter_mut(Axis(0), chunk_sizes[0])
            .next()
        {
            if let Some(subband) = subband.axis_chunks_iter_mut(Axis(1), chunk_sizes[1]).next() {
                idwt_53_nd(subband, &[Axis(0), Axis(1)]);
            } else {
                return;
            }
        } else {
            return;
        }
    }
}

fn to_bitplanes(
    pixel_data: &ArrayView2<i32>,
    bits_per_coefficient: u8,
) -> impl Iterator<Item = bool> {
    (0..bits_per_coefficient)
        .rev()
        .flat_map(move |current_bit| to_bitplane(pixel_data, current_bit))
        .chain(to_signplane(pixel_data))
}

fn from_bitplanes<
    const WIDTH: usize,
    const HEIGHT: usize,
    I: Iterator<Item = binrw::io::Result<bool>>,
>(
    mut bits: I,
    bits_per_coefficient: u8,
) -> binrw::io::Result<Array2<i32>> {
    let mut output = Array2::from_elem([HEIGHT, WIDTH], 0);
    for current_bit in (0..bits_per_coefficient).rev() {
        from_bitplane(&mut bits, current_bit, output.view_mut())?;
    }
    from_signplane(&mut bits, output.view_mut())?;

    Ok(output)
}

fn to_bitplane(pixel_data: &ArrayView2<i32>, current_bit: u8) -> impl Iterator<Item = bool> {
    pixel_data
        .into_iter()
        .map(move |&x| (x.unsigned_abs() & (1_u32 << current_bit)) != 0)
}

fn to_signplane(pixel_data: &ArrayView2<i32>) -> impl Iterator<Item = bool> {
    pixel_data.into_iter().map(|&x| x < 0)
}

fn from_bitplane<I: Iterator<Item = binrw::io::Result<bool>>>(
    bits: &mut I,
    current_bit: u8,
    pixel_data: ArrayViewMut2<i32>,
) -> binrw::io::Result<()> {
    for (pixel, bit) in pixel_data.into_iter().zip(bits) {
        let bit = bit?;
        if bit {
            *pixel |= 1 << current_bit;
        }
    }

    Ok(())
}

fn from_signplane<I: Iterator<Item = binrw::io::Result<bool>>>(
    bits: &mut I,
    pixel_data: ArrayViewMut2<i32>,
) -> binrw::io::Result<()> {
    for (pixel, bit) in pixel_data.into_iter().zip(bits) {
        let bit = bit?;
        if bit {
            *pixel = -*pixel;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{io::Cursor, vec};

    use proptest::prelude::*;

    use super::*;

    proptest! {
        #[test]
        fn full_round_trip(
            frames in proptest::collection::vec(any::<DDMFrame<20, 52>>(), 1..64),
            dc_adjust in any::<bool>(),
            wavelet_stages in 0..16u8,
            gop_length in 1..=32u8
        ) {
            let config = MQ00EncoderConfig::new(0, dc_adjust, wavelet_stages, gop_length)?;
            let mut encoder = MQ00Encoder::new(config);
            let mut buf = vec![];
            let mut cursor = Cursor::new(&mut buf);
            for frame in frames.clone() {
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
            frames in proptest::collection::vec(any::<DDMFrame<20, 52>>(), 1..64),
            dc_adjust in any::<bool>(),
            wavelet_stages in 0..16u8,
            gop_length in 1..=32u8
        ) {
            let config = MQ00EncoderConfig::new(0, dc_adjust, wavelet_stages, gop_length)?;
            let mut encoder = MQ00Encoder::new(config);
            let mut packets = vec![];
            for frame in frames.clone() {
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
        fn bytestream_round_trip(packets in proptest::collection::vec(any::<MQ00Packet<20, 52>>(), 1..64)) {
            let mut buf = vec![];
            let mut cursor = Cursor::new(&mut buf);
            for packet in &packets {
                packet.write(&mut cursor)?;
            }
            let mut decoded_packets = vec![];
            let mut cursor = Cursor::new(cursor.into_inner());
            for _ in 0..packets.len() {
                decoded_packets.push(MQ00Packet::read(&mut cursor)?);
            }
            prop_assert_eq!(packets, decoded_packets);
        }

        #[test]
        fn any_config_errors(
            frames in proptest::collection::vec(any::<DDMFrame<20, 52>>(), 1..64),
            quantisation in 0..16u8,
            dc_adjust in any::<bool>(),
            wavelet_stages in 0..16u8
        ) {
            let config = MQ00EncoderConfig::new(quantisation, dc_adjust, wavelet_stages, 1)?;
            let mut encoder = MQ00Encoder::new(config);
            let mut buf = vec![];
            let mut cursor = Cursor::new(&mut buf);
            for frame in frames.clone() {
                encoder.encode(frame)?.write(&mut cursor)?;
            }
            let mut decoder: MQ00Decoder<20, 52> = MQ00Decoder::new();
            let mut decoded_frames = vec![];
            let mut cursor = Cursor::new(cursor.into_inner());
            loop {
                match MQ00Packet::read(&mut cursor) {
                    Ok(packet) => decoded_frames.push(decoder.decode(packet)?),
                    Err(binrw::Error::Io(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                    Err(e) => return Err(e.into())
                }
            }
            prop_assert_eq!(frames.len(), decoded_frames.len());


        }
    }
}
