//! Encoding to bitplanes
use bitstream_io::BitWrite;
use ndarray::ArrayView2;

use crate::coding::{CabacContext, CabacEncoder, WaveletPixelData, cabac};

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
        todo!()
    }

    /// Encode bitplane `bit_idx` to encoder
    ///
    /// # Errors
    /// Passes up errors from encoder
    pub fn encode_bitplane<W: BitWrite>(
        &self,
        bit_idx: u8,
        encoder: &mut CabacEncoder<W>,
        main_context: &mut CabacContext,
        neighbour_context: &mut CabacContext,
        refinement_context: &mut CabacContext,
        config: BitplaneConfig,
    ) -> Result<(), cabac::Error> {
        if let Some(subband) = self.data.dc_subband()
            && let SignEncoding::Bitplanes { bypass_sign } = config.dc_encoding
        {
            encode_subband_bitplane(
                &subband,
                bit_idx,
                encoder,
                main_context,
                neighbour_context,
                refinement_context,
                SubbandConfig {
                    bypass_sign,
                    quantisation: config.quantisation.dc_quantisation(),
                    significant_encoding: config.significant_encoding,
                    neighbour_encoding: config.neighbour_encoding,
                    refinement_encoding: config.refinement_encoding,
                },
            )?;
        }
        if let SignEncoding::Bitplanes { bypass_sign } = config.ac_encoding {
            for (i, stage) in self.data.ac_subbands().enumerate() {
                for subband in stage {
                    #[expect(clippy::cast_possible_truncation, reason = "Stage can't be above u8")]
                    encode_subband_bitplane(
                        &subband,
                        bit_idx,
                        encoder,
                        main_context,
                        neighbour_context,
                        refinement_context,
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
    main_context: &mut CabacContext,
    neighbour_context: &mut CabacContext,
    refinement_context: &mut CabacContext,
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
            main_context,
            neighbour_context,
            refinement_context,
            config,
        )?;
    }
    Ok(())
}

fn encode_subband_pixel<W: BitWrite>(
    pixel: i32,
    bit_idx: u8,
    encoder: &mut CabacEncoder<W>,
    main_context: &mut CabacContext,
    neighbour_context: &mut CabacContext,
    refinement_context: &mut CabacContext,
    config: SubbandConfig,
) -> Result<(), cabac::Error> {
    let bit_value = (pixel.unsigned_abs() >> config.quantisation) >> bit_idx == 1;
    let pixel_sign = pixel < 0;
    if bit_is_significant(pixel, bit_idx) {
        match config.significant_encoding {
            SignificantEncoding::UseMainContext => {
                encoder.encode_decision(bit_value, main_context)?;
            }
            SignificantEncoding::Bypass => encoder.encode_bypass(bit_value)?,
        }
        if config.bypass_sign {
            encoder.encode_bypass(pixel_sign)?;
        } else {
            encoder.encode_decision(pixel_sign, main_context)?;
        }
    } else if bit_is_neighbour(pixel, bit_idx) {
        match config.neighbour_encoding {
            NeighbourEncoding::UseMainContext => {
                encoder.encode_decision(bit_value, main_context)?;
            }
            NeighbourEncoding::UseOwnContext => {
                encoder.encode_decision(bit_value, neighbour_context)?;
            }
            NeighbourEncoding::Bypass => encoder.encode_bypass(bit_value)?,
        }
    } else {
        match config.refinement_encoding {
            RefinementEncoding::UseMainContext => {
                encoder.encode_decision(bit_value, main_context)?;
            }
            RefinementEncoding::UseNeighbourContext => {
                encoder.encode_decision(bit_value, neighbour_context)?;
            }
            RefinementEncoding::UseOwnContext => {
                encoder.encode_decision(bit_value, refinement_context)?;
            }
            RefinementEncoding::Bypass => encoder.encode_bypass(bit_value)?,
        }
    }
    Ok(())
}

fn bit_is_significant(pixel: i32, bit_idx: u8) -> bool {
    pixel.unsigned_abs().checked_ilog2().unwrap_or(0) <= u32::from(bit_idx)
}

fn bit_is_neighbour(pixel: i32, bit_idx: u8) -> bool {
    pixel.unsigned_abs().checked_ilog2().unwrap_or(0) < u32::from(bit_idx)
}
