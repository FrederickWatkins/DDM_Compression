/// Benchmark algorithm configurations on dataset
use std::io::Cursor;

use bitstream_io::{BigEndian, BitReader, BitWrite, BitWriter};
use color_eyre::eyre;
use ddm_core::coding::{
    BitplaneConfig, BitplanePixelData, WaveletPixelData,
    bitplane::{
        NeighbourEncoding, Quantisation, RefinementEncoding, SignEncoding, SignificantEncoding,
    },
    wavelet::{MallatSubband, WaveletSubband},
};
use duckdb::{Connection, params};
use itertools::iproduct;
use ndarray::{Array2, s};

/// Range of wavelet stages to sweep
const WAVELET_STAGES: std::ops::RangeInclusive<usize> = 0..=2;
/// Range of quantisation bits to sweep (used for both fixed and adaptive)
const QUANTISATION_BITS: std::ops::RangeInclusive<u8> = 0..=8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BenchmarkConfig {
    pub dc_adjust: bool,
    pub wavelet_stages: usize,
    pub bitplane_config: BitplaneConfig,
}

/// Number of rows at the top of the DDM that make up the noise box.
/// ASSUMPTION: row 0 is the top of the DDM and the noise box spans all columns.
/// Adjust to match your DDM geometry.
const NOISE_BOX_ROWS: usize = 16;

/// Signal/noise measurements taken from a single DDM
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct DdmMetrics {
    /// Estimated power at the peak of the DDM (signal + noise)
    pub peak_power: f64,
    /// Mean power in the noise box
    pub noise: f64,
    /// Unbiased SNR: (`peak_power` - `noise`) / `noise`, linear
    pub snr: f64,
    /// Signal-only power estimate: `peak_power` - `noise`
    pub signal: f64,
}

impl DdmMetrics {
    /// Measure peak power, noise, SNR and signal-only power of a DDM
    pub fn measure(frame: &Array2<i32>) -> eyre::Result<Self> {
        eyre::ensure!(
            frame.nrows() > NOISE_BOX_ROWS,
            "Frame has {} rows, need more than {NOISE_BOX_ROWS} for the noise box",
            frame.nrows()
        );

        let peak_power = frame
            .iter()
            .copied()
            .max()
            .map(f64::from)
            .ok_or(eyre::eyre!("Empty frame"))?;

        let noise_box = frame.slice(s![..NOISE_BOX_ROWS, ..]);
        #[expect(clippy::cast_precision_loss, reason = "Noise box is small")]
        let noise = noise_box.iter().map(|&x| f64::from(x)).sum::<f64>() / noise_box.len() as f64;

        let signal = peak_power - noise;
        let snr = signal / noise;

        Ok(Self {
            peak_power,
            noise,
            snr,
            signal,
        })
    }

    fn accumulate(&mut self, other: &Self) {
        self.peak_power += other.peak_power;
        self.noise += other.noise;
        self.snr += other.snr;
        self.signal += other.signal;
    }

