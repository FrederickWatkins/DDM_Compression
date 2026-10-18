//! Module for CABAC (Context Adaptive Binary Arithmetic Compression) based on H264 standard
//!
//! Advanced features like bypass and termination are not supported
//!
//! # Examples
//! ```
//! use bitstream_io::{BigEndian, BitRead, BitReader, BitWrite, BitWriter};
//! use ddm_compression::l0b_packet::mq00::cabac::{CabacContext, CabacDecoder, CabacEncoder};
//! use std::io::Cursor;
//!
//! let test_vec: Vec<u64> = vec![1, 89, 122, 63, 42];
//! let original_size = test_vec.len() * size_of::<u64>();
//!
//! let writer = BitWriter::endian(Cursor::new(vec![]), BigEndian);
//! let mut ctx = CabacContext::new();
//! let mut encoder = CabacEncoder::new(writer);
//! // Encode each bit of every u64 (from MSB to LSB)
//! for &x in &test_vec {
//!     for bit_idx in 0..64 {
//!         let bit = (x & (1 << (63 - bit_idx))) != 0;
//!         encoder.encode_decision(bit, &mut ctx).unwrap();
//!     }
//! }
//!
//! encoder.encode_flush().unwrap();
//! let mut writer = encoder.into_inner();
//! // Flush any trailing bits
//! writer.byte_align().unwrap();
//!
//! let encoded_data = writer.into_writer().into_inner();
//! let encoded_size = encoded_data.len() * size_of::<u8>();
//!
//! // Decode back into a Vec<u8> by grouping every 64 decoded bits
//! let reader = BitReader::endian(Cursor::new(encoded_data), BigEndian);
//! let mut ctx = CabacContext::new();
//! let mut decoder = CabacDecoder::new(reader).unwrap();
//! let mut decoded_data = vec![0u64; test_vec.len()];
//! for x in decoded_data.iter_mut() {
//!     for bit_idx in 0..64 {
//!         let bit = decoder.decode_decision(&mut ctx).unwrap();
//!         if bit {
//!             *x |= 1 << (63 - bit_idx);
//!         }
//!     }
//! }
//! assert_eq!(test_vec, decoded_data);
//!
//! println!("Original size: {original_size}");
//! println!("Encoded size: {encoded_size}");
//! println!(
//!     "Compression ratio: {:.2}",
//!     original_size as f32 / encoded_size as f32
//! );
//! // Output:
//! // Original size: 72
//! // Encoded size: 26
//! // Compression ratio: 2.77
//! ```
use anyhow::Result;
use bitstream_io::{BitRead, BitWrite};

/// LPS ranges depending on context/encoder state
const RANGE_TAB_LPS: [[u8; 4]; 64] = [
    [128, 176, 208, 240],
    [128, 167, 197, 227],
    [128, 158, 187, 216],
    [123, 150, 178, 205],
    [116, 142, 169, 195],
    [111, 135, 160, 185],
    [105, 128, 152, 175],
    [100, 122, 144, 166],
    [95, 116, 137, 158],
    [90, 110, 130, 150],
    [85, 104, 123, 142],
    [81, 99, 117, 135],
    [77, 94, 111, 128],
    [73, 89, 105, 122],
    [69, 85, 100, 116],
    [66, 80, 95, 110],
    [62, 76, 90, 104],
    [59, 72, 86, 99],
    [56, 69, 81, 94],
    [53, 65, 77, 89],
    [51, 62, 73, 85],
    [48, 59, 69, 80],
    [46, 56, 66, 76],
    [43, 53, 63, 72],
    [41, 50, 59, 69],
    [39, 48, 56, 65],
    [37, 45, 54, 62],
    [35, 43, 51, 59],
    [33, 41, 48, 56],
    [32, 39, 46, 53],
    [30, 37, 43, 50],
    [29, 35, 41, 48],
    [27, 33, 39, 45],
    [26, 31, 37, 43],
    [24, 30, 35, 41],
    [23, 28, 33, 39],
    [22, 27, 32, 37],
    [21, 26, 30, 35],
    [20, 24, 29, 33],
    [19, 23, 27, 31],
    [18, 22, 26, 30],
    [17, 21, 25, 28],
    [16, 20, 23, 27],
    [15, 19, 22, 25],
    [14, 18, 21, 24],
    [14, 17, 20, 23],
    [13, 16, 19, 22],
    [12, 15, 18, 21],
    [12, 14, 17, 20],
    [11, 14, 16, 19],
    [11, 13, 15, 18],
    [10, 12, 15, 17],
    [10, 12, 14, 16],
    [9, 11, 13, 15],
    [9, 11, 12, 14],
    [8, 10, 12, 14],
    [8, 9, 11, 13],
    [7, 9, 11, 12],
    [7, 9, 10, 12],
    [7, 8, 10, 11],
    [6, 8, 9, 11],
    [6, 7, 9, 10],
    [6, 7, 8, 9],
    [2, 2, 2, 2],
];

