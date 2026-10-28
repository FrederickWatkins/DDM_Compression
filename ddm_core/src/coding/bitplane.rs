//! Encoding to bitplanes
use bitstream_io::{BitRead, BitWrite};
use ndarray::{ArrayView2, ArrayViewMut2};

use crate::coding::{CabacContext, CabacDecoder, CabacEncoder, WaveletPixelData, cabac};

/// Options for significant bit encoding
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SignificantEncoding {
    /// Use context
    UseMainContext,
    /// Don't use context
    Bypass,
}

/// Options for neighbour encoding
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NeighbourEncoding {
    /// Use same context as significant bits
    UseMainContext,
    /// Use own context
    UseOwnContext,
    /// Don't use context
    Bypass,
}

/// Options for refinement encoding
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RefinementEncoding {
    /// Use same context as significant bits
    UseMainContext,
    /// Use same context as neighbour bits
    UseNeighbourContext,
    /// Use own context
    UseOwnContext,
    /// Don't use context
    Bypass,
}

/// Options for sign encoding
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SignEncoding {
    /// Encode with sign-magnitude with bitplanes
    Bitplanes {
        /// Bypass sign bit through encoder
        bypass_sign: bool,
    },
    /// Encode with twos complement
    TwosComplement {
        /// Bypass all bits through encoder
        bypass: bool,
    },
}

/// Options for quantisation
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Quantisation {
    /// Encode all subbands with same quantisation
    FixedQuantisation {
        /// Bits of quantisation
        bits: u8,
    },
    /// Encode with quantisation (max stage - stage + 2) * `base_bits` for detail bands, or
    /// `base_bits` for DC
    AdaptiveQuantisation {
        /// Bits to quantise DC coefficients
        base_bits: u8,
    },
}

impl Quantisation {
    /// Quantisation for DC coefficients
    fn dc_quantisation(self) -> u8 {
        match self {
            Self::FixedQuantisation { bits } => bits,
            Self::AdaptiveQuantisation { base_bits } => base_bits,
        }
    }

    /// Quantisation for AC index. This function is messy but self consistent when used in module
    fn ac_quantisation(self, index: u8) -> u8 {
        match self {
            Self::FixedQuantisation { bits } => bits,
            Self::AdaptiveQuantisation { base_bits } => base_bits + index + 1,
        }
    }
}

/// Config for encoding to bitplanes
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BitplaneConfig {
    /// How to quantise coefficients
    pub quantisation: Quantisation,
    /// How to encode DC coefficients
    pub dc_encoding: SignEncoding,
    /// How to encode AC coefficients
    pub ac_encoding: SignEncoding,
    /// How to encode significant bits
    pub significant_encoding: SignificantEncoding,
    /// How to encode neighbours to significant bits
    pub neighbour_encoding: NeighbourEncoding,
    /// How to encode refinement bits
    pub refinement_encoding: RefinementEncoding,
}

/// Config for encoding single subband
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SubbandConfig {
    /// Whether to route the sign through the bypass bin
    pub bypass_sign: bool,
    /// How much to quantise this subband
    pub quantisation: u8,
    /// How to encode significant bits
    pub significant_encoding: SignificantEncoding,
    /// How to encode neighbours to significant bits
    pub neighbour_encoding: NeighbourEncoding,
    /// How to encode refinement bits
    pub refinement_encoding: RefinementEncoding,
}

/// How a magnitude bit is coded. Decided only from bits above it, so the decoder can reproduce it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BitClass {
    /// No higher bit set: this may be the leading 1 (a sign follows if it is)
    Significant,
    /// Leading bit is directly above this one
    Neighbour,
    /// Everything further below
    Refinement,
}

impl BitClass {
    /// `magnitude` may include bits at or below `bit_idx`; they are ignored
    fn of(magnitude: u32, bit_idx: u8) -> Self {
        match u64::from(magnitude) >> (u32::from(bit_idx) + 1) {
            0 => Self::Significant,
            1 => Self::Neighbour,
            _ => Self::Refinement,
        }
    }
}

/// Bitplane enoded pixel data
pub struct BitplanePixelData {
    data: WaveletPixelData,
}

impl BitplanePixelData {
    /// Create new from [`crate::coding::WaveletPixelData`]
    pub fn new(data: WaveletPixelData) -> Self {
        Self { data }
    }

