//! Le Gall 5/3 lifting wavelet transform
//!
//! Based on mathematical definition in DOI: 10.1109/PACRIM.2001.953547
use ndarray::{ArrayViewMut1, ArrayViewMutD, Axis};

/// Forward 1D DWT wavelet transform
///
/// DOI: 10.1109/PACRIM.2001.953547
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
    #[allow(clippy::unwrap_used, reason = "We've already checked the length")]
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
    #[allow(clippy::unwrap_used, reason = "We've already checked the length")]
    let update_pairs = std::iter::once((*d.first().unwrap(), *d.first().unwrap()))
        .chain(d.windows(2).into_iter().map(|w| (w[0], w[1])))
        .chain(std::iter::once((*d.last().unwrap(), *d.last().unwrap())))
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

/// Inverse 1D DWT wavelet transform
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

    #[allow(clippy::unwrap_used, reason = "We've already checked the length")]
    let update_pairs = std::iter::once((*d.first().unwrap(), *d.first().unwrap()))
        .chain(d.windows(2).into_iter().map(|w| (w[0], w[1])))
        .chain(std::iter::once((*d.last().unwrap(), *d.last().unwrap())))
        .take(s_len);

    // Undo update step
    s_restored.extend(
        s.iter()
            .zip(update_pairs)
            .map(|(&s_n, (d_left, d_right))| s_n - (d_left + d_right + 2).div_euclid(4)),
    );
    #[allow(clippy::unwrap_used, reason = "We've already checked the length")]
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

/// Performs a forward N-D 5/3 DWT across a provided slice of axes
///
/// The order of `axes` must be the same in `dwt_53_nd` and `idwt_53_nd` or the transformation may
/// not be reversible
pub fn dwt_53_nd(mut array: ArrayViewMutD<i32>, axes: &[Axis]) {
    for &axis in axes {
        for lane in array.lanes_mut(axis) {
            dwt_53(lane);
        }
    }
}

/// Performs an inverse N-D 5/3 DWT across a provided slice of axes (in reverse order)
///
/// The order of `axes` must be the same in `dwt_53_nd` and `idwt_53_nd` or the transformation may
/// not be reversible
pub fn idwt_53_nd(mut array: ArrayViewMutD<i32>, axes: &[Axis]) {
    for &axis in axes.iter().rev() {
        for lane in array.lanes_mut(axis) {
            idwt_53(lane);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::ops::Range;

    use super::*;
    use ndarray::{Array1, ArrayD};
    use proptest::prelude::*;

    fn arb_array_d_i32(
        num_dimensions: Range<usize>,
        axis_length: Range<usize>,
        inner_strategy: impl Strategy<Value = i32> + Clone,
    ) -> impl Strategy<Value = ArrayD<i32>> {
        num_dimensions.prop_flat_map(move |ndim| {
            prop::collection::vec(axis_length.clone(), ndim).prop_flat_map({
                let value = inner_strategy.clone();
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
        fn dwt_53_roundtrip(data in prop::collection::vec(-100_000..100_000_i32, 0..256)) {
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
    }
}