/// State transition upon encountering MPS
const TRANS_IDX_MPS: [u8; 64] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26,
    27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50,
    51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 62, 63,
];

/// State transition upon encountering LPS
const TRANS_IDX_LPS: [u8; 64] = [
    0, 0, 1, 2, 2, 4, 4, 5, 6, 7, 8, 9, 9, 11, 11, 12, 13, 13, 15, 15, 16, 16, 18, 18, 19, 19, 21,
    21, 22, 22, 23, 24, 24, 25, 26, 26, 27, 27, 28, 29, 29, 30, 30, 30, 31, 32, 32, 33, 33, 33, 34,
    34, 35, 35, 35, 36, 36, 36, 37, 37, 37, 38, 38, 63,
];

/// Context for CABAC encoder/decoder
///
/// A single CABAC encoder/decoder can use multiple contexts safely during the same bitstream as
/// long as the contexts are used in the same way during the reverse operation
pub struct CabacContext {
    p_state_idx: u8,
    mps: bool,
}

impl Default for CabacContext {
    fn default() -> Self {
        Self::new()
    }
}

impl CabacContext {
    /// Create a new context with default init values
    pub fn new() -> Self {
        Self {
            p_state_idx: 0,
            mps: true,
        }
    }
}

/// CABAC Encoder
///
/// Implementation based directly on H264 standard and unoptimized
pub struct CabacEncoder<W> {
    writer: W,
    cod_i_low: u16,
    cod_i_range: u16,
    first_bit_flag: bool,
    bits_outstanding: u8,
}

impl<W> CabacEncoder<W> {
    /// Consumes the encoder and returns the inner writer
    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: BitWrite> CabacEncoder<W> {
    /// Create a new CABAC encoder, passing it a writer
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            cod_i_low: 0,
            cod_i_range: 510,
            first_bit_flag: true,
            bits_outstanding: 0,
        }
    }

    /// Encode a single bit
    pub fn encode_decision(&mut self, bin_val: bool, ctx: &mut CabacContext) -> Result<()> {
        let q_cod_i_range_idx = (self.cod_i_range >> 6) & 3;
        let cod_i_range_lps =
            u16::from(RANGE_TAB_LPS[ctx.p_state_idx as usize][q_cod_i_range_idx as usize]);
        self.cod_i_range -= cod_i_range_lps;
        if bin_val == ctx.mps {
            ctx.p_state_idx = TRANS_IDX_MPS[ctx.p_state_idx as usize];
        } else {
            self.cod_i_low += self.cod_i_range;
            self.cod_i_range = cod_i_range_lps;
            if ctx.p_state_idx == 0 {
                ctx.mps = !ctx.mps;
            }
            ctx.p_state_idx = TRANS_IDX_LPS[ctx.p_state_idx as usize];
        }
        self.renorm_e()?;
        Ok(())
    }

    /// Flush all outstanding bits to the buffer
    ///
    /// This function must be called after all bits have been encoded, otherwise the encoded buffer
    /// will be incomplete
    pub fn encode_flush(&mut self) -> Result<()> {
        self.cod_i_range = 2;
        self.renorm_e()?;
        self.put_bit((self.cod_i_low >> 9) & 1 == 1)?;
        self.writer
            .write::<2, u16>(((self.cod_i_low >> 7) & 3) | 1)?;
        Ok(())
    }

    /// Renormalize the range and write resolved bits
    fn renorm_e(&mut self) -> Result<()> {
        while self.cod_i_range < 256 {
            if self.cod_i_low < 256 {
                self.put_bit(false)?;
            } else if self.cod_i_low >= 512 {
                self.cod_i_low -= 512;
                self.put_bit(true)?;
            } else {
                self.cod_i_low -= 256;
                self.bits_outstanding += 1;
            }
            self.cod_i_range <<= 1;
            self.cod_i_low <<= 1;
        }
        Ok(())
    }

    /// Write resolved bits
    fn put_bit(&mut self, bit: bool) -> Result<()> {
        if self.first_bit_flag {
            self.first_bit_flag = false;
        } else {
            self.writer.write_bit(bit)?;
        }
        while self.bits_outstanding > 0 {
            self.writer.write_bit(!bit)?;
            self.bits_outstanding -= 1;
        }
        Ok(())
    }
}

