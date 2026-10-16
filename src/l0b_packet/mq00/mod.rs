//! Experimental MQ00 packet type
//! ```
//! use ddm_compression::{
//!     DDMPixelData,
//!     l0b_packet::{Decode, Encode, mp00::MP00, mq00::MQ00},
//! };
//! use rand::rngs::StdRng;
//! use rand::{RngExt, SeedableRng};
//! use std::io::Cursor;
//!
//! let mut pixel_data: DDMPixelData = [[0; _]; _];
//!
//! let mut rng = StdRng::seed_from_u64(42);
//! for row in &mut pixel_data {
//!     for pixel in row {
//!         *pixel = rng.random_range(1000..1200);
//!     }
//! }
//!
//! let original_packet = MQ00::new(2, 5, 0, 0, 452, 4356, pixel_data).unwrap();
//! let original_size = size_of::<MP00>();
//! let mut buffer = vec![];
//! let mut cursor = Cursor::new(&mut buffer);
//! original_packet.encode(&mut cursor).unwrap();
//!
//! let encoded_size = buffer.len();
//!
//! let mp00 = MP00::new(2, 5, 0, 452, 4356, pixel_data);
//! let mut mp00_buffer = vec![];
//! let mut cursor = Cursor::new(&mut mp00_buffer);
//! mp00.encode(&mut cursor).unwrap();
//! let mp00_size = mp00_buffer.len();
//!
//! let mut cursor = Cursor::new(buffer);
//! let decoded_packet = MQ00::decode(&mut cursor).unwrap();
//!
//! assert_eq!(original_packet, decoded_packet);
//!
//! println!("Original size: {original_size}");
//! println!("Encoded size: {encoded_size}");
//! println!(
//!     "Compression ratio: {:.2}",
//!     original_size as f32 / encoded_size as f32
//! );
//! println!(
//!     "{:.2}x better than MP00 with no quantization",
//!     mp00_size as f32 / encoded_size as f32
//! );
//! // Output:
//! // Original size: 2092
//! // Encoded size: 1160
//! // Compression ratio: 1.80
//! // 1.32x better than MP00 with no quantization
//! ```
use std::io::{Read, Seek, Write};

use crate::{
    DDM_HEIGHT, DDM_WIDTH, DDMPixelData,
    l0b_packet::{Decode, Encode, mp00::MP00},
};

use cabac::{CabacContext, CabacDecoder, CabacEncoder};

use anyhow::{Result, anyhow};
use bitstream_io::{BigEndian, BitRead, BitReader, BitWrite, BitWriter};
use ndarray::{Array2, ArrayView2, ArrayViewMut2, Axis};

pub mod cabac;
pub mod wavelet;

const AXIS_ORDER: [Axis; 2] = [Axis(0), Axis(1)];
const SUBBAND_ORDER: [SubBand; 4] = [SubBand::HH, SubBand::HL, SubBand::LH, SubBand::LL];
const HEADER_VERSION: u8 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SubBand {
    LL,
    LH,
    HL,
    HH,
}

impl SubBand {
    fn indices(self) -> (usize, usize) {
        match self {
            SubBand::LL => (0, 0),
            SubBand::LH => (0, 1),
            SubBand::HL => (1, 0),
            SubBand::HH => (1, 1),
        }
    }
}

/// Experimental MQ00 packet type
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MQ00 {
    channel_num: u8,
    status_flag: u8,
    ll_quantisation: u8,
    detail_quantisation: u8,
    timestamp_seconds: u32,
    timestamp_clocks: i32,
    pixel_data: Array2<i32>,
}

impl From<MP00> for MQ00 {
    fn from(mp00: MP00) -> Self {
        #[expect(clippy::unwrap_used, reason = "Size guaranteed to match by type")]
        let pixel_data = Array2::from_shape_vec(
            (DDM_HEIGHT, DDM_WIDTH),
            mp00.pixel_data()
                .iter()
                .flat_map(|x| x.iter())
                .map(|&x| i32::from(x))
                .collect(),
        )
        .unwrap();
        Self {
            channel_num: mp00.channel_num(),
            status_flag: mp00.status_flag(),
            ll_quantisation: 0,
            detail_quantisation: 0,
            timestamp_seconds: mp00.timestamp_seconds(),
            timestamp_clocks: mp00.timestamp_clocks(),
            pixel_data,
        }
    }
}

