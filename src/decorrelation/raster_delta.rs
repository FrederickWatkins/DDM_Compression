use ndarray::{Array, Axis, Dimension};

pub fn raster_code<D: Dimension>(input: &Array<u16, D>) -> Array<i16, D> {
    assert!(input.ndim() >= 2, "input must have at least two dimensions");

    let ndim = input.ndim();
    let rows = input.shape()[ndim - 2];
    let cols = input.shape()[ndim - 1];

    let leading_shape = &input.shape()[..ndim - 2];
    let n = leading_shape.iter().product::<usize>();

    let input_frames = input.view().into_shape_with_order((n, rows, cols)).unwrap();

    let mut output = Array::<i16, D>::zeros(input.raw_dim());

    let mut output_frames = output
        .view_mut()
        .into_shape_with_order((n, rows, cols))
        .unwrap();

    for i in 0..n {
        let input_frame = input_frames.index_axis(Axis(0), i);
        let mut output_frame = output_frames.index_axis_mut(Axis(0), i);

        let mut prev_pixel: u16 = 0;

        for (pixel, encoded) in input_frame.iter().zip(output_frame.iter_mut()) {
            *encoded = pixel.wrapping_sub(prev_pixel) as i16;
            prev_pixel = *pixel;
        }
    }

    output
}

pub fn inverse_raster_code<D: Dimension>(input: &Array<i16, D>) -> Array<u16, D> {
    assert!(input.ndim() >= 2, "input must have at least two dimensions");

    let ndim = input.ndim();
    let rows = input.shape()[ndim - 2];
    let cols = input.shape()[ndim - 1];

    let leading_shape = &input.shape()[..ndim - 2];
    let n = leading_shape.iter().product::<usize>();

    let input_frames = input.view().into_shape_with_order((n, rows, cols)).unwrap();

    let mut output = Array::<u16, D>::zeros(input.raw_dim());

    let mut output_frames = output
        .view_mut()
        .into_shape_with_order((n, rows, cols))
        .unwrap();

    for i in 0..n {
        let input_frame = input_frames.index_axis(Axis(0), i);
        let mut output_frame = output_frames.index_axis_mut(Axis(0), i);

        let mut prev_pixel: u16 = 0;

        for (encoded, pixel) in input_frame.iter().zip(output_frame.iter_mut()) {
            let value = prev_pixel.wrapping_add(*encoded as u16);
            *pixel = value;
            prev_pixel = value;
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
        fn raster_code_round_trip(
            input in arb_nd_array(2, 4, 50)
        ) {
            let encoded = raster_code(&input);
            let decoded = inverse_raster_code(&encoded);

            prop_assert_eq!(decoded, input);
        }
    }
}
