/// Benchmark algorithm configurations on dataset
use std::io::Cursor;

use bitstream_io::{BigEndian, BitReader, BitWrite, BitWriter};
use color_eyre::eyre;
use ddm_core::coding::{
    BitplaneConfig, BitplanePixelData, WaveletPixelData,
    bitplane::{
        NeighbourEncoding, Quantisation, RefinementEncoding, SignEncoding, SignificantEncoding,
    },
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

/// Mean and worst case of a quantity over frames
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Summary {
    pub mean: f64,
    pub max: f64,
}

/// Accumulates a [`Summary`] frame by frame.
///
/// NaN is sticky: if any frame's value is NaN (an undefined error), both the mean and
/// the max come out NaN, so such a config can never slip under a budget by accident.
#[derive(Debug, Clone, Copy)]
struct Tracker {
    sum: f64,
    max: f64,
    count: usize,
}

impl Tracker {
    fn new() -> Self {
        Self {
            sum: 0.0,
            max: f64::NEG_INFINITY,
            count: 0,
        }
    }

    fn push(&mut self, value: f64) {
        self.sum += value;
        // f64::max would silently ignore a NaN, so propagate it explicitly
        self.max = if value.is_nan() || self.max.is_nan() {
            f64::NAN
        } else {
            self.max.max(value)
        };
        self.count += 1;
    }

    fn summary(self) -> Summary {
        #[expect(clippy::cast_precision_loss, reason = "Number of frames is small")]
        let n = self.count as f64;
        Summary {
            mean: self.sum / n,
            max: self.max,
        }
    }
}

/// Signal/noise measurements taken from a single DDM
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
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
}

/// Change in each DDM metric caused by compression, in dB, for a single frame
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct DdmError {
    pub peak_power_db: f64,
    pub noise_db: f64,
    pub snr_db: f64,
    pub signal_db: f64,
}

impl DdmError {
    fn between(reference: &DdmMetrics, roundtripped: &DdmMetrics) -> Self {
        Self {
            peak_power_db: db_error(reference.peak_power, roundtripped.peak_power),
            noise_db: db_error(reference.noise, roundtripped.noise),
            snr_db: db_error(reference.snr, roundtripped.snr),
            signal_db: db_error(reference.signal, roundtripped.signal),
        }
    }
}

/// `|10 * log10(roundtripped / reference)|`. All four DDM metrics are power-like (SNR is
/// a power ratio), so the same formula applies. Identical values give exactly 0. Otherwise
/// the error is NaN unless both are positive, so a sign flip or a zero can never look like
/// a small error.
#[allow(clippy::float_cmp, reason = "Exact equality is intended: lossless gives 0 dB")]
fn db_error(reference: f64, roundtripped: f64) -> f64 {
    if reference == roundtripped {
        0.0
    } else if reference > 0.0 && roundtripped > 0.0 {
        (10.0 * (roundtripped / reference).log10()).abs()
    } else {
        f64::NAN
    }
}

/// Mean and worst-frame error of each DDM metric, in dB
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct DdmErrorSummary {
    pub peak_power_db: Summary,
    pub noise_db: Summary,
    pub snr_db: Summary,
    pub signal_db: Summary,
}

#[derive(Debug, Clone, Copy)]
struct DdmErrorTracker {
    peak_power: Tracker,
    noise: Tracker,
    snr: Tracker,
    signal: Tracker,
}

impl DdmErrorTracker {
    fn new() -> Self {
        Self {
            peak_power: Tracker::new(),
            noise: Tracker::new(),
            snr: Tracker::new(),
            signal: Tracker::new(),
        }
    }

    fn push(&mut self, error: &DdmError) {
        self.peak_power.push(error.peak_power_db);
        self.noise.push(error.noise_db);
        self.snr.push(error.snr_db);
        self.signal.push(error.signal_db);
    }

