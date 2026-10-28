//! Encoding tools
pub mod cabac;
pub use cabac::{CabacContext, CabacDecoder, CabacEncoder};

pub mod wavelet;
pub use wavelet::WaveletPixelData;

pub mod bitplane;
pub use bitplane::{BitplaneConfig, BitplanePixelData};

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use bitstream_io::{BigEndian, BitReader, BitWrite, BitWriter};
    use itertools::iproduct;
    use ndarray::Array2;
    use proptest::{prelude::*, test_runner::TestCaseError};

    use super::{
        WaveletPixelData,
        bitplane::{
            BitplaneConfig, BitplanePixelData, NeighbourEncoding, Quantisation, RefinementEncoding,
            SignEncoding, SignificantEncoding,
        },
        wavelet::{MallatSubband, WaveletSubband},
    };

    const IMAGE_SHAPE: (usize, usize) = (8, 8);
    const STAGES: std::ops::RangeInclusive<usize> = 0..=3;

    fn configurations(quantisation: Quantisation) -> impl Iterator<Item = BitplaneConfig> {
        let sign_encodings = [
            SignEncoding::Bitplanes { bypass_sign: false },
            SignEncoding::Bitplanes { bypass_sign: true },
            SignEncoding::TwosComplement { bypass: false },
            SignEncoding::TwosComplement { bypass: true },
        ];
        let significant_encodings = [
            SignificantEncoding::UseMainContext,
            SignificantEncoding::Bypass,
        ];
        let neighbour_encodings = [
            NeighbourEncoding::UseMainContext,
            NeighbourEncoding::UseOwnContext,
            NeighbourEncoding::Bypass,
        ];
        let refinement_encodings = [
            RefinementEncoding::UseMainContext,
            RefinementEncoding::UseNeighbourContext,
            RefinementEncoding::UseOwnContext,
            RefinementEncoding::Bypass,
        ];

        iproduct!(
            sign_encodings,
            sign_encodings,
            significant_encodings,
            neighbour_encodings,
            refinement_encodings
        )
        .map(
            move |(
                dc_encoding,
                ac_encoding,
                significant_encoding,
                neighbour_encoding,
                refinement_encoding,
            )| BitplaneConfig {
                quantisation,
                dc_encoding,
                ac_encoding,
                significant_encoding,
                neighbour_encoding,
                refinement_encoding,
            },
        )
    }

    fn encode_decode(
        pixels: &Array2<i32>,
        stages: usize,
        config: BitplaneConfig,
    ) -> Result<(Array2<i32>, usize, usize), TestCaseError> {
        let mut wavelet_data = WaveletPixelData::new(pixels.clone());
        wavelet_data
            .dwt_mallat(stages)
            .map_err(|error| TestCaseError::fail(format!("Wavelet transform failed: {error:?}")))?;
        prop_assert_eq!(usize::from(wavelet_data.stage()), stages);

        let input = BitplanePixelData::new(wavelet_data);
        let dc_bitdepth = match config.dc_encoding {
            SignEncoding::Bitplanes { .. } => input.dc_bitdepth_magnitude(),
            SignEncoding::TwosComplement { .. } => input.dc_bitdepth_twos_complement(),
        };
        let ac_bitdepth = match config.ac_encoding {
            SignEncoding::Bitplanes { .. } => input.ac_bitdepth_magnitude(),
            SignEncoding::TwosComplement { .. } => input.ac_bitdepth_twos_complement(),
        };

        let mut bytes = Vec::new();
        {
            let mut writer = BitWriter::endian(&mut bytes, BigEndian);
            input
                .encode(&mut writer, config)
                .map_err(|error| TestCaseError::fail(format!("Encoding failed: {error:?}")))?;
            writer.byte_align().map_err(|error| {
                TestCaseError::fail(format!("Byte alignment failed: {error:?}"))
            })?;
        }
        let bytes_written = bytes.len();

        let stage = u8::try_from(stages)
            .map_err(|error| TestCaseError::fail(format!("Invalid stage count: {error}")))?;
        let mut cursor = Cursor::new(bytes.as_slice());
        let decoded = {
            let mut reader = BitReader::endian(&mut cursor, BigEndian);
            BitplanePixelData::decode(
                WaveletPixelData::empty(IMAGE_SHAPE, stage),
                &mut reader,
                config,
                dc_bitdepth,
                ac_bitdepth,
            )
            .map_err(|error| TestCaseError::fail(format!("Decoding failed: {error:?}")))?
        };
        let bytes_read = usize::try_from(cursor.position())
            .map_err(|error| TestCaseError::fail(format!("Invalid read position: {error}")))?;

        let mut decoded = decoded.into_wavelet_data();
        prop_assert_eq!(usize::from(decoded.stage()), stages);
        decoded.idwt_mallat();
        let pixels = decoded
            .get(MallatSubband {
                stage: 0,
                subband: WaveletSubband::LL,
            })
            .ok_or_else(|| TestCaseError::fail("Decoded pixel data is missing"))?
            .to_owned();

        Ok((pixels, bytes_written, bytes_read))
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 4,
            ..ProptestConfig::default()
        })]

        #[test]
        fn zero_quantisation_round_trips_wavelets_and_bitplanes(
            pixels in prop::collection::vec(-4096i32..=4096, IMAGE_SHAPE.0 * IMAGE_SHAPE.1),
        ) {
            let original = Array2::from_shape_vec(IMAGE_SHAPE, pixels)
                .map_err(|error| TestCaseError::fail(format!("Invalid test image: {error}")))?;

            for stages in STAGES.clone() {
                for config in configurations(Quantisation::FixedQuantisation { bits: 0 }) {
                    let (decoded, bytes_written, bytes_read) =
                        encode_decode(&original, stages, config)?;
                    prop_assert_eq!(
                        bytes_read,
                        bytes_written,
                        "Configuration: {:?}, stages: {}",
                        config,
                        stages
                    );
                    prop_assert_eq!(
                        decoded,
                        original.clone(),
                        "Configuration: {:?}, stages: {}",
                        config,
                        stages
                    );
                }
            }
        }

        #[test]
        fn nonzero_quantisation_consumes_the_encoded_stream(
            pixels in prop::collection::vec(-4096i32..=4096, IMAGE_SHAPE.0 * IMAGE_SHAPE.1),
        ) {
            let original = Array2::from_shape_vec(IMAGE_SHAPE, pixels)
                .map_err(|error| TestCaseError::fail(format!("Invalid test image: {error}")))?;

            for stages in STAGES.clone() {
                for quantisation in [
                    Quantisation::FixedQuantisation { bits: 2 },
                    Quantisation::AdaptiveQuantisation { base_bits: 1 },
                ] {
                    for config in configurations(quantisation) {
                        let (_, bytes_written, bytes_read) =
                            encode_decode(&original, stages, config)?;
                        prop_assert_eq!(
                            bytes_read,
                            bytes_written,
                            "Configuration: {:?}, stages: {}",
                            config,
                            stages
                        );
                    }
                }
            }
        }
    }
}
