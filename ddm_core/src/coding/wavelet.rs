//! Le Gall 5/3 lifting wavelet transform
//!
//! Based on mathematical definition in DOI: 10.1109/PACRIM.2001.953547
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use core::result::Result;

use ndarray::{ArrayViewMut, ArrayViewMut1, Axis, Dimension};

/// Failures reported by the integer wavelet transforms.
#[derive(Debug, thiserror::Error)]
pub enum WaveletError {
    /// A caller-provided scratch buffer is too small.
    #[error("wavelet scratch buffer is too small")]
    ScratchBufferTooSmall,
    /// A lifting step exceeded the representable coefficient range.
    #[error("wavelet coefficient overflow")]
    CoefficientOverflow,
    /// A selected axis is outside the array dimensions.
    #[error("wavelet axis is outside the array dimensions")]
    InvalidAxis,
    /// Temporary storage could not be reserved.
    #[error("wavelet temporary storage could not be reserved")]
    Allocation,
}

type WaveletResult<T = ()> = Result<T, WaveletError>;

fn checked_pair_sum(left: i32, right: i32) -> WaveletResult<i32> {
    left.checked_add(right)
        .ok_or(WaveletError::CoefficientOverflow)
}

fn checked_update(left: i32, right: i32) -> WaveletResult<i32> {
    checked_pair_sum(left, right)?
        .checked_add(2)
        .ok_or(WaveletError::CoefficientOverflow)
}

fn pair(window: &[i32]) -> Option<(i32, i32)> {
    let (left, rest) = window.split_first()?;
    rest.first().map(|right| (*left, *right))
}