    fn divided_by(self, n: f64) -> Self {
        Self {
            peak_power: self.peak_power / n,
            noise: self.noise / n,
            snr: self.snr / n,
            signal: self.signal / n,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct BenchmarkResults {
    /// Mean encoded size per frame, in bytes
    pub size: usize,
    pub mse: f64,
    pub rmse: f64,
    pub psnr: f64,
    /// DDM metrics of the original frames (mean over frames)
    pub reference: DdmMetrics,
    /// DDM metrics of the roundtripped frames (mean over frames)
    pub roundtripped: DdmMetrics,
}

const CREATE_TABLE: &str = "
CREATE TABLE IF NOT EXISTS benchmarks (
    dc_adjust           BOOLEAN,
    wavelet_stages      BIGINT,
    quantisation_kind   VARCHAR,   -- 'Fixed' | 'Adaptive'
    quantisation_bits   UTINYINT,  -- bits (Fixed) or base_bits (Adaptive)
    dc_sign_kind        VARCHAR,   -- 'Bitplanes' | 'TwosComplement'
    dc_bypass           BOOLEAN,   -- bypass_sign (Bitplanes) or bypass (TwosComplement)
    ac_sign_kind        VARCHAR,
    ac_bypass           BOOLEAN,
    significant_encoding VARCHAR,
    neighbour_encoding  VARCHAR,
    refinement_encoding VARCHAR,
    size                BIGINT,
    mse                 DOUBLE,
    rmse                DOUBLE,
    psnr                DOUBLE,
    ref_peak_power      DOUBLE,    -- DDM metrics of original frames
    ref_noise           DOUBLE,
    ref_snr             DOUBLE,
    ref_signal          DOUBLE,
    rt_peak_power       DOUBLE,    -- DDM metrics of roundtripped frames
    rt_noise            DOUBLE,
    rt_snr              DOUBLE,
    rt_signal           DOUBLE
)";

/// Number of columns in the `benchmarks` table
const COLUMN_COUNT: usize = 23;

fn insert_sql() -> String {
    format!(
        "INSERT INTO benchmarks VALUES ({})",
        vec!["?"; COLUMN_COUNT].join(", ")
    )
}

/// Split a quantisation into (variant name, inner bits)
fn split_quantisation(q: Quantisation) -> (&'static str, u8) {
    match q {
        Quantisation::FixedQuantisation { bits } => ("Fixed", bits),
        Quantisation::AdaptiveQuantisation { base_bits } => ("Adaptive", base_bits),
    }
}

/// Split a sign encoding into (variant name, inner bypass flag)
fn split_sign(s: SignEncoding) -> (&'static str, bool) {
    match s {
        SignEncoding::Bitplanes { bypass_sign } => ("Bitplanes", bypass_sign),
        SignEncoding::TwosComplement { bypass } => ("TwosComplement", bypass),
    }
}

/// Run benchmarks on all configurations and write to duckdb `conn`
pub fn benchmark_all(test_frames: &[Array2<i32>], conn: &mut Connection) -> eyre::Result<()> {
    // 99.9 percentile pixel value, used as the peak value for PSNR
    let p999 = percentile_999(test_frames)?;

    conn.execute_batch(CREATE_TABLE)?;
    conn.execute_batch("BEGIN TRANSACTION")?;
    let mut insert = conn.prepare(&insert_sql())?;

    for config in all_configs() {
        let results = match benchmark(test_frames, config, p999) {
            Ok(results) => results,
            Err(err) => {
                // Don't let one bad config kill a long sweep
                eprintln!("Skipping {config:?}: {err:#}");
                continue;
            }
        };

        let bp = config.bitplane_config;
        let (quant_kind, quant_bits) = split_quantisation(bp.quantisation);
        let (dc_kind, dc_bypass) = split_sign(bp.dc_encoding);
        let (ac_kind, ac_bypass) = split_sign(bp.ac_encoding);
        #[expect(
            clippy::cast_possible_wrap,
            reason = "None of these values are anywhere near i64::MAX"
        )]
        insert.execute(params![
            config.dc_adjust,
            config.wavelet_stages as i64,
            quant_kind,
            quant_bits,
            dc_kind,
            dc_bypass,
            ac_kind,
            ac_bypass,
            format!("{:?}", bp.significant_encoding),
            format!("{:?}", bp.neighbour_encoding),
            format!("{:?}", bp.refinement_encoding),
            results.size as i64,
            results.mse,
            results.rmse,
            results.psnr,
            results.reference.peak_power,
            results.reference.noise,
            results.reference.snr,
            results.reference.signal,
            results.roundtripped.peak_power,
            results.roundtripped.noise,
            results.roundtripped.snr,
            results.roundtripped.signal,
        ])?;
    }

    drop(insert);
    conn.execute_batch("COMMIT")?;
    Ok(())
}

/// 99.9th percentile of all pixel values across all frames
fn percentile_999(frames: &[Array2<i32>]) -> eyre::Result<i32> {
    let mut values: Vec<i32> = frames.iter().flat_map(|f| f.iter().copied()).collect();
    eyre::ensure!(!values.is_empty(), "No pixels to compute percentile from");

    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "Its an estimate anyway"
    )]
    let index = ((values.len() as f64 * 0.999).ceil() as usize)
        .saturating_sub(1)
        .min(values.len() - 1);

    let (_, value, _) = values.select_nth_unstable(index);
    Ok(*value)
}

