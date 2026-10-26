//! Wavelet coding libary

use ndarray::{Array2, ArrayView2, ArrayViewMut, ArrayViewMut1, ArrayViewMut2, Axis, Dimension};
use thiserror::Error;

/// Wavelet coding error types
#[derive(Error, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Error {
    /// Attempted to get/transform a stage higher than the current encoding stage
    #[error(
        "Requested stage {requested_stage} from pixel data, but current transformed stage is only {pixel_data_stage}"
    )]
    StageTooHigh {
        /// Stage requested by function
        requested_stage: usize,
        /// Current stage of pixel data
        pixel_data_stage: usize,
    },
    /// Attempted to transform when stage too low to perform transform
    #[error("Attempted to inverse transform data that is already raw")]
    InvertRaw,
    /// Input value was too large to safely transform
    #[error("Input value {input_value} is larger than the safe maximum magnitude {max_value}")]
    InputTooLarge {
        /// Input value from caller
        input_value: i32,
        /// Maximum value
        max_value: u32,
    },
}

type Result<T> = std::result::Result<T, Error>;

/// Wavelet subbands
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WaveletSubband {
    /// Low low subband
    LL,
    /// Low high subband
    LH,
    /// High low subband
    HL,
    /// High high subband
    HH,
}

impl WaveletSubband {
    /// Get 2d index of subband as tuple
    pub fn as_index(&self) -> (usize, usize) {
        match self {
            Self::LL => (0, 0),
            Self::LH => (0, 1),
            Self::HL => (1, 0),
            Self::HH => (1, 1),
        }
    }
}

/// Mallat subband
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MallatSubband {
    /// Decomposition stage
    pub stage: usize,
    /// Frequency subband
    pub subband: WaveletSubband,
}

/// Pixel data in wavelet coded format
#[derive(Debug, Clone)]
pub struct WaveletPixelData {
    stage: usize,
    data: Array2<i32>,
}

impl WaveletPixelData {
    /// Create from unencoded pixel data
    ///
    /// # Errors
    /// If input data contains a value with a larger magnitude than 2**28.
    pub fn new(data: Array2<i32>) -> Result<Self> {
        const MAX_INPUT: u32 = 1 << 28;
        let stage = 0;
        if let Some(&max_elem) = data.iter().max_by_key(|x| x.unsigned_abs())
            && max_elem.unsigned_abs() > MAX_INPUT
        {
            return Err(Error::InputTooLarge {
                input_value: max_elem, // This is the original signed value
                max_value: MAX_INPUT,
            });
        }
        Ok(Self { stage, data })
    }

    /// Apply a single stage dwt to `subband`
    #[expect(
        clippy::unwrap_used,
        clippy::missing_panics_doc,
        reason = "Functions guarantees invariant"
    )]
    pub fn dwt(&mut self, subband: WaveletSubband) {
        let mallat_subband = MallatSubband {
            stage: self.stage,
            subband,
        };
        dwt_53_nd(&mut self.get_mut(mallat_subband).unwrap());
        self.stage += 1;
    }

    /// Apply single stage idwt to `subband`
    ///
    /// # Errors
    /// If the pixel data isn't wavelet coded
    #[expect(
        clippy::unwrap_used,
        clippy::missing_panics_doc,
        reason = "Functions guarantees invariant"
    )]
    pub fn idwt(&mut self, subband: WaveletSubband) -> Result<()> {
        let Some(stage) = self.stage.checked_sub(1) else {
            return Err(Error::InvertRaw);
        };
        let mallat_subband = MallatSubband {
            stage, // Need to get previous subband for inverse
            subband,
        };
        idwt_53_nd(&mut self.get_mut(mallat_subband).unwrap());
        self.stage -= 1;
        Ok(())
    }

    /// Perform a multi-stage mallat transform on pixel data
    pub fn dwt_mallat(&mut self, stages: usize) {
        for _ in 0..stages {
            self.dwt(WaveletSubband::LL);
        }
    }

    /// Invert a multi-stage mallat transform on data
    pub fn idwt_mallat(&mut self) {
        while self.idwt(WaveletSubband::LL).is_ok() {}
    }

    /// Get subband
    pub fn get(&self, subband: &MallatSubband) -> Option<ArrayView2<'_, i32>> {
        if subband.stage > self.stage {
            return None;
        }

        let (rows, cols) = self.data.dim();
        let div = 1usize << subband.stage;
        let ll_rows = rows.div_ceil(div);
        let ll_cols = cols.div_ceil(div);

        let (x_index, y_index) = subband.subband.as_index();

        let x_start = x_index * ll_rows;
        let y_start = y_index * ll_cols;
        if x_start >= rows || y_start >= cols {
            return None;
        }
        // last chunk will be shorter if dim is not divisable by 2**subband.stage
        let x_end = (x_start + ll_rows).min(rows);
        let y_end = (y_start + ll_cols).min(cols);

        Some(self.data.slice(ndarray::s![x_start..x_end, y_start..y_end]))
    }

    /// Get subband mutably
    pub fn get_mut(&mut self, subband: MallatSubband) -> Option<ArrayViewMut2<'_, i32>> {
        if subband.stage > self.stage {
            return None;
        }

        let (rows, cols) = self.data.dim();
        let div = 1usize << subband.stage;
        let ll_rows = rows.div_ceil(div);
        let ll_cols = cols.div_ceil(div);

        let (x_index, y_index) = subband.subband.as_index();

        let x_start = x_index * ll_rows;
        let y_start = y_index * ll_cols;
        if x_start >= rows || y_start >= cols {
            return None;
        }
        // last chunk will be shorter if dim is not divisable by 2**subband.stage
        let x_end = (x_start + ll_rows).min(rows);
        let y_end = (y_start + ll_cols).min(cols);

        Some(
            self.data
                .slice_mut(ndarray::s![x_start..x_end, y_start..y_end]),
        )
    }
}