/// Forward 1D DWT wavelet transform
///
/// DOI: 10.1109/PACRIM.2001.953547
///
/// # Errors
/// Returns [`WaveletError::CoefficientOverflow`] if a lifting step exceeds the coefficient range,
/// or [`WaveletError::Allocation`] if temporary storage cannot be reserved.
pub fn dwt_53(array: ArrayViewMut1<i32>) -> WaveletResult {
    if array.len() < 3 {
        return Ok(());
    }

    let len = array.len();
    let s_cap = len.div_ceil(2);
    let d_cap = len / 2;

    let mut d0 = Vec::new();
    d0.try_reserve_exact(d_cap)
        .map_err(|_| WaveletError::Allocation)?;
    d0.extend(array.iter().skip(1).step_by(2).copied());
    let s0_capacity = s_cap.checked_add(1).ok_or(WaveletError::Allocation)?;
    let mut s0 = Vec::new();
    s0.try_reserve_exact(s0_capacity)
        .map_err(|_| WaveletError::Allocation)?;
    s0.extend(array.iter().step_by(2).copied());

    let s_len = s0.len();
    let Some(last_scaling_value) = s0.last().copied() else {
        return Err(WaveletError::ScratchBufferTooSmall);
    };
    s0.push(last_scaling_value);

    let (mut s, mut d) = array.split_at(Axis(0), s_len);

    // Predict
    for ((d_n, d0_n), (s0_left, s0_right)) in
        d.iter_mut().zip(&d0).zip(s0.windows(2).filter_map(pair))
    {
        *d_n = d0_n
            .checked_sub(checked_pair_sum(s0_left, s0_right)?.div_euclid(2))
            .ok_or(WaveletError::CoefficientOverflow)?;
    }

    // Iterator with padding for boundary conditions
    let update_pairs = d
        .iter()
        .copied()
        .take(1)
        .map(|value| (value, value))
        .chain(d.iter().copied().zip(d.iter().copied().skip(1)))
        .chain(d.iter().copied().last().map(|value| (value, value)))
        .take(s_len);

    // Update
    for ((s_n, s0_n), (d_left, d_right)) in s
        .iter_mut()
        .zip(s0.iter().copied().take(s_len))
        .zip(update_pairs)
    {
        *s_n = s0_n
            .checked_add(checked_update(d_left, d_right)?.div_euclid(4))
            .ok_or(WaveletError::CoefficientOverflow)?;
    }
    Ok(())
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
/// # Errors
/// Returns [`WaveletError::ScratchBufferTooSmall`] if either scratch buffer is too short, or
/// [`WaveletError::CoefficientOverflow`] if a lifting step overflows.
pub fn dwt_53_scratch(
    array: ArrayViewMut1<i32>,
    d0_scratch: &mut [i32],
    s0_scratch: &mut [i32],
) -> WaveletResult {
    if array.len() < 3 {
        return Ok(());
    }

    let len = array.len();
    let s_cap = len.div_ceil(2);
    let d_cap = len / 2;

    if d0_scratch.len() < d_cap || s0_scratch.len() < s_cap.saturating_add(1) {
        return Err(WaveletError::ScratchBufferTooSmall);
    }

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
    let Some(padded_value) = s_cap
        .checked_sub(1)
        .and_then(|i| s0_scratch.get(i))
        .copied()
    else {
        return Err(WaveletError::ScratchBufferTooSmall);
    };
    let Some(padded_slot) = s0_scratch.get_mut(s_cap) else {
        return Err(WaveletError::ScratchBufferTooSmall);
    };
    *padded_slot = padded_value;

    let (mut s, mut d) = array.split_at(Axis(0), s_len);

    // Predict
    for ((d_n, d0_n), (s0_left, s0_right)) in d
        .iter_mut()
        .zip(d0_scratch.iter())
        .zip(s0_scratch.windows(2).filter_map(pair))
    {
        *d_n = d0_n
            .checked_sub(checked_pair_sum(s0_left, s0_right)?.div_euclid(2))
            .ok_or(WaveletError::CoefficientOverflow)?;
    }

    // Iterator with padding for boundary conditions
    let update_pairs = d
        .iter()
        .copied()
        .take(1)
        .map(|value| (value, value))
        .chain(d.iter().copied().zip(d.iter().copied().skip(1)))
        .chain(d.iter().copied().last().map(|value| (value, value)))
        .take(s_len);

    // Update
    for ((s_n, s0_n), (d_left, d_right)) in s
        .iter_mut()
        .zip(s0_scratch.iter().copied().take(s_len))
        .zip(update_pairs)
    {
        *s_n = s0_n
            .checked_add(checked_update(d_left, d_right)?.div_euclid(4))
            .ok_or(WaveletError::CoefficientOverflow)?;
    }
    Ok(())
}

/// Inverse 1D DWT wavelet transform.
///
/// # Errors
/// Returns [`WaveletError::CoefficientOverflow`] if an inverse lifting step overflows.
pub fn idwt_53(mut array: ArrayViewMut1<i32>) -> WaveletResult {
    if array.len() < 3 {
        return Ok(());
    }

    let len = array.len();
    let s_len = len.div_ceil(2);
    let d_len = len / 2;

    let (s, d) = array.view_mut().split_at(Axis(0), s_len);

    // Allocation 1: Restored scaling coefficients with padding capacity
    let restored_capacity = s_len.checked_add(1).ok_or(WaveletError::Allocation)?;
    let mut s_restored = Vec::new();
    s_restored
        .try_reserve_exact(restored_capacity)
        .map_err(|_| WaveletError::Allocation)?;

    let update_pairs = d
        .iter()
        .copied()
        .take(1)
        .map(|value| (value, value))
        .chain(d.iter().copied().zip(d.iter().copied().skip(1)))
        .chain(d.iter().copied().last().map(|value| (value, value)))
        .take(s_len);

    // Undo update step
    for (&s_n, (d_left, d_right)) in s.iter().zip(update_pairs) {
        let restored = s_n
            .checked_sub(checked_update(d_left, d_right)?.div_euclid(4))
            .ok_or(WaveletError::CoefficientOverflow)?;
        s_restored.push(restored);
    }
    let Some(last_scaling_value) = s_restored.last().copied() else {
        return Err(WaveletError::ScratchBufferTooSmall);
    };
    s_restored.push(last_scaling_value); // Padding for undo predict

    // Allocation 2: Restored detail coefficients
    let mut d_restored = Vec::new();
    d_restored
        .try_reserve_exact(d_len)
        .map_err(|_| WaveletError::Allocation)?;
    for (&d_n, s_window) in d.iter().zip(s_restored.windows(2)) {
        if let Some((left, right)) = pair(s_window) {
            d_restored.push(
                d_n.checked_add(checked_pair_sum(right, left)?.div_euclid(2))
                    .ok_or(WaveletError::CoefficientOverflow)?,
            );
        }
    }

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
    Ok(())
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
/// # Errors
/// Returns [`WaveletError::ScratchBufferTooSmall`] if either scratch buffer is too short, or
/// [`WaveletError::CoefficientOverflow`] if an inverse lifting step overflows.
pub fn idwt_53_scratch(
    mut array: ArrayViewMut1<i32>,
    d_scratch: &mut [i32],
    s_scratch: &mut [i32],
) -> WaveletResult {
    if array.len() < 3 {
        return Ok(());
    }

    let len = array.len();
    let s_len = len.div_ceil(2);
    let d_len = len / 2;

    if s_scratch.len() <= s_len || d_scratch.len() < d_len {
        return Err(WaveletError::ScratchBufferTooSmall);
    }

    let (s, d) = array.view_mut().split_at(Axis(0), s_len);

    let update_pairs = d
        .iter()
        .copied()
        .take(1)
        .map(|value| (value, value))
        .chain(d.iter().copied().zip(d.iter().copied().skip(1)))
        .chain(d.iter().copied().last().map(|value| (value, value)))
        .take(s_len);

    // Undo update step
    for (x, y) in
        s_scratch
            .iter_mut()
            .zip(s.iter().zip(update_pairs).map(|(&s_n, (d_left, d_right))| {
                s_n.checked_sub(checked_update(d_left, d_right)?.div_euclid(4))
                    .ok_or(WaveletError::CoefficientOverflow)
            }))
    {
        *x = y?;
    }
    let Some(padded_value) = s_len.checked_sub(1).and_then(|i| s_scratch.get(i)).copied() else {
        return Err(WaveletError::ScratchBufferTooSmall);
    };
    let Some(padded_slot) = s_scratch.get_mut(s_len) else {
        return Err(WaveletError::ScratchBufferTooSmall);
    };
    *padded_slot = padded_value; // Padding for undo predict

    // Allocation 2: Restored detail coefficients
    for (x, y) in d_scratch.iter_mut().zip(
        d.iter()
            .zip(s_scratch.windows(2))
            .filter_map(|(&d_n, s_window)| pair(s_window).map(|(left, right)| (d_n, left, right)))
            .map(|(d_n, left, right)| {
                d_n.checked_add(checked_pair_sum(right, left)?.div_euclid(2))
                    .ok_or(WaveletError::CoefficientOverflow)
            }),
    ) {
        *x = y?;
    }

    // Interleave scaling and detail coefficients back into the original array
    let interleave_iter = s_scratch
        .iter()
        .copied()
        .take(d_len)
        .zip(d_scratch.iter().copied().take(d_len))
        .flat_map(|(s_val, d_val)| [s_val, d_val])
        .chain(
            s_scratch
                .iter()
                .copied()
                .skip(d_len)
                .take(s_len.saturating_sub(d_len)),
        );

    for (arr_n, val) in array.iter_mut().zip(interleave_iter) {
        *arr_n = val;
    }
    Ok(())
}

/// Performs a forward N-D 5/3 DWT across a provided slice of axes
///
/// The order of `axes` must be the same in `dwt_53_nd` and `idwt_53_nd` or the transformation may
/// not be reversible
///
/// # Errors
/// Returns [`WaveletError::InvalidAxis`] for an invalid axis and propagates transform failures.
pub fn dwt_53_nd<D: Dimension>(mut array: ArrayViewMut<i32, D>, axes: &[Axis]) -> WaveletResult {
    if axes.iter().any(|axis| axis.0 >= array.ndim()) {
        return Err(WaveletError::InvalidAxis);
    }
    for &axis in axes {
        for lane in array.lanes_mut(axis) {
            dwt_53(lane)?;
        }
    }
    Ok(())
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
/// # Errors
/// Returns an error if an axis is invalid, a scratch buffer is too small, or a lifting step
/// overflows.
///
/// # Errors
/// Returns an error if an axis is invalid, a scratch buffer is too small, or a lifting step
/// overflows.
pub fn dwt_53_nd_scratch<D: Dimension>(
    mut array: ArrayViewMut<i32, D>,
    axes: &[Axis],
    d0_scratch: &mut [i32],
    s0_scratch: &mut [i32],
) -> WaveletResult {
    if axes.iter().any(|axis| axis.0 >= array.ndim()) {
        return Err(WaveletError::InvalidAxis);
    }
    for &axis in axes {
        for lane in array.lanes_mut(axis) {
            dwt_53_scratch(lane, d0_scratch, s0_scratch)?;
        }
    }
    Ok(())
}

/// Performs an inverse N-D 5/3 DWT across a provided slice of axes (in reverse order)
///
/// The order of `axes` must be the same in `dwt_53_nd` and `idwt_53_nd` or the transformation may
/// not be reversible
///
/// # Errors
/// Returns [`WaveletError::InvalidAxis`] for an invalid axis and propagates transform failures.
pub fn idwt_53_nd<D: Dimension>(mut array: ArrayViewMut<i32, D>, axes: &[Axis]) -> WaveletResult {
    if axes.iter().any(|axis| axis.0 >= array.ndim()) {
        return Err(WaveletError::InvalidAxis);
    }
    for &axis in axes.iter().rev() {
        for lane in array.lanes_mut(axis) {
            idwt_53(lane)?;
        }
    }
    Ok(())
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
/// # Errors
/// Returns an error if an axis is invalid, a scratch buffer is too small, or a lifting step
/// overflows.
pub fn idwt_53_nd_scratch<D: Dimension>(
    mut array: ArrayViewMut<i32, D>,
    axes: &[Axis],
    d_scratch: &mut [i32],
    s_scratch: &mut [i32],
) -> WaveletResult {
    if axes.iter().any(|axis| axis.0 >= array.ndim()) {
        return Err(WaveletError::InvalidAxis);
    }
    for &axis in axes.iter().rev() {
        for lane in array.lanes_mut(axis) {
            idwt_53_scratch(lane, d_scratch, s_scratch)?;
        }
    }
    Ok(())
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
                move |shape| {
                    let len = shape
                        .iter()
                        .try_fold(1_usize, |length, axis| length.checked_mul(*axis))
                        .unwrap_or(0);
                    prop::collection::vec(value.clone(), len)
                        .prop_filter_map("array shape must be representable", move |data| {
                            ArrayD::from_shape_vec(shape.clone(), data).ok()
                        })
                }
            })
        })
    }

    proptest! {
        #[test]
        fn dwt_53_roundtrip(data in prop::collection::vec(-100_000..100_000_i32, 0..128)) {
            let mut transformed = Array1::from_vec(data.clone());
            dwt_53(transformed.view_mut())?;
            idwt_53(transformed.view_mut())?;
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
            dwt_53_nd(transformed.view_mut(), &axes)?;
            idwt_53_nd(transformed.view_mut(), &axes)?;
            prop_assert_eq!(data, transformed);
        }

        #[test]
        fn dwt_53_scratch_roundtrip(data in prop::collection::vec(-100_000..100_000_i32, 0..128)) {
            let mut transformed = Array1::from_vec(data.clone());
            let len = transformed.len();
            let s_cap = len.div_ceil(2).saturating_add(1);
            let d_cap = len / 2;
            let mut s_scratch = vec![0; s_cap];
            let mut d_scratch = vec![0; d_cap];
            dwt_53_scratch(transformed.view_mut(), &mut d_scratch, &mut s_scratch)?;
            idwt_53_scratch(transformed.view_mut(), &mut d_scratch, &mut s_scratch)?;
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
            let len = transformed.shape().iter().copied().fold(0, usize::max);
            let s_cap = len.div_ceil(2).saturating_add(1);
            let d_cap = len / 2;
            let mut s_scratch = vec![0; s_cap];
            let mut d_scratch = vec![0; d_cap];
            dwt_53_nd_scratch(transformed.view_mut(), &axes, &mut d_scratch, &mut s_scratch)?;
            idwt_53_nd_scratch(transformed.view_mut(), &axes, &mut d_scratch, &mut s_scratch)?;
            prop_assert_eq!(data, transformed);
        }
    }
}
