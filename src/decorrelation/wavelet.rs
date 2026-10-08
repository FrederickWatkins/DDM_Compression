use ndarray::{ArrayBase, Axis, DataMut, Dimension};
use osclet::Osclet;

pub fn dwt53_axis<S, D>(data: &mut ArrayBase<S, D>, axis: Axis) -> Result<(), ndarray::ErrorKind>
where
    S: DataMut<Elem = i32>,
    D: Dimension,
{
    if axis.0 >= data.ndim() {
        return Err(ndarray::ErrorKind::OutOfBounds);
    }

    let len = data.len_of(axis);
    if len <= 3 {
        return Ok(());
    }

    let osclet = Osclet::make_cdf53_i32();

    // CDF 5/3 puts the extra sample in approx when length is odd
    let approx_len = len.div_ceil(2);

    let mut temp_buffer = vec![0i32; len];

    for mut lane in data.lanes_mut(axis) {
        let buf: Vec<i32> = lane.to_vec();
        {
            let (approx_buffer, detail_buffer) = temp_buffer.split_at_mut(approx_len);
            osclet
                .as_ref()
                .execute_forward(&buf, approx_buffer, detail_buffer).unwrap();
        }
        lane.assign(&ndarray::ArrayView1::from(&temp_buffer));
    }

    Ok(())
}

pub fn idwt53_axis<S, D>(data: &mut ArrayBase<S, D>, axis: Axis) -> Result<(), ndarray::ErrorKind>
where
    S: DataMut<Elem = i32>,
    D: Dimension,
{
    if axis.0 >= data.ndim() {
        return Err(ndarray::ErrorKind::OutOfBounds);
    }

    let len = data.len_of(axis);
    if len <= 3 {
        return Ok(());
    }

    let osclet = Osclet::make_cdf53_i32();

    // CDF 5/3 puts the extra sample in approx when length is odd
    let approx_len = len.div_ceil(2);

    let mut approx_buffer = vec![0i32; approx_len];
    let mut detail_buffer = vec![0i32; len - approx_len];

    for mut lane in data.lanes_mut(axis) {
        approx_buffer.copy_from_slice(&lane.to_vec()[..approx_len]);
        detail_buffer.copy_from_slice(&lane.to_vec()[approx_len..]);

        let mut buf = vec![0i32; len];

        osclet.as_ref().execute_inverse(&approx_buffer, &detail_buffer, &mut buf).unwrap();

        lane.assign(&ndarray::ArrayView1::from(&buf));
    }

    Ok(())
}

pub fn dwt53_nd<S, D>(data: &mut ArrayBase<S, D>) -> Result<(), ndarray::ErrorKind>
where
    S: DataMut<Elem = i32>,
    D: Dimension,
{
    let ndim = data.ndim();
    for i in 0..ndim {
        dwt53_axis(data, Axis(i))?;
    }
    Ok(())
}

pub fn idwt53_nd<S, D>(data: &mut ArrayBase<S, D>) -> Result<(), ndarray::ErrorKind>
where
    S: DataMut<Elem = i32>,
    D: Dimension,
{
    let ndim = data.ndim();
    for i in (0..ndim).rev() {
        idwt53_axis(data, Axis(i))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::{Array1, ArrayD, Axis, IxDyn};
    use proptest::prelude::*;

    fn arb_nd_array(
        min_ndim: usize,
        max_ndim: usize,
        max_len_per_axis: usize,
    ) -> impl Strategy<Value = ArrayD<i32>> {
        (min_ndim..=max_ndim).prop_flat_map(move |ndim| {
            let shape_strat = prop::collection::vec(1..=max_len_per_axis, ndim);

            shape_strat.prop_flat_map(|shape| {
                let total_elements: usize = shape.iter().product();
                let data_strat = prop::collection::vec(u16::MIN as i32..=u16::MAX as i32, total_elements);

                data_strat.prop_map(move |data| {
                    ArrayD::from_shape_vec(IxDyn(&shape), data)
                        .expect("Failed to create N-dimensional array from shape and data")
                })
            })
        })
    }

    proptest! {
        // Test arbitrary 1D arrays with lengths from 0 up to 500
        #[test]
        fn test_dwt53_roundtrip_1d(data in proptest::collection::vec(u16::MIN as i32..=u16::MAX as i32, 0..500)) {
            let original = Array1::from(data);
            let mut transformed = original.clone();

            // Perform forward transform along Axis(0)
            dwt53_axis(&mut transformed, Axis(0)).expect("Forward DWT failed");

            // Perform inverse transform along Axis(0)
            idwt53_axis(&mut transformed, Axis(0)).expect("Inverse DWT failed");

            // Verify that the signal is perfectly reconstructed
            prop_assert_eq!(transformed, original);
        }

        // Run tests across 1D to 5D arrays with varying dimensions and lengths
        #[test]
        fn test_dwt53_nd_roundtrip_up_to_5d(data in arb_nd_array(1, 5, 8)) {
            let original = data.clone();
            let mut transformed = original.clone();

            // Forward multi-dimensional transform (axes 0 -> N-1)
            dwt53_nd(&mut transformed).expect("Forward multi-dimensional DWT failed");

            // Inverse multi-dimensional transform (axes N-1 -> 0)
            idwt53_nd(&mut transformed).expect("Inverse multi-dimensional DWT failed");

            // Verify perfect reconstruction across all dimensions and non-uniform axis lengths
            prop_assert_eq!(transformed, original);
        }
    }
}