/// Le Gall 5/3 N dimensional transform
///
/// This function uses the Mallat layout, where low pass coefficient come at a lower index than
/// higher coefficients on all axes.
pub fn dwt_53_nd<D: Dimension>(input: &mut ArrayViewMut<i32, D>) {
    for axis in (0..input.ndim()).map(Axis) {
        for mut lane in input.lanes_mut(axis) {
            dwt_53_1d(&mut lane);
        }
    }
}

/// Le Gall 5/3 N dimensional transform
///
/// This function expects the mallat layout produced by [`dwt_53_nd`].
pub fn idwt_53_nd<D: Dimension>(input: &mut ArrayViewMut<i32, D>) {
    for axis in (0..input.ndim()).map(Axis).rev() {
        for mut lane in input.lanes_mut(axis) {
            idwt_53_1d(&mut lane);
        }
    }
}

/// Le Gall 5/3 1 dimensional transform.
///
/// Works in place and uses the Mallat layout: the `len.div_ceil(2)` lowpass
/// (approximation) coefficients come first, followed by the `len / 2` highpass
/// (detail) coefficients.
pub fn dwt_53_1d(input: &mut ArrayViewMut1<i32>) {
    if input.len() < 2 {
        return;
    }
    lift(input, 1, highpass_53);
    lift(input, 0, lowpass_53);
    deinterleave(input);
}

/// Inverse Le Gall 5/3 1 dimensional transform.
///
/// Expects the Mallat layout produced by [`dwt_53_1d`].
pub fn idwt_53_1d(input: &mut ArrayViewMut1<i32>) {
    if input.len() < 2 {
        return;
    }
    interleave(input);
    lift(input, 0, |s0, d| s0 - lowpass_53(0, d));
    lift(input, 1, |d0, s| d0 - highpass_53(0, s));
}

/// Floor of the average of two values, without intermediate overflow.
fn floor_midpoint(a: i32, b: i32) -> i32 {
    (a & b) + ((a ^ b) >> 1)
}

/// Le Gall 5/3 lifting scheme highpass filter: `d0 - floor((s_l + s_r) / 2)`
fn highpass_53(d0: i32, s0: [i32; 2]) -> i32 {
    d0 - floor_midpoint(s0[0], s0[1])
}

/// Le Gall 5/3 lifting scheme lowpass filter: `s0 + floor((d_l + d_r + 2) / 4)`
fn lowpass_53(s0: i32, d: [i32; 2]) -> i32 {
    s0 + ((floor_midpoint(d[0], d[1]) + 1) >> 1)
}

/// Applies one lifting step in place.
///
/// Every second sample starting at `start` is replaced by `filter(sample, neighbours)`,
/// where `neighbours` are the two adjacent samples (mirrored at the boundaries).
/// Neighbours always have the opposite parity of the updated samples, so they are
/// never modified during the step.
fn lift(input: &mut ArrayViewMut1<i32>, start: usize, filter: impl Fn(i32, [i32; 2]) -> i32) {
    // Indexing is necessary here to satisfy the borrow checker
    for i in (start..input.len()).step_by(2) {
        let neighbours = mirrored_neighbours(input, i);
        input[i] = filter(input[i], neighbours);
    }
}