impl Decode for MQ00 {
    fn decode<R: Read + Seek>(reader: &mut R) -> Result<Self> {
        let start_pos = reader.stream_position()?;
        let mut header_buf = [0u8; 12];

        reader.read_exact(&mut header_buf)?;
        let header_version = header_buf[0];
        let channel_num = header_buf[1];
        let status_flag = header_buf[2];
        let pixel_quantisation = header_buf[3];
        #[expect(clippy::unwrap_used, reason = "Guaranteed to be 4 long by slice")]
        let timestamp_seconds = u32::from_be_bytes(header_buf[4..8].try_into().unwrap());
        #[expect(clippy::unwrap_used, reason = "Guaranteed to be 4 long by slice")]
        let timestamp_clocks = i32::from_be_bytes(header_buf[8..12].try_into().unwrap());

        if header_version != HEADER_VERSION {
            return Err(anyhow!(
                "Illegal MQ00 header version {header_version} found at byte {:x}\nOnly MQ00 header version {HEADER_VERSION} is supported",
                reader.stream_position()?
            ));
        }

        let detail_quantisation = pixel_quantisation >> 4;
        let ll_quantisation = pixel_quantisation & 0x0F;

        let pixel_data = Array2::from_elem([DDM_HEIGHT, DDM_WIDTH], 0);

        let mut ctx = CabacContext::new();
        let mut decoder = CabacDecoder::new(BitReader::<_, BigEndian>::new(&mut *reader))?;

        let mut output = Self {
            channel_num,
            status_flag,
            ll_quantisation,
            detail_quantisation,
            timestamp_seconds,
            timestamp_clocks,
            pixel_data,
        };

        for bit_idx in (output.detail_quantisation..20).rev() {
            for subband in &SUBBAND_ORDER[0..3] {
                output.decode_bit_plane(&mut decoder, &mut ctx, bit_idx.into(), *subband)?;
            }
        }
        for bit_idx in (output.ll_quantisation..20).rev() {
            output.decode_bit_plane(&mut decoder, &mut ctx, bit_idx.into(), SUBBAND_ORDER[3])?;
        }
        for subband in &SUBBAND_ORDER {
            output.decode_sign_plane(&mut decoder, &mut ctx, *subband)?;
        }

        output.idwt();

        let reader = decoder.into_inner().into_reader();
        let bytes_decoded = reader.stream_position()? - start_pos;
        if !bytes_decoded.is_multiple_of(4) {
            let target_distance = 4 - (bytes_decoded % 4);
            #[expect(
                clippy::unwrap_used,
                reason = "If the distance can't fit into an i64 we're buggered anyway"
            )]
            reader.seek_relative(target_distance.try_into().unwrap())?;
        }
        let mut crc = [0u8; 4];
        reader.read_exact(&mut crc)?;
        let _crc = u32::from_be_bytes(crc);

        Ok(output)
    }
}

impl Encode for MQ00 {
    fn encode<W: Write + Seek>(&self, writer: &mut W) -> Result<()> {
        let start_pos = writer.stream_position()?;
        let mut header_buf = [0u8; 12];

        header_buf[0] = HEADER_VERSION;
        header_buf[1] = self.channel_num;
        header_buf[2] = self.status_flag;
        header_buf[3] = self.detail_quantisation << 4 | self.ll_quantisation;
        header_buf[4..8].copy_from_slice(&self.timestamp_seconds.to_be_bytes());
        header_buf[8..12].copy_from_slice(&self.timestamp_clocks.to_be_bytes());

        writer.write_all(&header_buf)?;

        let mut copy = self.clone();

        copy.dwt();

        let mut ctx = CabacContext::new();
        let mut encoder = CabacEncoder::new(BitWriter::<_, BigEndian>::new(writer));

        for bit_idx in (self.detail_quantisation..20).rev() {
            for subband in &SUBBAND_ORDER[0..3] {
                copy.encode_bit_plane(&mut encoder, &mut ctx, bit_idx.into(), *subband)?;
            }
        }
        for bit_idx in (self.ll_quantisation..20).rev() {
            copy.encode_bit_plane(&mut encoder, &mut ctx, bit_idx.into(), SUBBAND_ORDER[3])?;
        }
        for subband in &SUBBAND_ORDER {
            copy.encode_sign_plane(&mut encoder, &mut ctx, *subband)?;
        }
        encoder.encode_flush()?;
        let mut writer = encoder.into_inner();
        writer.byte_align()?;
        let writer = writer.into_writer();

        let current_pos = writer.stream_position()?;
        let bytes_encoded = current_pos - start_pos;

        if !bytes_encoded.is_multiple_of(4) {
            let target_distance = 4 - (bytes_encoded % 4);
            #[expect(
                clippy::cast_possible_truncation,
                reason = "Only possible if packet is longer than address space"
            )]
            writer.write_all(&vec![0u8; target_distance as usize])?;
        }
        let crc = 0u32; // TODO write actual CRC code
        writer.write_all(&crc.to_be_bytes())?;
        Ok(())
    }
}