fn benchmark(
    test_frames: &[Array2<i32>],
    config: BenchmarkConfig,
    p999: i32,
) -> eyre::Result<BenchmarkResults> {
    eyre::ensure!(!test_frames.is_empty(), "No test frames");

    let mut size = 0;
    let mut mse: f64 = 0.0;
    let mut reference = DdmMetrics::default();
    let mut roundtripped = DdmMetrics::default();
    for frame in test_frames {
        let result = roundtrip_frame(frame, config)?;
        size += result.size;
        mse += result.mse;
        reference.accumulate(&result.reference);
        roundtripped.accumulate(&result.roundtripped);
    }
    let size = size / test_frames.len();
    #[expect(clippy::cast_precision_loss, reason = "Number of frames is small")]
    let n_frames = test_frames.len() as f64;
    let mse = mse / n_frames;
    // Per-frame metrics are averaged over frames (including SNR)
    let reference = reference.divided_by(n_frames);
    let roundtripped = roundtripped.divided_by(n_frames);
    let rmse = mse.sqrt();
    // rmse == 0 (lossless) gives psnr = inf, which DuckDB stores fine
    let psnr = 20.0 * (f64::from(p999) / rmse).log10();

    Ok(BenchmarkResults {
        size,
        mse,
        rmse,
        psnr,
        reference,
        roundtripped,
    })
}

/// Per-frame result of a roundtrip
struct FrameResult {
    /// Encoded size in bytes
    size: usize,
    mse: f64,
    /// DDM metrics of the original frame
    reference: DdmMetrics,
    /// DDM metrics of the roundtripped frame
    roundtripped: DdmMetrics,
}

/// Encode and decode one frame, measuring size, mse and DDM metrics
fn roundtrip_frame(frame: &Array2<i32>, config: BenchmarkConfig) -> eyre::Result<FrameResult> {
    let mut owned_frame = frame.clone();
    // Remember the mean so it can be restored after decoding
    let mean = if config.dc_adjust {
        let mean = owned_frame.mean().ok_or(eyre::eyre!("Empty frame"))?;
        owned_frame.map_inplace(|x| *x -= mean);
        mean
    } else {
        0
    };

    let mut wavelet_pixels = WaveletPixelData::new(owned_frame);
    wavelet_pixels.dwt_mallat(config.wavelet_stages)?;
    let bitplane_pixels = BitplanePixelData::new(wavelet_pixels);

    // Encode. The writer is scoped so its borrow of `buf` ends before we read `buf`.
    let mut buf: Vec<u8> = vec![];
    let (dc_bitdepth, ac_bitdepth) = {
        let mut bitwriter = BitWriter::<_, BigEndian>::new(&mut buf);
        let depths = bitplane_pixels.encode(&mut bitwriter, config.bitplane_config)?;
        // Flush any partial trailing byte so the size is accurate
        bitwriter.byte_align()?;
        depths
    };
    let size = buf.len();
    let wavelet_pixels = WaveletPixelData::empty(frame.dim(), u8::try_from(config.wavelet_stages)?);
    let mut bitreader = BitReader::<_, BigEndian>::new(Cursor::new(buf.as_slice()));
    let bitplane_pixels = BitplanePixelData::decode(
        wavelet_pixels,
        &mut bitreader,
        config.bitplane_config,
        dc_bitdepth,
        ac_bitdepth,
    )?;

    // Undo the wavelet transform so we compare pixels, not coefficients
    let mut wavelet_pixels = bitplane_pixels.into_wavelet_data();
    wavelet_pixels.idwt_mallat();
    let mut roundtripped = wavelet_pixels.into_inner();

    // Undo the DC adjustment
    if config.dc_adjust {
        roundtripped.map_inplace(|x| *x += mean);
    }

    let mse = compare_frames(frame, &roundtripped);
    // Measured on the original (not DC-adjusted) frame and the restored roundtrip
    let reference = DdmMetrics::measure(frame)?;
    let roundtripped = DdmMetrics::measure(&roundtripped)?;

    Ok(FrameResult {
        size,
        mse,
        reference,
        roundtripped,
    })
}

/// Mean squared error between two equally-sized frames
fn compare_frames(frame: &Array2<i32>, roundtripped: &Array2<i32>) -> f64 {
    debug_assert_eq!(frame.dim(), roundtripped.dim());
    let sum_sq: f64 = frame
        .iter()
        .zip(roundtripped.iter())
        .map(|(&reference, &altered)| (f64::from(reference) - f64::from(altered)).powi(2))
        .sum();
    #[expect(
        clippy::cast_precision_loss,
        reason = "Frames should be around 100000 pixels"
    )]
    let n = frame.len() as f64;
    sum_sq / n
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
        [
            SignificantEncoding::UseMainContext,
            SignificantEncoding::Bypass
        ],
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