/// Returns the samples left and right of `index` using whole-sample symmetric
/// extension, i.e. `x[-1] = x[1]` and `x[len] = x[len - 2]`.
#[expect(
    clippy::cast_possible_wrap,
    reason = "Don't wavelet transform 9 quintiliion values"
)]
fn mirrored_neighbours(input: &ArrayViewMut1<i32>, index: usize) -> [i32; 2] {
    let left = mirror_index(index as isize - 1, input.len());
    let right = mirror_index(index as isize + 1, input.len());
    [input[left], input[right]]
}

/// Reflects an index that is at most one step out of bounds back into `0..len`.
///
/// Requires `len >= 2`.
#[expect(
    clippy::cast_possible_wrap,
    reason = "Don't wavelet transform 9 quintiliion values"
)]
#[expect(clippy::cast_sign_loss, reason = "Checked manually")]
fn mirror_index(index: isize, len: usize) -> usize {
    let last = len as isize - 1;
    match index {
        i if i < 0 => (-i) as usize,
        i if i > last => (2 * last - i) as usize,
        i => i as usize,
    }
}

/// Reorders `[L0 H0 L1 H1 ...]` into the Mallat layout `[L0 L1 ... H0 H1 ...]`.
#[expect(
    clippy::indexing_slicing,
    reason = "Enumerate is derived from vec being indexed"
)]
fn deinterleave(input: &mut ArrayViewMut1<i32>) {
    let samples = input.to_vec();
    let split = samples.len().div_ceil(2);

    for (i, dst) in input.iter_mut().enumerate() {
        *dst = if i < split {
            samples[2 * i]
        } else {
            samples[2 * (i - split) + 1]
        };
    }
}

/// Reorders the Mallat layout `[L0 L1 ... H0 H1 ...]` back into `[L0 H0 L1 H1 ...]`.
#[expect(
    clippy::indexing_slicing,
    reason = "Enumerate is derived from vec being indexed"
)]
fn interleave(input: &mut ArrayViewMut1<i32>) {
    let samples = input.to_vec();
    let split = samples.len().div_ceil(2);

    for (i, dst) in input.iter_mut().enumerate() {
        *dst = if i % 2 == 0 {
            samples[i / 2]
        } else {
            samples[split + i / 2]
        };
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "Tests may panic"
)]
mod tests {
    use ndarray::{Array, Array2, ArrayD, Dimension, Ix1, Ix2, Ix3, IxDyn};
    use proptest::prelude::*;

    use super::{Error, WaveletPixelData, dwt_53_1d, dwt_53_nd, idwt_53_1d, idwt_53_nd};

    /// Magnitude of generated values; far from overflow so round trips are the only thing tested.
    const VALUE_LIMIT: i32 = 1 << 20;

    // ---------- generators ----------

    /// Generates an array of any dimensionality from a shape strategy and a value strategy.
    fn arb_array<D: Dimension>(
        shape: impl Strategy<Value = D>,
        values: impl Strategy<Value = i32> + Clone,
    ) -> impl Strategy<Value = Array<i32, D>> {
        shape.prop_flat_map(move |dim| {
            prop::collection::vec(values.clone(), dim.size())
                .prop_map(move |v| Array::from_shape_vec(dim.clone(), v).unwrap())
        })
    }

    fn small_values() -> impl Strategy<Value = i32> + Clone {
        -VALUE_LIMIT..=VALUE_LIMIT
    }

    fn shape_1d() -> impl Strategy<Value = Ix1> {
        (1usize..=64).prop_map(Ix1)
    }

    fn shape_2d() -> impl Strategy<Value = Ix2> {
        (1usize..=24, 1usize..=24).prop_map(|(r, c)| Ix2(r, c))
    }

    fn shape_3d() -> impl Strategy<Value = Ix3> {
        (1usize..=9, 1usize..=9, 1usize..=9).prop_map(|(a, b, c)| Ix3(a, b, c))
    }

    // ---------- round trips ----------

    fn assert_nd_roundtrip<D: Dimension>(original: Array<i32, D>) -> Result<(), TestCaseError> {
        let mut data = original.clone();
        dwt_53_nd(&mut data.view_mut());
        idwt_53_nd(&mut data.view_mut());
        prop_assert_eq!(data, original);
        Ok(())
    }

