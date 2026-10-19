//! Le Gall 5/3 lifting wavelet transform
//!
//! Based on mathematical definition in DOI: 10.1109/PACRIM.2001.953547
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use ndarray::{ArrayViewMut, ArrayViewMut1, Axis, Dimension};

/// Forward 1D DWT wavelet transform
///
/// DOI: 10.1109/PACRIM.2001.953547
#[expect(
    clippy::missing_panics_doc,
    reason = "panics on unreachable code paths"
)]
pub fn dwt_53(array: ArrayViewMut1<i32>) {
    if array.len() < 3 {
        return;
    }

    let len = array.len();
    let s_cap = len.div_ceil(2);
    let d_cap = len / 2;

    let mut d0 = Vec::with_capacity(d_cap);
    d0.extend(array.iter().skip(1).step_by(2).copied());
    let mut s0 = Vec::with_capacity(s_cap + 1);
    s0.extend(array.iter().step_by(2).copied());

    let s_len = s0.len();
    #[expect(clippy::unwrap_used, reason = "We've already checked the length")]
    s0.push(*s0.last().unwrap());

    let (mut s, mut d) = array.split_at(Axis(0), s_len);

    // Predict
    for ((d_n, d0_n), (s0_left, s0_right)) in d
        .iter_mut()
        .zip(&d0)
        .zip(s0.windows(2).map(|window| (window[0], window[1])))
    {
        *d_n = d0_n - ((s0_left + s0_right).div_euclid(2));
    }

    // Iterator with padding for boundary conditions
    #[expect(clippy::unwrap_used, reason = "We've already checked the length")]
    let update_pairs = core::iter::once((*d.first().unwrap(), *d.first().unwrap()))
        .chain(d.windows(2).into_iter().map(|w| (w[0], w[1])))
        .chain(core::iter::once((*d.last().unwrap(), *d.last().unwrap())))
        .take(s_len);

    // Update
    for ((s_n, s0_n), (d_left, d_right)) in s
        .iter_mut()
        .zip(s0.iter().copied().take(s_len))
        .zip(update_pairs)
    {
        *s_n = s0_n + (d_left + d_right + 2).div_euclid(4);
    }
}

/// Performs a forward 1D 5/3 DWT using caller-provided scratch buffers.
///
/// For an input of length `n >= 3`, the scratch buffers must satisfy:
///
/// - `d0_scratch.len() >= n / 2`
/// - `s0_scratch.len() >= n.div_ceil(2) + 1`
///
/// The extra element in `s0_scratch` provides padding for the boundary
/// condition. Buffers may be larger than the minimum required size.
///
/// Inputs shorter than 3 return without accessing either scratch buffer.
///
/// # Panics
///
/// Panics if either scratch buffer is too short for the input.
pub fn dwt_53_scratch(array: ArrayViewMut1<i32>, d0_scratch: &mut [i32], s0_scratch: &mut [i32]) {
    if array.len() < 3 {
        return;
    }

    let len = array.len();
    let s_cap = len.div_ceil(2);
    let d_cap = len / 2;

    assert!(d0_scratch.len() >= d_cap);
    assert!(s0_scratch.len() >= s_cap);

    for (x, y) in d0_scratch
        .iter_mut()
        .zip(array.iter().skip(1).step_by(2).copied())
    {
        *x = y;
    }
    for (x, y) in s0_scratch.iter_mut().zip(array.iter().step_by(2).copied()) {
        *x = y;
    }

    let s_len = s_cap;
    s0_scratch[s_cap] = s0_scratch[s_cap - 1];

    let (mut s, mut d) = array.split_at(Axis(0), s_len);

    // Predict
    for ((d_n, d0_n), (s0_left, s0_right)) in d
        .iter_mut()
        .zip(d0_scratch.iter())
        .zip(s0_scratch.windows(2).map(|window| (window[0], window[1])))
    {
        *d_n = *d0_n - ((s0_left + s0_right).div_euclid(2));
    }

    // Iterator with padding for boundary conditions
    #[expect(clippy::unwrap_used, reason = "We've already checked the length")]
    let update_pairs = core::iter::once((d[0], d[0]))
        .chain(d.windows(2).into_iter().map(|w| (w[0], w[1])))
        .chain(core::iter::once((*d.last().unwrap(), *d.last().unwrap())))
        .take(s_len);

    // Update
    for ((s_n, s0_n), (d_left, d_right)) in s
        .iter_mut()
        .zip(s0_scratch.iter().copied().take(s_len))
        .zip(update_pairs)
    {
        *s_n = s0_n + (d_left + d_right + 2).div_euclid(4);
    }
}