    fn summary(self) -> DdmErrorSummary {
        DdmErrorSummary {
            peak_power_db: self.peak_power.summary(),
            noise_db: self.noise.summary(),
            snr_db: self.snr.summary(),
            signal_db: self.signal.summary(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct BenchmarkResults {
    /// Mean encoded size per frame, in bytes
    pub size: usize,
    /// MSE: mean over frames, and worst frame
    pub mse: Summary,
    /// Square root of the mean MSE
    pub rmse: f64,
    /// PSNR computed from the mean MSE
    pub psnr: f64,
    /// PSNR computed from the worst-frame MSE (the lowest PSNR of any frame)
    pub psnr_min: f64,
    /// DDM metric errors: mean over frames, and worst frame
    pub ddm_error: DdmErrorSummary,
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
    size                BIGINT,    -- mean bytes per frame
    mse                 DOUBLE,    -- mean over frames
    mse_max             DOUBLE,    -- worst frame
    rmse                DOUBLE,    -- sqrt of mean mse
    psnr                DOUBLE,    -- from mean mse
    psnr_min            DOUBLE,    -- from worst-frame mse
    -- DDM metric errors in dB, |10*log10(roundtripped/reference)| per frame
    err_peak_power_db_mean DOUBLE,
    err_peak_power_db_max  DOUBLE,
    err_noise_db_mean      DOUBLE,
    err_noise_db_max       DOUBLE,
    err_snr_db_mean        DOUBLE,
    err_snr_db_max         DOUBLE,
    err_signal_db_mean     DOUBLE,
    err_signal_db_max      DOUBLE
)";

/// Number of columns in the `benchmarks` table
const COLUMN_COUNT: usize = 25;

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

    // The reference DDM metrics don't depend on the config, so measure them once.
    // This also fails fast if the frames are too small for the noise box.
    let references = test_frames
        .iter()
        .map(DdmMetrics::measure)
        .collect::<eyre::Result<Vec<_>>>()?;

    conn.execute_batch(CREATE_TABLE)?;
    conn.execute_batch("BEGIN TRANSACTION")?;
    let mut insert = conn.prepare(&insert_sql())?;

    for config in all_configs() {
        let results = match benchmark(test_frames, &references, config, p999) {
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
        let ddm = results.ddm_error;
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
            results.mse.mean,
            results.mse.max,
            results.rmse,
            results.psnr,
            results.psnr_min,
            ddm.peak_power_db.mean,
            ddm.peak_power_db.max,
            ddm.noise_db.mean,
            ddm.noise_db.max,
            ddm.snr_db.mean,
            ddm.snr_db.max,
            ddm.signal_db.mean,
            ddm.signal_db.max,
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

/// PSNR in dB for a given MSE and peak value. A zero MSE gives infinity.
fn psnr(peak: i32, mse: f64) -> f64 {
    20.0 * (f64::from(peak) / mse.sqrt()).log10()
}

/// Benchmark one config over all frames. `references` are the pre-measured DDM metrics of
/// `test_frames`, in the same order.
fn benchmark(
    test_frames: &[Array2<i32>],
    references: &[DdmMetrics],
    config: BenchmarkConfig,
    p999: i32,
) -> eyre::Result<BenchmarkResults> {
    eyre::ensure!(!test_frames.is_empty(), "No test frames");
    eyre::ensure!(
        test_frames.len() == references.len(),
        "Got {} reference metrics for {} frames",
        references.len(),
        test_frames.len()
    );

    let mut size = 0;
    let mut mse = Tracker::new();
    let mut ddm_error = DdmErrorTracker::new();
    for (frame, reference) in test_frames.iter().zip(references) {
        let result = roundtrip_frame(frame, reference, config)?;
        size += result.size;
        mse.push(result.mse);
        ddm_error.push(&result.ddm_error);
    }
    let size = size / test_frames.len();
    let mse = mse.summary();

    Ok(BenchmarkResults {
        size,
        mse,
        rmse: mse.mean.sqrt(),
        psnr: psnr(p999, mse.mean),
        psnr_min: psnr(p999, mse.max),
        ddm_error: ddm_error.summary(),
    })
}

/// Per-frame result of a roundtrip
struct FrameResult {
    /// Encoded size in bytes
    size: usize,
    mse: f64,
    /// Error in each DDM metric versus the original frame
    ddm_error: DdmError,
}

/// Encode and decode one frame, measuring size, mse and DDM metric errors
fn roundtrip_frame(
    frame: &Array2<i32>,
    reference: &DdmMetrics,
    config: BenchmarkConfig,
) -> eyre::Result<FrameResult> {
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

    // Decode into a fresh, empty container so the decoder can't see any encoder data
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
    let measured = DdmMetrics::measure(&roundtripped)?;
    let ddm_error = DdmError::between(reference, &measured);

    Ok(FrameResult {
        size,
        mse,
        ddm_error,
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