impl MQ00 {
    /// Create new MQ00 packet from raw attributes
    ///
    /// `ll_quantisation` and `detail_quantisation` must be representable in 4 bits each
    pub fn new(
        channel_num: u8,
        status_flag: u8,
        ll_quantisation: u8,
        detail_quantisation: u8,
        timestamp_seconds: u32,
        timestamp_clocks: i32,
        pixel_data: DDMPixelData,
    ) -> Result<Self> {
        if ll_quantisation > 0xF {
            return Err(anyhow!(
                "Illegal ll_quantisation {ll_quantisation}; low frequency quantisation must be representable in 4 bits"
            ));
        }
        if detail_quantisation > 0xF {
            return Err(anyhow!(
                "Illegal detail_quantisation {detail_quantisation}; high frequency quantisation must be representable in 4 bits"
            ));
        }
        let shape = (pixel_data.len(), pixel_data[0].len());

        let pixel_data = ArrayView2::from_shape(shape, pixel_data.as_flattened())?.mapv(i32::from);
        Ok(Self {
            channel_num,
            status_flag,
            ll_quantisation,
            detail_quantisation,
            timestamp_seconds,
            timestamp_clocks,
            pixel_data,
        })
    }

    /// DDM hardware channel number
    pub fn channel_num(&self) -> u8 {
        self.channel_num
    }

    /// Packet status flags.
    pub fn status_flag(&self) -> u8 {
        self.status_flag
    }

    /// Quantisation applied to low-frequency wavelet coefficients.
    pub fn ll_quantisation(&self) -> u8 {
        self.ll_quantisation
    }

    /// Quantisation applied to low-frequency wavelet coefficients.
    pub fn set_ll_quantisation(&mut self, value: u8) -> Result<()> {
        if value > 0xF {
            return Err(anyhow!(
                "Illegal ll_quantisation {value}; low frequency quantisation must be representable in 4 bits"
            ));
        }
        self.ll_quantisation = value;
        Ok(())
    }

    /// Quantisation applied to detail wavelet coefficients.
    pub fn detail_quantisation(&self) -> u8 {
        self.detail_quantisation
    }

    /// Quantisation applied to detail wavelet coefficients.
    pub fn set_detail_quantisation(&mut self, value: u8) -> Result<()> {
        if value > 0xF {
            return Err(anyhow!(
                "Illegal detail_quantisation {value}; high frequency quantisation must be representable in 4 bits"
            ));
        }
        self.detail_quantisation = value;
        Ok(())
    }

    /// Time stamp of integration start: GPS seconds at last PPS as rounded integer
    pub fn timestamp_seconds(&self) -> u32 {
        self.timestamp_seconds
    }

    /// Time stamp of integration start: sample clocks since last PPS
    pub fn timestamp_clocks(&self) -> i32 {
        self.timestamp_clocks
    }

    /// DDM pixel data
    pub fn pixel_data(&self) -> Result<DDMPixelData> {
        let mut output: DDMPixelData = [[0; _]; _];
        for (new_row, old_row) in output.iter_mut().zip(self.pixel_data.axis_iter(Axis(0))) {
            for (new_pixel, old_pixel) in new_row.iter_mut().zip(old_row.iter()) {
                *new_pixel = u16::try_from(*old_pixel)?;
            }
        }
        Ok(output)
    }

