/// Benchmark algorithm configurations on dataset
use std::io::Cursor;

use bitstream_io::{BigEndian, BitReader, BitWriter};
use color_eyre::eyre;
use ddm_core::coding::{
    BitplaneConfig, BitplanePixelData, WaveletPixelData,
    bitplane::{
        NeighbourEncoding, Quantisation, RefinementEncoding, SignEncoding, SignificantEncoding,
    },
    wavelet::{MallatSubband, WaveletSubband},
};
use duckdb::Connection;
use ndarray::Array2;

/// Range of wavelet stages to sweep
const WAVELET_STAGES: std::ops::RangeInclusive<usize> = 1..=5;
/// Range of quantisation bits to sweep (used for both fixed and adaptive)
const QUANTISATION_BITS: std::ops::RangeInclusive<u8> = 0..=8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BenchmarkConfig {
    pub dc_adjust: bool,
    pub wavelet_stages: usize,
    pub bitplane_config: BitplaneConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct BenchmarkResults {
    pub size: usize,
    pub mse: f64,
    pub rmse: f64,
    pub psnr: f64,
}

/// Run benchmarks on all configurations and write to duckdb `conn`
pub fn benchmark_all(test_frames: &Vec<Array2<i32>>, conn: Connection) -> eyre::Result<()> {
    // Calculate 99.9 percentile value for PSNR calculations
    let p999 = percentile_999(test_frames)?;
    for config in all_configs() {
        results = benchmark(test_frames, config, p999);
        // Write config and results into duckdb here
    }

    Ok(())
}

fn benchmark(
    test_frames: &Vec<Array2<i32>>,
    config: BenchmarkConfig,
    p999: i32,
) -> eyre::Result<BenchmarkResults> {
    let mut size = 0;
    let mut mse: f64 = 0.0;
    for frame in test_frames {
        let (frame_size, frame_mse) = roundtrip_frame(frame, config)?;
        size += frame_size;
        mse += frame_mse;
    }
    let size = size / test_frames.len();
    #[expect(clippy::cast_precision_loss, reason = "Frames should be around 100000")]
    let mse = mse / test_frames.len() as f64;
    let rmse = mse.sqrt();
    let psnr = 20.0 * (f64::from(p999) / rmse).log10();

    Ok(BenchmarkResults {
        size,
        mse,
        rmse,
        psnr,
    })
}

fn roundtrip_frame(frame: &Array2<i32>, config: BenchmarkConfig) -> eyre::Result<(usize, f64)> {
    let mut owned_frame = frame.clone();
    if config.dc_adjust {
        let mean = owned_frame.mean().ok_or(eyre::eyre!("Empty frame"))?;
        owned_frame.map_inplace(|x| *x -= mean);
    }
    let mut wavelet_pixels = WaveletPixelData::new(owned_frame);
    wavelet_pixels.dwt_mallat(config.wavelet_stages)?;
    let bitplane_pixels = BitplanePixelData::new(wavelet_pixels);

    let mut buf = vec![];
    let mut cursor = Cursor::new(&mut buf);
    let mut bitwriter = BitWriter::<_, BigEndian>::new(&mut cursor);

    let (dc_bitdepth, ac_bitdepth) =
        bitplane_pixels.encode(&mut bitwriter, config.bitplane_config)?;
    let size = buf.len();

    // Zero out data before decoding to make sure we aren't cheating
    let mut wavelet_pixels = bitplane_pixels.into_wavelet_data();
    wavelet_pixels
        .get_mut(MallatSubband {
            stage: 0,
            subband: WaveletSubband::LL,
        })
        .ok_or(eyre::eyre!("Pixel data empty"))?
        .map_inplace(|x| *x = 0);

    let mut cursor = Cursor::new(&mut buf);
    let mut bitreader = BitReader::<_, BigEndian>::new(&mut cursor);

    let mut wavelet_pixels = BitplanePixelData::decode(
        wavelet_pixels,
        &mut bitreader,
        config.bitplane_config,
        dc_bitdepth,
        ac_bitdepth,
    )?
    .into_wavelet_data();
    wavelet_pixels.idwt_mallat();
    let roundtripped = wavelet_pixels.into_inner();

    let mse = compare_frames(frame, &roundtripped);

    Ok((size, mse))
}

fn compare_frames(frame: &Array2<i32>, roundtripped: &Array2<i32>) -> f64 {
    frame
        .iter()
        .zip(roundtripped.iter())
        .map(|(reference, altered)| f64::from(reference - altered).powi(2))
        .sum::<f64>()
        / frame.len() as f64
}

fn all_configs() -> impl Iterator<Item = BenchmarkConfig> {
    // Generate iterator of every possible config here (probably use itertools cartesian product)
    todo!()
}

/// 99.9th percentile of all pixel values across all frames
fn percentile_999(frames: &[Array2<i32>]) -> eyre::Result<i32> {
    let mut values: Vec<i32> = frames.iter().flat_map(|f| f.iter().copied()).collect();
    eyre::ensure!(!values.is_empty(), "No pixels to compute percentile from");

    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "Doesn't need to be precise"
    )]
    let index = ((values.len() as f64 * 0.999).ceil() as usize)
        .saturating_sub(1)
        .min(values.len() - 1);

    let (_, value, _) = values.select_nth_unstable(index);
    Ok(*value)
}


fn sign_encodings() -> Vec<SignEncoding> {
    [false, true]
        .into_iter()
        .flat_map(|b| {
            [
                SignEncoding::Bitplanes { bypass_sign: b },
                SignEncoding::TwosComplement { bypass: b },
            ]
        })
        .collect()
}
 
fn quantisations() -> Vec<Quantisation> {
    QUANTISATION_BITS
        .flat_map(|bits| {
            [
                Quantisation::FixedQuantisation { bits },
                Quantisation::AdaptiveQuantisation { base_bits: bits },
            ]
        })
        .collect()
}
 
/// Every combination of options
fn all_configs() -> impl Iterator<Item = BenchmarkConfig> {
    iproduct!(
        [false, true],
        WAVELET_STAGES,
        quantisations(),
        sign_encodings(), // dc
        sign_encodings(), // ac
        [SignificantEncoding::UseMainContext, SignificantEncoding::Bypass],
        [
            NeighbourEncoding::UseMainContext,
            NeighbourEncoding::UseOwnContext,
            NeighbourEncoding::Bypass,
        ],
        [
            RefinementEncoding::UseMainContext,
            RefinementEncoding::UseNeighbourContext,
            RefinementEncoding::UseOwnContext,
            RefinementEncoding::Bypass,
        ]
    )
    .map(
        |(
            dc_adjust,
            wavelet_stages,
            quantisation,
            dc_encoding,
            ac_encoding,
            significant_encoding,
            neighbour_encoding,
            refinement_encoding,
        )| BenchmarkConfig {
            dc_adjust,
            wavelet_stages,
            bitplane_config: BitplaneConfig {
                quantisation,
                dc_encoding,
                ac_encoding,
                significant_encoding,
                neighbour_encoding,
                refinement_encoding,
            },
        },
    )
}