/// Inverse 1D DWT wavelet transform
#[expect(
    clippy::missing_panics_doc,
    reason = "panics on unreachable code paths"
)]
pub fn idwt_53(mut array: ArrayViewMut1<i32>) {
    if array.len() < 3 {
        return;
    }

    let len = array.len();
    let s_len = len.div_ceil(2);
    let d_len = len / 2;

    let (s, d) = array.view_mut().split_at(Axis(0), s_len);

    // Allocation 1: Restored scaling coefficients with padding capacity
    let mut s_restored = Vec::with_capacity(s_len + 1);

    #[expect(clippy::unwrap_used, reason = "We've already checked the length")]
    let update_pairs = core::iter::once((*d.first().unwrap(), *d.first().unwrap()))
        .chain(d.windows(2).into_iter().map(|w| (w[0], w[1])))
        .chain(core::iter::once((*d.last().unwrap(), *d.last().unwrap())))
        .take(s_len);

    // Undo update step
    s_restored.extend(
        s.iter()
            .zip(update_pairs)
            .map(|(&s_n, (d_left, d_right))| s_n - (d_left + d_right + 2).div_euclid(4)),
    );
    #[expect(clippy::unwrap_used, reason = "We've already checked the length")]
    s_restored.push(*s_restored.last().unwrap()); // Padding for undo predict

    // Allocation 2: Restored detail coefficients
    let d_restored: Vec<i32> = d
        .iter()
        .zip(s_restored.windows(2))
        .map(|(&d_n, s_window)| d_n + (s_window[1] + s_window[0]).div_euclid(2))
        .collect();

    // Interleave scaling and detail coefficients back into the original array
    let interleave_iter = s_restored
        .iter()
        .copied()
        .take(d_len)
        .zip(d_restored.iter().copied())
        .flat_map(|(s_val, d_val)| [s_val, d_val])
        .chain(s_restored.iter().copied().skip(d_len));

    for (arr_n, val) in array.iter_mut().zip(interleave_iter) {
        *arr_n = val;
    }
}

/// Performs an inverse 1D 5/3 DWT using caller-provided scratch buffers.
///
/// For an input of length `n >= 3`, the scratch buffers must satisfy:
///
/// - `d_scratch.len() >= n / 2`
/// - `s_scratch.len() >= n.div_ceil(2) + 1`
///
/// The extra element in `s_scratch` provides padding for the boundary
/// condition. Buffers may be larger than the minimum required size.
///
/// Inputs shorter than 3 return without accessing either scratch buffer.
///
/// # Panics
///
/// Panics if either scratch buffer is too short for the input.
pub fn idwt_53_scratch(
    mut array: ArrayViewMut1<i32>,
    d_scratch: &mut [i32],
    s_scratch: &mut [i32],
) {
    if array.len() < 3 {
        return;
    }

    let len = array.len();
    let s_len = len.div_ceil(2);
    let d_len = len / 2;

    assert!(s_scratch.len() > s_len);
    assert!(d_scratch.len() >= d_len);

    let (s, d) = array.view_mut().split_at(Axis(0), s_len);

    #[expect(clippy::unwrap_used, reason = "We've already checked the length")]
    let update_pairs = core::iter::once((*d.first().unwrap(), *d.first().unwrap()))
        .chain(d.windows(2).into_iter().map(|w| (w[0], w[1])))
        .chain(core::iter::once((*d.last().unwrap(), *d.last().unwrap())))
        .take(s_len);

    // Undo update step
    for (x, y) in s_scratch.iter_mut().zip(
        s.iter()
            .zip(update_pairs)
            .map(|(&s_n, (d_left, d_right))| s_n - (d_left + d_right + 2).div_euclid(4)),
    ) {
        *x = y;
    }
    s_scratch[s_len] = s_scratch[s_len - 1]; // Padding for undo predict

    // Allocation 2: Restored detail coefficients
    for (x, y) in d_scratch.iter_mut().zip(
        d.iter()
            .zip(s_scratch.windows(2))
            .map(|(&d_n, s_window)| d_n + (s_window[1] + s_window[0]).div_euclid(2)),
    ) {
        *x = y;
    }

    // Interleave scaling and detail coefficients back into the original array
    let interleave_iter = s_scratch
        .iter()
        .copied()
        .take(d_len)
        .zip(d_scratch.iter().copied())
        .flat_map(|(s_val, d_val)| [s_val, d_val])
        .chain(s_scratch.iter().copied().skip(d_len));

    for (arr_n, val) in array.iter_mut().zip(interleave_iter) {
        *arr_n = val;
    }
}