/// CABAC Decoder
///
/// Implementation based directly on H264 standard and unoptimized
pub struct CabacDecoder<R> {
    reader: R,
    cod_i_range: u16,
    cod_i_offset: u16,
}

impl<R> CabacDecoder<R> {
    /// Consumes the decoder and returns the inner reader
    pub fn into_inner(self) -> R {
        self.reader
    }
}

impl<R: BitRead> CabacDecoder<R> {
    /// Create a new CABAC decoder, passing it a reader
    pub fn new(mut reader: R) -> Result<Self> {
        let cod_i_range = 510;
        let cod_i_offset = reader.read::<9, u16>()?;
        Ok(Self {
            reader,
            cod_i_range,
            cod_i_offset,
        })
    }

    /// Decode a single bit
    pub fn decode_decision(&mut self, ctx: &mut CabacContext) -> Result<bool> {
        let q_cod_i_range_idx = (self.cod_i_range >> 6) & 3;
        let cod_i_range_lps = RANGE_TAB_LPS[ctx.p_state_idx as usize][q_cod_i_range_idx as usize];
        let bin_val;
        self.cod_i_range -= u16::from(cod_i_range_lps);
        if self.cod_i_offset < self.cod_i_range {
            bin_val = ctx.mps;
            ctx.p_state_idx = TRANS_IDX_MPS[ctx.p_state_idx as usize];
        } else {
            bin_val = !ctx.mps;
            self.cod_i_offset -= self.cod_i_range;
            self.cod_i_range = u16::from(cod_i_range_lps);
            if ctx.p_state_idx == 0 {
                ctx.mps = !ctx.mps;
            }
            ctx.p_state_idx = TRANS_IDX_LPS[ctx.p_state_idx as usize];
        }
        self.renorm_d()?;
        Ok(bin_val)
    }

    /// Renormalize the range and slide in bits from input
    fn renorm_d(&mut self) -> Result<()> {
        while self.cod_i_range < 256 {
            self.cod_i_range <<= 1;
            self.cod_i_offset <<= 1;
            self.cod_i_offset |= u16::from(self.reader.read_bit()?);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitstream_io::{BigEndian, BitReader, BitWriter};
    use proptest::prelude::*;
    use std::io::Cursor;

    proptest! {
        #[test]
        fn cabac_roundtrip(data in prop::collection::vec(any::<u64>(), 1..1000)) {
            let writer = BitWriter::endian(Cursor::new(vec![]), BigEndian);
            let mut ctx = CabacContext::new();
            let mut encoder = CabacEncoder::new(writer);

            // Encode each bit of every u64 (from MSB to LSB)
            for &x in &data {
                for bit_idx in 0..64 {
                    let bit = (x & (1 << (63 - bit_idx))) != 0;
                    encoder.encode_decision(bit, &mut ctx).unwrap();
                }
            }

            encoder.encode_flush().unwrap();
            let mut writer = encoder.into_inner();
            writer.byte_align().unwrap();

            let encoded_data = writer.into_writer().into_inner();

            // Decode back into a Vec<u64> by grouping every 64 decoded bits
            let reader = BitReader::endian(Cursor::new(encoded_data), BigEndian);
            let mut ctx = CabacContext::new();
            let mut decoder = CabacDecoder::new(reader).unwrap();

            let mut decoded_data = vec![0u64; data.len()];
            for x in &mut decoded_data {
                for bit_idx in 0..64 {
                    let bit = decoder.decode_decision(&mut ctx).unwrap();
                    if bit {
                        *x |= 1 << (63 - bit_idx);
                    }
                }
            }

            assert_eq!(data, decoded_data);
        }
    }
}