    proptest! {
        #[test]
        fn roundtrip_1d(original in arb_array(shape_1d(), small_values())) {
            let mut data = original.clone();
            dwt_53_1d(&mut data.view_mut());
            idwt_53_1d(&mut data.view_mut());
            prop_assert_eq!(data, original);
        }

        #[test]
        fn roundtrip_nd_2d(original in arb_array(shape_2d(), small_values())) {
            assert_nd_roundtrip(original)?;
        }

        #[test]
        fn roundtrip_nd_3d(original in arb_array(shape_3d(), small_values())) {
            assert_nd_roundtrip(original)?;
        }

        #[test]
        fn roundtrip_mallat(
            original in arb_array(shape_2d(), small_values()),
            stages in 0usize..=5,
        ) {
            let mut pixels = WaveletPixelData::new(original.clone())?;
            pixels.dwt_mallat(stages);
            pixels.idwt_mallat();
            prop_assert_eq!(pixels.data, original);
        }

        /// Random sign patterns at the maximum input magnitude. In debug builds
        /// (overflow checks on) any overflow in a multi-stage transform panics.
        #[test]
        fn extreme_inputs_survive_mallat(
            original in arb_array(
                shape_2d(),
                prop_oneof![Just(-(1 << 28)), Just(1 << 28)],
            ),
            stages in 0usize..=5,
        ) {
            let mut pixels = WaveletPixelData::new(original.clone()).unwrap();
            pixels.dwt_mallat(stages);
            pixels.idwt_mallat();
            prop_assert_eq!(pixels.data, original);
        }
    }

    // ---------- overflow limit ----------

    /// Largest amplitude for which a single-stage `ndim`-dimensional transform fits in `i32`.
    ///
    /// Each axis pass can double the peak magnitude, so the limit is `i32::MAX / 2^ndim`.
    fn safe_limit(ndim: u32) -> i32 {
        i32::MAX >> ndim
    }

    /// `±amplitude` checkerboard: every axis sees an alternating signal.
    fn checkerboard(shape: &[usize], amplitude: i32) -> ArrayD<i32> {
        ArrayD::from_shape_fn(IxDyn(shape), |idx| {
            if idx.slice().iter().sum::<usize>() % 2 == 0 {
                amplitude
            } else {
                -amplitude
            }
        })
    }

    fn peak(data: &ArrayD<i32>) -> i64 {
        data.iter()
            .map(|&v| i64::from(v.unsigned_abs()))
            .max()
            .unwrap()
    }

    #[test]
    fn alternating_extremes_hit_the_gain_bound_in_every_dimension() {
        for ndim in 1u32..=4 {
            let limit = safe_limit(ndim);
            let shape = vec![6; ndim as usize]; // even lengths keep the pattern alternating at the mirrored edges

            for amplitude in [limit, -limit] {
                let mut data = checkerboard(&shape, amplitude);
                dwt_53_nd(&mut data.view_mut());

                // The transform reaches exactly 2^ndim * amplitude...
                let expected = (1i64 << ndim) * i64::from(limit);
                assert_eq!(peak(&data), expected, "ndim {ndim}, amplitude {amplitude}");
                assert!(
                    expected <= i64::from(i32::MAX),
                    "ndim {ndim}: limit itself overflows"
                );

                // ...so one more unit of amplitude cannot fit.
                assert!(
                    (1i64 << ndim) * (i64::from(limit) + 1) > i64::from(i32::MAX),
                    "ndim {ndim}: limit is not tight"
                );
            }
        }
    }

    #[test]
    fn new_limit_is_safe_for_a_single_2d_stage() {
        let amplitude = 1 << 28;
        let mut pixels = WaveletPixelData::new(checkerboard_2d(amplitude)).unwrap();
        pixels.dwt_mallat(10);
        let peak = pixels
            .data
            .iter()
            .map(|&v| i64::from(v.unsigned_abs()))
            .max()
            .unwrap();
        assert_eq!(peak, 4 * i64::from(amplitude));
        assert!(peak <= i64::from(i32::MAX));
    }

    #[test]
    fn new_rejects_input_above_limit() {
        let too_big = (1 << 28) + 1;
        assert!(matches!(
            WaveletPixelData::new(checkerboard_2d(too_big)),
            Err(Error::InputTooLarge { .. })
        ));
        // i32::MIN has no positive counterpart, so make sure it is rejected too.
        let mut data = checkerboard_2d(1);
        data[[0, 0]] = i32::MIN;
        assert!(WaveletPixelData::new(data).is_err());
    }

    fn checkerboard_2d(amplitude: i32) -> Array2<i32> {
        checkerboard(&[64, 64], amplitude)
            .into_dimensionality()
            .unwrap()
    }
}
