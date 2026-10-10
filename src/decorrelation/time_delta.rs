use ndarray::{Array, Axis, Dimension, RemoveAxis};

pub fn time_code<D: Dimension + RemoveAxis>(input: &Array<u16, D>) -> Array<i16, D> {
    assert!(
        input.ndim() >= 3,
        "input must have at least three dimensions"
    );

    let ndim = input.ndim();
    let time_axis = Axis(ndim - 3);
    let time_len = input.shape()[ndim - 3];

    let mut output = Array::<i16, D>::zeros(input.raw_dim());

    for t in 0..time_len {
        let current = input.index_axis(Axis(ndim - 3), t);
        let mut encoded = output.index_axis_mut(time_axis, t);

        if t == 0 {
            // First frame is encoded absolutely.
            for (src, dst) in current.iter().zip(encoded.iter_mut()) {
                *dst = *src as i16;
            }
        } else {
            let previous = input.index_axis(time_axis, t - 1);

            for ((&curr, &prev), dst) in current.iter().zip(previous.iter()).zip(encoded.iter_mut())
            {
                *dst = curr.wrapping_sub(prev) as i16;
            }
        }
    }

    output
}

pub fn inverse_time_code<D: Dimension + RemoveAxis>(input: &Array<i16, D>) -> Array<u16, D> {
    assert!(
        input.ndim() >= 3,
        "input must have at least three dimensions"
    );

    let ndim = input.ndim();
    let time_len = input.shape()[ndim - 3];
    let time_axis = Axis(ndim - 3);

    let mut output = Array::<u16, D>::zeros(input.raw_dim());

    for t in 0..time_len {
        let encoded = input.index_axis(time_axis, t);

        if t == 0 {
            let mut decoded = output.index_axis_mut(time_axis, t);

            for (&src, dst) in encoded.iter().zip(decoded.iter_mut()) {
                *dst = src as u16;
            }
        } else {
            // Copy the previous decoded frame before mutably borrowing output.
            let previous = output.index_axis(time_axis, t - 1).to_owned();
            let mut decoded = output.index_axis_mut(time_axis, t);

            for ((&diff, &prev), dst) in
                encoded.iter().zip(previous.iter()).zip(decoded.iter_mut())
            {
                *dst = prev.wrapping_add(diff as u16);
            }
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::{ArrayD, IxDyn};
    use proptest::prelude::*;

    fn arb_nd_array(
        min_ndim: usize,
        max_ndim: usize,
        max_len_per_axis: usize,
    ) -> impl Strategy<Value = ArrayD<u16>> {
        (min_ndim..=max_ndim).prop_flat_map(move |ndim| {
            let shape_strat = prop::collection::vec(1..=max_len_per_axis, ndim);

            shape_strat.prop_flat_map(|shape| {
                let total_elements: usize = shape.iter().product();
                let data_strat =
                    prop::collection::vec(u16::MIN..=u16::MAX, total_elements);

                data_strat.prop_map(move |data| {
                    ArrayD::from_shape_vec(IxDyn(&shape), data)
                        .expect("Failed to create N-dimensional array from shape and data")
                })
            })
        })
    }

    proptest! {
        #[test]
        fn time_code_round_trip(
            input in arb_nd_array(3, 5, 4)
        ) {
            let input = input.mapv(|x| x as u16);

            let encoded = time_code(&input);
            let decoded = inverse_time_code(&encoded);

            prop_assert_eq!(decoded, input);
        }
    }
}