    /// Get minimum number of bits to represent dc coefficients in twos complement
    pub fn dc_bitdepth_twos_complement(&self) -> u8 {
        self.data.dc_subband().map_or(0, |subband| {
            subband
                .iter()
                .copied()
                .map(twos_complement_bitdepth)
                .max()
                .unwrap_or(0)
        })
    }

    /// Get minimum number of magnitude bits to represent dc coefficients
    pub fn dc_bitdepth_magnitude(&self) -> u8 {
        self.data.dc_subband().map_or(0, |subband| {
            subband
                .iter()
                .copied()
                .map(magnitude_bitdepth)
                .max()
                .unwrap_or(0)
        })
    }

    /// Get minimum number of bits to represent ac coefficients in twos complement
    pub fn ac_bitdepth_twos_complement(&self) -> u8 {
        self.data
            .ac_subbands()
            .flatten()
            .filter_map(|subband| subband.iter().copied().map(twos_complement_bitdepth).max())
            .max()
            .unwrap_or(0)
    }

    /// Get minimum number of magnitude bits to represent ac coefficients
    pub fn ac_bitdepth_magnitude(&self) -> u8 {
        self.data
            .ac_subbands()
            .flatten()
            .filter_map(|subband| subband.iter().copied().map(magnitude_bitdepth).max())
            .max()
            .unwrap_or(0)
    }

    /// Encode all data with the requested configuration
    ///
    /// Note that this function does not encode the configuration itself as a header. This must be
    /// done by the caller if the bitstream needs to be decodable on its own
    ///
    /// # Errors
    /// Passes up any error from the encoder
    pub fn encode<W: BitWrite>(
        &self,
        writer: &mut W,
        config: BitplaneConfig,
    ) -> Result<(), cabac::Error> {
        let mut main_ctx = CabacContext::new();
        let mut neighbour_ctx = CabacContext::new();
        let mut refinement_ctx = CabacContext::new();

        let mut encoder = CabacEncoder::new(writer);

        if let SignEncoding::TwosComplement { bypass } = config.dc_encoding
            && let Some(subband) = self.data.dc_subband()
        {
            let bitdepth = self.dc_bitdepth_twos_complement();
            encode_subband_twos_complement(
                &subband,
                &mut encoder,
                &mut main_ctx,
                bypass,
                config.quantisation.dc_quantisation(),
                bitdepth,
            )?;
        }
        if let SignEncoding::TwosComplement { bypass } = config.ac_encoding {
            let bitdepth = self.ac_bitdepth_twos_complement();
            for (i, stage) in self.data.ac_subbands().enumerate() {
                for subband in stage {
                    #[expect(clippy::cast_possible_truncation, reason = "Stage can't be above u8")]
                    encode_subband_twos_complement(
                        &subband,
                        &mut encoder,
                        &mut main_ctx,
                        bypass,
                        config.quantisation.ac_quantisation(i as u8),
                        bitdepth,
                    )?;
                }
            }
        }
        for bit_idx in (0..=32).rev() {
            self.encode_bitplane(
                bit_idx,
                &mut encoder,
                &mut main_ctx,
                &mut neighbour_ctx,
                &mut refinement_ctx,
                config,
            )?;
        }
        encoder.encode_flush()?;
        Ok(())
    }