/// Performs a forward N-D 5/3 DWT across a provided slice of axes
///
/// The order of `axes` must be the same in `dwt_53_nd` and `idwt_53_nd` or the transformation may
/// not be reversible
pub fn dwt_53_nd<D: Dimension>(mut array: ArrayViewMut<i32, D>, axes: &[Axis]) {
    for &axis in axes {
        for lane in array.lanes_mut(axis) {
            dwt_53(lane);
        }
    }
}

/// Performs a forward N-D 5/3 DWT across the provided axes using
/// caller-provided scratch buffers.
///
/// For every selected axis, the scratch buffers must
/// satisfy:
///
/// - `d0_scratch.len() >= n / 2`
/// - `s0_scratch.len() >= n.div_ceil(2) + 1`
///
/// Size the buffers for the longest selected axis. The same buffers are
/// reused for every lane, so no additional allocation is required per lane.
/// Axes of length less than 3 require no scratch space.
///
/// # Panics
///
/// Panics if either scratch buffer is too short for any selected axis.
pub fn dwt_53_nd_scratch<D: Dimension>(
    mut array: ArrayViewMut<i32, D>,
    axes: &[Axis],
    d0_scratch: &mut [i32],
    s0_scratch: &mut [i32],
) {
    for &axis in axes {
        for lane in array.lanes_mut(axis) {
            dwt_53_scratch(lane, d0_scratch, s0_scratch);
        }
    }
}

/// Performs an inverse N-D 5/3 DWT across a provided slice of axes (in reverse order)
///
/// The order of `axes` must be the same in `dwt_53_nd` and `idwt_53_nd` or the transformation may
/// not be reversible
pub fn idwt_53_nd<D: Dimension>(mut array: ArrayViewMut<i32, D>, axes: &[Axis]) {
    for &axis in axes.iter().rev() {
        for lane in array.lanes_mut(axis) {
            idwt_53(lane);
        }
    }
}