    /// DDM pixel data
    pub fn raw_pixel_data(&self) -> ArrayView2<'_, i32> {
        self.pixel_data.view()
    }

    /// Apply 2D wavelet transform
    pub fn dwt(&mut self) {
        wavelet::dwt_53_nd(self.pixel_data.view_mut(), &AXIS_ORDER);
    }

    /// Apply inverse 2D wavelet transform
    pub fn idwt(&mut self) {
        wavelet::idwt_53_nd(self.pixel_data.view_mut(), &AXIS_ORDER);
    }

    fn encode_bit_plane<W: BitWrite>(
        &self,
        encoder: &mut CabacEncoder<W>,
        ctx: &mut CabacContext,
        bit_idx: usize,
        subband: SubBand,
    ) -> Result<()> {
        for pixel in self.subband(subband) {
            encoder.encode_decision(pixel.abs() >> bit_idx & 1 == 1, ctx)?;
        }
        Ok(())
    }

    fn encode_sign_plane<W: BitWrite>(
        &self,
        encoder: &mut CabacEncoder<W>,
        ctx: &mut CabacContext,
        subband: SubBand,
    ) -> Result<()> {
        for pixel in self.subband(subband) {
            encoder.encode_decision(*pixel < 0, ctx)?;
        }
        Ok(())
    }

    fn decode_bit_plane<R: BitRead>(
        &mut self,
        decoder: &mut CabacDecoder<R>,
        ctx: &mut CabacContext,
        bit_idx: usize,
        subband: SubBand,
    ) -> Result<()> {
        for pixel in self.subband_mut(subband) {
            if decoder.decode_decision(ctx)? {
                *pixel |= 1 << bit_idx;
            }
        }
        Ok(())
    }

    fn decode_sign_plane<R: BitRead>(
        &mut self,
        decoder: &mut CabacDecoder<R>,
        ctx: &mut CabacContext,
        subband: SubBand,
    ) -> Result<()> {
        for pixel in self.subband_mut(subband) {
            if decoder.decode_decision(ctx)? {
                *pixel = -*pixel;
            }
        }
        Ok(())
    }

    fn subband(&self, subband: SubBand) -> ArrayView2<'_, i32> {
        let indices = subband.indices();
        let h = DDM_HEIGHT / 2;
        let w = DDM_WIDTH / 2;

        self.pixel_data.slice(ndarray::s![
            indices.0 * h..(indices.0 + 1) * h,
            indices.1 * w..(indices.1 + 1) * w,
        ])
    }

    fn subband_mut(&mut self, subband: SubBand) -> ArrayViewMut2<'_, i32> {
        let indices = subband.indices();
        let h = DDM_HEIGHT / 2;
        let w = DDM_WIDTH / 2;

        self.pixel_data.slice_mut(ndarray::s![
            indices.0 * h..(indices.0 + 1) * h,
            indices.1 * w..(indices.1 + 1) * w,
        ])
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::tests::arb_pixel_data;

    use proptest::prelude::*;

    // Custom strategy to generate arbitrary valid MQ00 instances
    pub fn arb_mq00(
        pixel_data_strategy: impl Strategy<Value = u16> + Clone,
    ) -> impl Strategy<Value = MQ00> {
        (
            any::<u8>(),
            any::<u8>(),
            0..1u8,
            0..1u8,
            any::<u32>(),
            any::<i32>(),
            arb_pixel_data(pixel_data_strategy),
        )
            .prop_map(
                |(
                    channel_num,
                    status_flag,
                    ll_quantisation,
                    detail_quantisation,
                    timestamp_seconds,
                    timestamp_clocks,
                    pixel_data,
                )| {
                    MQ00::new(
                        channel_num,
                        status_flag,
                        ll_quantisation,
                        detail_quantisation,
                        timestamp_seconds,
                        timestamp_clocks,
                        pixel_data,
                    )
                    .unwrap()
                },
            )
    }

    proptest! {
        #[test]
        fn pixel_conversion(original in arb_pixel_data(any::<u16>())) {
            // Encode the packet into an in-memory buffer
            let packet = MQ00::new(0, 0, 0, 0, 0, 0, original).unwrap();
            // Ensure round-trip integrity holds
            prop_assert_eq!(original, packet.pixel_data().unwrap());
        }

        #[test]
        fn quantisation_limit(ll_quantisation in 16u8.., detail_quantisation in 16u8..) {
            let packet = MQ00::new(0, 0, ll_quantisation, detail_quantisation, 0, 0, [[0; 20]; 52]);

            prop_assert!(packet.is_err());
        }

        #[test]
        fn dwt_roundtrip(original in arb_mq00(any::<u16>())) {
            let mut transformed = original.clone();
            transformed.dwt();
            transformed.idwt();
            prop_assert_eq!(original, transformed);
        }

        #[test]
        fn mq00_roundtrip(original in arb_mq00(any::<u16>())) {
            // Encode the packet into an in-memory buffer
            let mut buffer = vec![];
            let mut cursor = Cursor::new(&mut buffer);
            original.encode(&mut cursor).unwrap();

            // Decode the packet back from the buffer
            let mut cursor = Cursor::new(buffer);
            let decoded = MQ00::decode(&mut cursor).unwrap();
            prop_assert_eq!(original, decoded);
        }
    }
}