    /// Decode from the provided reader
    ///
    /// Note that data can be provided by creating an empty [`WaveletPixelData`] using
    /// [`WaveletPixelData::empty`]. The dimensions and stage of `data`, as well as `dc_bitdepth`
    /// and `ac_bitdepth` must match the original.
    ///
    /// # Errors
    /// Passes up errors from the encoder
    pub fn decode<R: BitRead>(
        data: WaveletPixelData,
        reader: &mut R,
        config: BitplaneConfig,
        dc_bitdepth: u8,
        ac_bitdepth: u8,
    ) -> Result<Self, cabac::Error> {
        let mut output = Self { data };

        let mut main_ctx = CabacContext::new();
        let mut neighbour_ctx = CabacContext::new();
        let mut refinement_ctx = CabacContext::new();

        let mut decoder = CabacDecoder::new(reader)?;

        if let SignEncoding::TwosComplement { bypass } = config.dc_encoding
            && let Some(mut subband) = output.data.dc_subband_mut()
        {
            decode_subband_twos_complement(
                &mut subband,
                &mut decoder,
                &mut main_ctx,
                bypass,
                config.quantisation.dc_quantisation(),
                dc_bitdepth,
            )?;
        }
        if let SignEncoding::TwosComplement { bypass } = config.ac_encoding {
            output.data.for_each_ac_subband_mut(|index, mut subband| {
                decode_subband_twos_complement(
                    &mut subband,
                    &mut decoder,
                    &mut main_ctx,
                    bypass,
                    config.quantisation.ac_quantisation(index),
                    ac_bitdepth,
                )
            })?;
        }

        let dc_planes = match config.dc_encoding {
            SignEncoding::Bitplanes { .. } => dc_bitdepth,
            SignEncoding::TwosComplement { .. } => 0,
        };
        let ac_planes = match config.ac_encoding {
            SignEncoding::Bitplanes { .. } => ac_bitdepth,
            SignEncoding::TwosComplement { .. } => 0,
        };
        for bit_idx in (0..dc_planes.max(ac_planes)).rev() {
            if bit_idx < dc_planes
                && let SignEncoding::Bitplanes { bypass_sign } = config.dc_encoding
                && let Some(mut subband) = output.data.dc_subband_mut()
            {
                decode_subband_bitplane(
                    &mut subband,
                    bit_idx,
                    &mut decoder,
                    &mut main_ctx,
                    &mut neighbour_ctx,
                    &mut refinement_ctx,
                    SubbandConfig {
                        bypass_sign,
                        quantisation: config.quantisation.dc_quantisation(),
                        significant_encoding: config.significant_encoding,
                        neighbour_encoding: config.neighbour_encoding,
                        refinement_encoding: config.refinement_encoding,
                    },
                )?;
            }
            if bit_idx < ac_planes
                && let SignEncoding::Bitplanes { bypass_sign } = config.ac_encoding
            {
                output.data.for_each_ac_subband_mut(|index, mut subband| {
                    decode_subband_bitplane(
                        &mut subband,
                        bit_idx,
                        &mut decoder,
                        &mut main_ctx,
                        &mut neighbour_ctx,
                        &mut refinement_ctx,
                        SubbandConfig {
                            bypass_sign,
                            quantisation: config.quantisation.ac_quantisation(index),
                            significant_encoding: config.significant_encoding,
                            neighbour_encoding: config.neighbour_encoding,
                            refinement_encoding: config.refinement_encoding,
                        },
                    )
                })?;
            }
        }

        Ok(output)
    }

    /// Encode bitplane `bit_idx` to encoder
    ///
    /// # Errors
    /// Passes up errors from encoder
    fn encode_bitplane<W: BitWrite>(
        &self,
        bit_idx: u8,
        encoder: &mut CabacEncoder<W>,
        main_ctx: &mut CabacContext,
        neighbour_ctx: &mut CabacContext,
        refinement_ctx: &mut CabacContext,
        config: BitplaneConfig,
    ) -> Result<(), cabac::Error> {
        if let Some(subband) = self.data.dc_subband()
            && let SignEncoding::Bitplanes { bypass_sign } = config.dc_encoding
            && bit_idx < self.dc_bitdepth_magnitude()
        {
            encode_subband_bitplane(
                &subband,
                bit_idx,
                encoder,
                main_ctx,
                neighbour_ctx,
                refinement_ctx,
                SubbandConfig {
                    bypass_sign,
                    quantisation: config.quantisation.dc_quantisation(),
                    significant_encoding: config.significant_encoding,
                    neighbour_encoding: config.neighbour_encoding,
                    refinement_encoding: config.refinement_encoding,
                },
            )?;
        }
        if let SignEncoding::Bitplanes { bypass_sign } = config.ac_encoding
            && bit_idx < self.ac_bitdepth_magnitude()
        {
            for (i, stage) in self.data.ac_subbands().enumerate() {
                for subband in stage {
                    #[expect(clippy::cast_possible_truncation, reason = "Stage can't be above u8")]
                    encode_subband_bitplane(
                        &subband,
                        bit_idx,
                        encoder,
                        main_ctx,
                        neighbour_ctx,
                        refinement_ctx,
                        SubbandConfig {
                            bypass_sign,
                            quantisation: config.quantisation.ac_quantisation(i as u8),
                            significant_encoding: config.significant_encoding,
                            neighbour_encoding: config.neighbour_encoding,
                            refinement_encoding: config.refinement_encoding,
                        },
                    )?;
                }
            }
        }
        Ok(())
    }
}