/// Performs an inverse N-D 5/3 DWT across the provided axes in reverse
/// order, using caller-provided scratch buffers.
///
/// For every selected axis with length, the scratch buffers must
/// satisfy:
///
/// - `d_scratch.len() >= n / 2`
/// - `s_scratch.len() >= n.div_ceil(2) + 1`
///
/// Size the buffers for the longest selected axis. The same buffers are
/// reused for every lane, so no additional allocation is required per lane.
/// Axes of length less than 3 require no scratch space.
///
/// # Panics
///
/// Panics if either scratch buffer is too short for any selected axis.
pub fn idwt_53_nd_scratch<D: Dimension>(
    mut array: ArrayViewMut<i32, D>,
    axes: &[Axis],
    d_scratch: &mut [i32],
    s_scratch: &mut [i32],
) {
    for &axis in axes.iter().rev() {
        for lane in array.lanes_mut(axis) {
            idwt_53_scratch(lane, d_scratch, s_scratch);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Range;
    #[cfg(not(feature = "std"))]
    use std::{vec, vec::Vec};

    use ndarray::{Array1, ArrayD};
    use proptest::prelude::*;

    use super::*;

    fn arb_array_d_i32(
        num_dimensions: Range<usize>,
        axis_length: Range<usize>,
        inner_strategy: impl Strategy<Value = i32> + Clone,
    ) -> impl Strategy<Value = ArrayD<i32>> {
        num_dimensions.prop_flat_map(move |ndim| {
            prop::collection::vec(axis_length.clone(), ndim).prop_flat_map({
                let value = inner_strategy.clone();
                #[expect(
                    clippy::unwrap_used,
                    reason = "Vec is initinalized same size as ndarray"
                )]
                move |shape| {
                    let len: usize = shape.iter().product();
                    prop::collection::vec(value.clone(), len)
                        .prop_map(move |data| ArrayD::from_shape_vec(shape.clone(), data).unwrap())
                }
            })
        })
    }

    proptest! {
        #[test]
        fn dwt_53_roundtrip(data in prop::collection::vec(-100_000..100_000_i32, 0..128)) {
            let mut transformed = Array1::from_vec(data.clone());
            dwt_53(transformed.view_mut());
            idwt_53(transformed.view_mut());
            prop_assert_eq!(data, transformed.iter().copied().collect::<Vec<i32>>());
        }

        #[test]
        fn dwt_53_nd_roundtrip(
            data in arb_array_d_i32(2..5, 0..20, -100_000..100_000_i32),
            axes in prop::collection::vec(0..5usize, 1..5)
                .prop_filter("axes must be unique", |v| {
                    let set: std::collections::HashSet<_> = v.iter().collect();
                    set.len() == v.len()
                }
            )
        ) {
            let axes: Vec<_> = axes.iter().filter(|&&x| x < data.ndim()).map(|&x| Axis(x)).collect();
            let mut transformed = data.clone();
            dwt_53_nd(transformed.view_mut(), &axes);
            idwt_53_nd(transformed.view_mut(), &axes);
            prop_assert_eq!(data, transformed);
        }

        #[test]
        fn dwt_53_scratch_roundtrip(data in prop::collection::vec(-100_000..100_000_i32, 0..128)) {
            let mut transformed = Array1::from_vec(data.clone());
            let len = transformed.len();
            let s_cap = len.div_ceil(2) + 1;
            let d_cap = len / 2;
            let mut s_scratch = vec![0; s_cap];
            let mut d_scratch = vec![0; d_cap];
            dwt_53_scratch(transformed.view_mut(), &mut d_scratch, &mut s_scratch);
            idwt_53_scratch(transformed.view_mut(), &mut d_scratch, &mut s_scratch);
            prop_assert_eq!(data, transformed.iter().copied().collect::<Vec<i32>>());
        }

        #[test]
        fn dwt_53_nd_scratch_roundtrip(
            data in arb_array_d_i32(2..5, 0..20, -100_000..100_000_i32),
            axes in prop::collection::vec(0..5usize, 1..5)
                .prop_filter("axes must be unique", |v| {
                    let set: std::collections::HashSet<_> = v.iter().collect();
                    set.len() == v.len()
                }
            )
        ) {
            let axes: Vec<_> = axes.iter().filter(|&&x| x < data.ndim()).map(|&x| Axis(x)).collect();
            let mut transformed = data.clone();
            #[expect(clippy::unwrap_used, reason = "Min number of axes is 1")]
            let len = transformed.shape().iter().max().unwrap();
            let s_cap = len.div_ceil(2) + 1;
            let d_cap = len / 2;
            let mut s_scratch = vec![0; s_cap];
            let mut d_scratch = vec![0; d_cap];
            dwt_53_nd_scratch(transformed.view_mut(), &axes, &mut d_scratch, &mut s_scratch);
            idwt_53_nd_scratch(transformed.view_mut(), &axes, &mut d_scratch, &mut s_scratch);
            prop_assert_eq!(data, transformed);
        }
    }
}
