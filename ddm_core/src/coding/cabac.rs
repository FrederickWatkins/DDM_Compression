//! Module for CABAC (Context Adaptive Binary Arithmetic Compression) based on H264 standard
//!
//! There are two possible modes of operation:
//! - The encoder and decoder can hold a contract for a fixed termination point, either because
//!   packet length is fixed, or by encoding the length in a header. In this mode,
//!   [`CabacEncoder::encode_terminate_continue`]/[`CabacEncoder::encode_terminate_finish`] and
//!   [`CabacDecoder::decode_terminate`] aren't necessary, simply encoding a flush is sufficient.
//! - The encoder and decoder can hold a contract for certain optional termination points - for
//!   example, after each bitplane of an image. In this case,
//!   [`CabacEncoder::encode_terminate_continue`]/[`CabacEncoder::encode_terminate_finish`] and
//!   [`CabacDecoder::decode_terminate`] must be used at each termination point to determine whether
//!   to continue. Note that there must be a contract between the encoder and decoder in either
//!   case, CABAC doesn't support terminating arbitrary length bitstreams.
//!
//! The cost of termination flags is non-zero but small. A good rule of thumb is that it costs
//! roughly 0.01 bits in the encoded stream, so they shouldn't be interlaced between every bit, but
//! there's no reason not to have them fairly frequently.
//!
//! # Examples
//! ```
//! use std::io::Cursor;
//!
//! use bitstream_io::{BigEndian, BitRead, BitReader, BitWrite, BitWriter};
//! use ddm_core::coding::cabac::{CabacContext, CabacDecoder, CabacEncoder};
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
#![allow(
    clippy::indexing_slicing,
    reason = "Indexing is only done into internal state machine"
)]

use bitstream_io::{BitRead, BitWrite};
use snafu::{Backtrace, Snafu, ensure};

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

/// Error kind for CABAC processes
#[derive(Debug, Snafu)]
pub enum Error {
    /// Error in the underlying stream
    #[snafu(transparent)]
    Io {
        /// Error source
        source: std::io::Error,
        /// Error backtrace
        backtrace: Backtrace,
    },
    /// Tried to decode bits after decoder returned terminate
    #[snafu(display("Attempted to decode value after terminate flag received"))]
    DecodeAfterTerminate {
        /// Error backtrace
        backtrace: Backtrace,
    },
}

type Result<T> = std::result::Result<T, Error>;

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
    bits_outstanding: usize,
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
    ///
    /// # Errors
    /// Passes along any I/O error from the underlying stream.
    pub fn encode_decision(&mut self, bin_val: bool, ctx: &mut CabacContext) -> Result<()> {
        let q_cod_i_range_idx = (self.cod_i_range >> 6) & 3;
        let cod_i_range_lps =
            u16::from(RANGE_TAB_LPS[usize::from(ctx.p_state_idx)][usize::from(q_cod_i_range_idx)]);
        self.cod_i_range -= cod_i_range_lps;
        if bin_val == ctx.mps {
            ctx.p_state_idx = TRANS_IDX_MPS[usize::from(ctx.p_state_idx)];
        } else {
            self.cod_i_low += self.cod_i_range;
            self.cod_i_range = cod_i_range_lps;
            if ctx.p_state_idx == 0 {
                ctx.mps = !ctx.mps;
            }
            ctx.p_state_idx = TRANS_IDX_LPS[usize::from(ctx.p_state_idx)];
        }
        self.renorm_e()?;
        Ok(())
    }

    /// Encode bits directly to the output stream
    ///
    /// This function should be used for high entropy data to avoid incurring the overhead of
    /// context adaptive encoding where there's nothing to gain from it.
    ///
    /// # Errors
    /// Passes on errors from the underlying writer
    pub fn encode_bypass(&mut self, bin_val: bool) -> Result<()> {
        self.cod_i_low <<= 1;
        if bin_val {
            self.cod_i_low += self.cod_i_range;
        }
        if self.cod_i_low < 1024 {
            if self.cod_i_low < 512 {
                self.put_bit(false)?;
            } else {
                self.cod_i_low -= 512;
                self.bits_outstanding += 1;
            }
        } else {
            self.put_bit(true)?;
            self.cod_i_low -= 1024;
        }
        Ok(())
    }

    /// Flush all outstanding bits to the buffer
    ///
    /// This function must be called after all bits have been encoded, otherwise the encoded buffer
    /// will be incomplete
    ///
    /// # Errors
    /// Passes along any I/O error from the underlying stream.
    pub fn encode_flush(&mut self) -> Result<()> {
        self.cod_i_range = 2;
        self.renorm_e()?;
        self.put_bit((self.cod_i_low >> 9) & 1 == 1)?;
        self.writer
            .write::<2, u16>(((self.cod_i_low >> 7) & 3) | 1)?;
        Ok(())
    }

    /// Encode a flag to continue the stream at a preagreed possible termination point
    ///
    /// # Errors
    /// Passes on errors from the underlying writer
    pub fn encode_terminate_continue(&mut self) -> Result<()> {
        self.cod_i_range -= 2;
        self.renorm_e()
    }

    /// Encode a flag to finish the stream at a preagreed possible termination point
    ///
    /// # Errors
    /// Passes on errors from the underlying writer
    pub fn encode_terminate_finish(mut self) -> Result<W> {
        self.cod_i_range -= 2;
        self.cod_i_low += self.cod_i_range;
        self.encode_flush()?;
        Ok(self.writer)
    }

    /// Renormalize the range and write resolved bits
    ///
    /// # Errors
    /// Passes along any I/O error from the underlying stream.
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
    ///
    /// # Errors
    /// Passes along any I/O error from the underlying stream.
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
    finished: bool,
}