/// Get minimum number of bits to represent `x` in two's complement
#[expect(clippy::cast_possible_truncation, reason = "Bit length is at most 32")]
fn twos_complement_bitdepth(x: i32) -> u8 {
    if x == 0 {
        return 0;
    }
    let sign_run = x.leading_zeros().max(x.leading_ones());
    (i32::BITS + 1 - sign_run) as u8
}

/// Get minimum number of bits to represent magnitude of `x`
#[expect(clippy::cast_possible_truncation, reason = "Bit length is at most 32")]
fn magnitude_bitdepth(x: i32) -> u8 {
    x.unsigned_abs().checked_ilog2().map_or(0, |x| x as u8 + 1)
}

/// Encode a single subband to a bitplane
fn encode_subband_bitplane<W: BitWrite>(
    subband: &ArrayView2<i32>,
    bit_idx: u8,
    encoder: &mut CabacEncoder<W>,
    main_ctx: &mut CabacContext,
    neighbour_ctx: &mut CabacContext,
    refinement_ctx: &mut CabacContext,
    config: SubbandConfig,
) -> Result<(), cabac::Error> {
    if bit_idx < config.quantisation {
        return Ok(());
    }
    for pixel in subband {
        encode_subband_pixel(
            *pixel,
            bit_idx,
            encoder,
            main_ctx,
            neighbour_ctx,
            refinement_ctx,
            config,
        )?;
    }
    Ok(())
}

fn decode_subband_bitplane<R: BitRead>(
    subband: &mut ArrayViewMut2<i32>,
    bit_idx: u8,
    decoder: &mut CabacDecoder<R>,
    main_ctx: &mut CabacContext,
    neighbour_ctx: &mut CabacContext,
    refinement_ctx: &mut CabacContext,
    config: SubbandConfig,
) -> Result<(), cabac::Error> {
    if bit_idx < config.quantisation {
        return Ok(());
    }
    for pixel in subband.iter_mut() {
        decode_subband_pixel(
            pixel,
            bit_idx,
            decoder,
            main_ctx,
            neighbour_ctx,
            refinement_ctx,
            config,
        )?;
    }
    Ok(())
}

fn encode_subband_pixel<W: BitWrite>(
    pixel: i32,
    bit_idx: u8,
    encoder: &mut CabacEncoder<W>,
    main_ctx: &mut CabacContext,
    neighbour_ctx: &mut CabacContext,
    refinement_ctx: &mut CabacContext,
    config: SubbandConfig,
) -> Result<(), cabac::Error> {
    let magnitude = pixel.unsigned_abs();
    let bit_value = (magnitude >> bit_idx) & 1 == 1;
    let pixel_sign = pixel < 0;
    match BitClass::of(magnitude, bit_idx) {
        BitClass::Significant => {
            match config.significant_encoding {
                SignificantEncoding::UseMainContext => {
                    encoder.encode_decision(bit_value, main_ctx)?;
                }
                SignificantEncoding::Bypass => encoder.encode_bypass(bit_value)?,
            }
            // Sign only goes with the leading 1, so zero pixels never get one
            if bit_value {
                if config.bypass_sign {
                    encoder.encode_bypass(pixel_sign)?;
                } else {
                    encoder.encode_decision(pixel_sign, main_ctx)?;
                }
            }
        }
        BitClass::Neighbour => match config.neighbour_encoding {
            NeighbourEncoding::UseMainContext => {
                encoder.encode_decision(bit_value, main_ctx)?;
            }
            NeighbourEncoding::UseOwnContext => {
                encoder.encode_decision(bit_value, neighbour_ctx)?;
            }
            NeighbourEncoding::Bypass => encoder.encode_bypass(bit_value)?,
        },
        BitClass::Refinement => match config.refinement_encoding {
            RefinementEncoding::UseMainContext => {
                encoder.encode_decision(bit_value, main_ctx)?;
            }
            RefinementEncoding::UseNeighbourContext => {
                encoder.encode_decision(bit_value, neighbour_ctx)?;
            }
            RefinementEncoding::UseOwnContext => {
                encoder.encode_decision(bit_value, refinement_ctx)?;
            }
            RefinementEncoding::Bypass => encoder.encode_bypass(bit_value)?,
        },
    }
    Ok(())
}

fn decode_subband_pixel<R: BitRead>(
    pixel: &mut i32,
    bit_idx: u8,
    decoder: &mut CabacDecoder<R>,
    main_ctx: &mut CabacContext,
    neighbour_ctx: &mut CabacContext,
    refinement_ctx: &mut CabacContext,
    config: SubbandConfig,
) -> Result<(), cabac::Error> {
    let class = BitClass::of(pixel.unsigned_abs(), bit_idx);
    let bit_value = match class {
        BitClass::Significant => match config.significant_encoding {
            SignificantEncoding::UseMainContext => decoder.decode_decision(main_ctx)?,
            SignificantEncoding::Bypass => decoder.decode_bypass()?,
        },
        BitClass::Neighbour => match config.neighbour_encoding {
            NeighbourEncoding::UseMainContext => decoder.decode_decision(main_ctx)?,
            NeighbourEncoding::UseOwnContext => decoder.decode_decision(neighbour_ctx)?,
            NeighbourEncoding::Bypass => decoder.decode_bypass()?,
        },
        BitClass::Refinement => match config.refinement_encoding {
            RefinementEncoding::UseMainContext => decoder.decode_decision(main_ctx)?,
            RefinementEncoding::UseNeighbourContext => decoder.decode_decision(neighbour_ctx)?,
            RefinementEncoding::UseOwnContext => decoder.decode_decision(refinement_ctx)?,
            RefinementEncoding::Bypass => decoder.decode_bypass()?,
        },
    };
    if !bit_value {
        return Ok(());
    }
    // The sign is only coded with the leading 1; after that it's already in `pixel`
    let negative = if class == BitClass::Significant {
        if config.bypass_sign {
            decoder.decode_bypass()?
        } else {
            decoder.decode_decision(main_ctx)?
        }
    } else {
        *pixel < 0
    };
    let bit = (1u32 << bit_idx).cast_signed();
    // Wrapping so that a leading 1 at plane 31 with a negative sign gives i32::MIN
    *pixel = if negative {
        pixel.wrapping_sub(bit)
    } else {
        pixel.wrapping_add(bit)
    };
    Ok(())
}

fn encode_subband_twos_complement<W: BitWrite>(
    subband: &ArrayView2<i32>,
    encoder: &mut CabacEncoder<W>,
    main_ctx: &mut CabacContext,
    bypass: bool,
    quantisation: u8,
    bitdepth: u8,
) -> Result<(), cabac::Error> {
    for pixel in subband {
        for bit_idx in (quantisation..bitdepth).rev() {
            let bit_val = (pixel >> bit_idx) & 1 == 1;
            if bypass {
                encoder.encode_bypass(bit_val)?;
            } else {
                encoder.encode_decision(bit_val, main_ctx)?;
            }
        }
    }
    Ok(())
}

fn decode_subband_twos_complement<R: BitRead>(
    subband: &mut ArrayViewMut2<i32>,
    decoder: &mut CabacDecoder<R>,
    main_ctx: &mut CabacContext,
    bypass: bool,
    quantisation: u8,
    bitdepth: u8,
) -> Result<(), cabac::Error> {
    for pixel in subband.iter_mut() {
        let mut bits = 0u32;
        for bit_idx in (quantisation..bitdepth).rev() {
            let bit = if bypass {
                decoder.decode_bypass()?
            } else {
                decoder.decode_decision(main_ctx)?
            };
            bits |= u32::from(bit) << bit_idx;
        }
        *pixel = sign_extend(bits, bitdepth);
    }
    Ok(())
}

/// Sign extend the low `width` bits of `bits`
fn sign_extend(bits: u32, width: u8) -> i32 {
    if width == 0 {
        return 0;
    }
    let shift = u32::BITS - u32::from(width);
    (bits << shift).cast_signed() >> shift
}