impl<R> CabacDecoder<R> {
    /// Consumes the decoder and returns the inner reader
    pub fn into_inner(self) -> R {
        self.reader
    }
}

impl<R: BitRead> CabacDecoder<R> {
    /// Create a new CABAC decoder, passing it a reader
    ///
    /// # Errors
    /// Passes along any I/O error from the underlying stream.
    pub fn new(mut reader: R) -> Result<Self> {
        let cod_i_range = 510;
        let cod_i_offset = reader.read::<9, u16>()?;
        let finished = false;
        Ok(Self {
            reader,
            cod_i_range,
            cod_i_offset,
            finished,
        })
    }

    /// Decode a single bit
    ///
    /// # Errors
    /// Passes along any I/O error from the underlying stream. Will error if decoder has already
    /// read a termination flag.
    pub fn decode_decision(&mut self, ctx: &mut CabacContext) -> Result<bool> {
        ensure!(!self.finished, DecodeAfterTerminateSnafu);
        let q_cod_i_range_idx = (self.cod_i_range >> 6) & 3;
        let cod_i_range_lps =
            RANGE_TAB_LPS[usize::from(ctx.p_state_idx)][usize::from(q_cod_i_range_idx)];
        let bin_val;
        self.cod_i_range -= u16::from(cod_i_range_lps);
        if self.cod_i_offset < self.cod_i_range {
            bin_val = ctx.mps;
            ctx.p_state_idx = TRANS_IDX_MPS[usize::from(ctx.p_state_idx)];
        } else {
            bin_val = !ctx.mps;
            self.cod_i_offset -= self.cod_i_range;
            self.cod_i_range = u16::from(cod_i_range_lps);
            if ctx.p_state_idx == 0 {
                ctx.mps = !ctx.mps;
            }
            ctx.p_state_idx = TRANS_IDX_LPS[usize::from(ctx.p_state_idx)];
        }
        self.renorm_d()?;
        Ok(bin_val)
    }

    /// Decode a single bypass bin
    ///
    /// Note that the encoder and decoder must have a preagreed contract on when bypass bits are
    /// used or they will become desynchronised.
    ///
    /// # Errors
    /// Passes on errors from the underlying stream. Will error if decoder has already read a
    /// termination flag.
    pub fn decode_bypass(&mut self) -> Result<bool> {
        ensure!(!self.finished, DecodeAfterTerminateSnafu);
        self.cod_i_offset <<= 1;
        if self.reader.read_bit()? {
            self.cod_i_offset |= 1;
        }
        if self.cod_i_offset >= self.cod_i_range {
            self.cod_i_offset -= self.cod_i_range;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Decode a termination flag
    ///
    /// Note that the encoder and decoder must have a preagreed contract on when termination flags
    /// can be encoded or they will become desynchronised. Also note that continuing to use a
    /// decoder after it returns a 1 from this function will result in an error.
    ///
    /// # Errors
    /// Passes on errors from the underlying stream. Will error if decoder has already read a
    /// termination flag.
    pub fn decode_terminate(&mut self) -> Result<bool> {
        ensure!(!self.finished, DecodeAfterTerminateSnafu);
        self.cod_i_range -= 2;
        if self.cod_i_offset >= self.cod_i_range {
            self.finished = true;
            Ok(true)
        } else {
            self.renorm_d()?;
            Ok(false)
        }
    }

    /// Renormalize the range and slide in bits from input
    ///
    /// # Errors
    /// Passes along any I/O error from the underlying stream.
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
    use std::io::Cursor;

    use bitstream_io::{BigEndian, BitReader, BitWriter};
    use proptest::prelude::*;

    use super::*;

    const NUM_CTX: usize = 3;
    /// Percentage chance of a 1 bin per context, so the contexts see skewed statistics
    const ONE_PERCENT: [u32; NUM_CTX] = [5, 50, 95];
    const SENTINEL: u16 = 0xA5C3;

    #[derive(Debug, Clone, Copy)]
    enum Op {
        Decision { ctx: usize, bin: bool },
        Bypass(bool),
    }

    type TestWriter = BitWriter<Cursor<Vec<u8>>, BigEndian>;
    type TestReader = BitReader<Cursor<Vec<u8>>, BigEndian>;

    fn new_writer() -> TestWriter {
        BitWriter::endian(Cursor::new(Vec::new()), BigEndian)
    }

    fn new_reader(data: Vec<u8>) -> TestReader {
        BitReader::endian(Cursor::new(data), BigEndian)
    }

    fn new_ctxs() -> Vec<CabacContext> {
        (0..NUM_CTX).map(|_| CabacContext::new()).collect()
    }

    fn bins(ops: &[Op]) -> Vec<bool> {
        ops.iter()
            .map(|op| match *op {
                Op::Decision { bin, .. } | Op::Bypass(bin) => bin,
            })
            .collect()
    }

    fn encode_ops<W: BitWrite>(
        enc: &mut CabacEncoder<W>,
        ctxs: &mut [CabacContext],
        ops: &[Op],
    ) -> Result<()> {
        for &op in ops {
            match op {
                Op::Decision { ctx, bin } => enc.encode_decision(bin, &mut ctxs[ctx])?,
                Op::Bypass(bin) => enc.encode_bypass(bin)?,
            }
        }
        Ok(())
    }

    /// The decoder replays the same schedule of operations the encoder used
    fn decode_ops<R: BitRead>(
        dec: &mut CabacDecoder<R>,
        ctxs: &mut [CabacContext],
        ops: &[Op],
    ) -> Result<Vec<bool>> {
        ops.iter()
            .map(|&op| match op {
                Op::Decision { ctx, .. } => dec.decode_decision(&mut ctxs[ctx]),
                Op::Bypass(_) => dec.decode_bypass(),
            })
            .collect()
    }

    fn op() -> impl Strategy<Value = Op> {
        prop_oneof![
            3 => (0..NUM_CTX, 0u32..100).prop_map(|(ctx, roll)| Op::Decision {
                ctx,
                bin: roll < ONE_PERCENT[ctx],
            }),
            1 => any::<bool>().prop_map(Op::Bypass),
        ]
    }

    /// Long runs of an identical op, to exercise the outstanding-bits path and state saturation
    fn run_ops() -> impl Strategy<Value = Vec<Op>> {
        prop::collection::vec((op(), 1usize..300), 1..20)
            .prop_map(|runs| runs.into_iter().flat_map(|(op, n)| vec![op; n]).collect())
    }

    /// Chunks of ops separated by termination boundaries, plus the boundary index to stop at
    fn chunks_with_stop() -> impl Strategy<Value = (Vec<Vec<Op>>, usize)> {
        prop::collection::vec(prop::collection::vec(op(), 0..12), 0..40).prop_flat_map(|chunks| {
            let n = chunks.len();
            (Just(chunks), 0..=n)
        })
    }

    proptest! {
        /// Bypass and decision bins mixed, flush-only termination, with an exact end-position check
        #[test]
        fn flush_only_mixed_roundtrip(ops in run_ops()) {
            let mut enc = CabacEncoder::new(new_writer());
            let mut ctxs = new_ctxs();
            encode_ops(&mut enc, &mut ctxs, &ops)?;
            enc.encode_flush()?;
            let mut writer = enc.into_inner();
            writer.write::<16, u16>(SENTINEL)?;
            writer.byte_align()?;
            let data = writer.into_writer().into_inner();

            let mut dec = CabacDecoder::new(new_reader(data))?;
            let mut ctxs = new_ctxs();
            let got = decode_ops(&mut dec, &mut ctxs, &ops)?;
            prop_assert_eq!(got, bins(&ops));
            prop_assert_eq!(dec.into_inner().read::<16, u16>()?, SENTINEL);
        }

        /// Same, but ending with terminate(1) instead of a bare flush
        #[test]
        fn terminate_finish_mixed_roundtrip(ops in run_ops()) {
            let mut enc = CabacEncoder::new(new_writer());
            let mut ctxs = new_ctxs();
            encode_ops(&mut enc, &mut ctxs, &ops)?;
            let mut writer = enc.encode_terminate_finish()?;
            writer.write::<16, u16>(SENTINEL)?;
            writer.byte_align()?;
            let data = writer.into_writer().into_inner();

            let mut dec = CabacDecoder::new(new_reader(data))?;
            let mut ctxs = new_ctxs();
            let got = decode_ops(&mut dec, &mut ctxs, &ops)?;
            prop_assert_eq!(got, bins(&ops));
            prop_assert!(dec.decode_terminate()?);
            prop_assert_eq!(dec.into_inner().read::<16, u16>()?, SENTINEL);
        }

        /// A terminate boundary before every chunk. The encoder chooses which boundary is the last.
        #[test]
        fn terminate_schedule_roundtrip((chunks, stop_at) in chunks_with_stop()) {
            let mut enc = CabacEncoder::new(new_writer());
            let mut ctxs = new_ctxs();
            for chunk in chunks.iter().take(stop_at) {
                enc.encode_terminate_continue()?;
                encode_ops(&mut enc, &mut ctxs, chunk)?;
            }
            let mut writer = enc.encode_terminate_finish()?;
            writer.write::<16, u16>(SENTINEL)?;
            writer.byte_align()?;
            let data = writer.into_writer().into_inner();

            let mut dec = CabacDecoder::new(new_reader(data))?;
            let mut ctxs = new_ctxs();
            let mut decoded_chunks = 0;
            for chunk in &chunks {
                if dec.decode_terminate()? {
                    break;
                }
                let got = decode_ops(&mut dec, &mut ctxs, chunk)?;
                prop_assert_eq!(got, bins(chunk));
                decoded_chunks += 1;
            }
            if decoded_chunks == chunks.len() {
                // The boundary after the final chunk must also be present
                prop_assert!(dec.decode_terminate()?);
            }
            prop_assert_eq!(decoded_chunks, stop_at);
            prop_assert_eq!(dec.into_inner().read::<16, u16>()?, SENTINEL);
        }

        /// Several independent segments in one stream, each starting on a byte boundary
        #[test]
        fn multi_segment_roundtrip(
            segments in prop::collection::vec(prop::collection::vec(op(), 0..100), 1..6)
        ) {
            let mut writer = new_writer();
            for seg in &segments {
                let mut enc = CabacEncoder::new(writer);
                let mut ctxs = new_ctxs();
                encode_ops(&mut enc, &mut ctxs, seg)?;
                writer = enc.encode_terminate_finish()?;
                writer.byte_align()?;
            }
            let data = writer.into_writer().into_inner();

            let mut reader = new_reader(data);
            for seg in &segments {
                let mut dec = CabacDecoder::new(reader)?;
                let mut ctxs = new_ctxs();
                let got = decode_ops(&mut dec, &mut ctxs, seg)?;
                prop_assert_eq!(got, bins(seg));
                prop_assert!(dec.decode_terminate()?);
                reader = dec.into_inner();
                reader.byte_align();
            }
        }
    }

    #[test]
    fn terminate_continue_is_cheap() -> Result<()> {
        let mut enc = CabacEncoder::new(new_writer());
        for _ in 0..1000 {
            enc.encode_terminate_continue()?;
        }
        let mut writer = enc.encode_terminate_finish()?;
        writer.byte_align()?;
        let data = writer.into_writer().into_inner();
        // 1000 * ~0.01 bits plus ~10 bits of flush overhead
        assert!(data.len() <= 4, "stream was {} bytes", data.len());

        let mut dec = CabacDecoder::new(new_reader(data))?;
        for _ in 0..1000 {
            assert!(!dec.decode_terminate()?);
        }
        assert!(dec.decode_terminate()?);
        Ok(())
    }

    #[test]
    fn terminate_immediately() -> Result<()> {
        let enc = CabacEncoder::new(new_writer());
        let mut writer = enc.encode_terminate_finish()?;
        writer.byte_align()?;
        let data = writer.into_writer().into_inner();

        let mut dec = CabacDecoder::new(new_reader(data))?;
        assert!(dec.decode_terminate()?);
        Ok(())
    }

    #[test]
    fn use_after_terminate_errors() -> Result<()> {
        let mut enc = CabacEncoder::new(new_writer());
        let mut ctx = CabacContext::new();
        enc.encode_decision(true, &mut ctx)?;
        let mut writer = enc.encode_terminate_finish()?;
        writer.byte_align()?;
        let data = writer.into_writer().into_inner();

        let mut dec = CabacDecoder::new(new_reader(data))?;
        let mut ctx = CabacContext::new();
        assert!(dec.decode_decision(&mut ctx)?);
        assert!(dec.decode_terminate()?);
        assert!(matches!(
            dec.decode_decision(&mut ctx),
            Err(Error::DecodeAfterTerminate { .. })
        ));
        assert!(matches!(
            dec.decode_bypass(),
            Err(Error::DecodeAfterTerminate { .. })
        ));
        assert!(matches!(
            dec.decode_terminate(),
            Err(Error::DecodeAfterTerminate { .. })
        ));
        Ok(())
    }
}
