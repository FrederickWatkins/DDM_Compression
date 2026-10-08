use ndarray::{Array, ArrayD, Dimension, IxDyn};

fn pack_bits(values: &[i32]) -> [u32; 32] {
    debug_assert!(values.len() <= 32, "Cannot pack more than 32 integers into bitplane");
    let mut planes = [0u32; 32];

    for (i, &value) in values.iter().enumerate() {
        let value = value as u32;

        for (bit, plane) in planes.iter_mut().enumerate() {
            *plane |= ((value >> bit) & 1) << i;
        }
    }

    planes
}

/// Transforms the last two dimensions of an i32 array into bitplanes.
///
/// Input:
///     [..., H, W]
///
/// Output:
///     [..., 32, ceil(H * W / 32)]
///
/// Each u32 contains 32 consecutive values packed along the least
/// significant bit positions of the word.
pub fn to_bitplanes<D: Dimension>(input: &Array<i32, D>) -> ArrayD<u32> {
    assert!(
        input.ndim() >= 2,
        "input must have at least two dimensions"
    );

    let shape = input.shape();
    let h = shape[shape.len() - 2];
    let w = shape[shape.len() - 1];

    let elements_per_plane = h * w;
    let blocks = elements_per_plane.div_ceil(32);

    let prefix = &shape[..shape.len() - 2];
    let outer = prefix.iter().product::<usize>();

    let mut output_shape = prefix.to_vec();
    output_shape.extend([32, blocks]);

    let mut output = ArrayD::<u32>::zeros(IxDyn(&output_shape));

    let input = input.as_standard_layout();
    let input_data = input.as_slice().unwrap();
    let output_data = output.as_slice_mut().unwrap();

    let plane_group_size = 32 * blocks;

    for outer_idx in 0..outer {
        let input_base = outer_idx * elements_per_plane;
        let output_base = outer_idx * plane_group_size;

        for block in 0..blocks {
            let start = block * 32;
            let end = (start + 32).min(elements_per_plane);

            let planes = pack_bits(&input_data[input_base + start..input_base + end]);

            for bit in 0..32 {
                output_data[output_base + bit * blocks + block] = planes[bit];
            }
        }
    }

    output
}

/// Restores an array from its bitplane representation.
///
/// Input:
///     [..., 32, ceil(H * W / 32)]
///
/// Output:
///     [..., H, W]
pub fn from_bitplanes(
    input: &ArrayD<u32>,
    original_h: usize,
    original_w: usize,
) -> ArrayD<i32> {
    assert!(
        input.ndim() >= 2,
        "input must have at least two dimensions"
    );

    let shape = input.shape();
    let num_bits = shape[shape.len() - 2];
    let blocks = shape[shape.len() - 1];

    assert_eq!(num_bits, 32, "bitplane dimension must be 32");

    let elements = original_h * original_w;
    let expected_blocks = elements.div_ceil(32);

    assert_eq!(
        blocks, expected_blocks,
        "bitplane block count does not match original dimensions"
    );

    let prefix = &shape[..shape.len() - 2];

    let mut output_shape = prefix.to_vec();
    output_shape.extend([original_h, original_w]);

    let mut output = ArrayD::<i32>::zeros(IxDyn(&output_shape));

    let input = input.as_standard_layout();
    let input_data = input.as_slice().unwrap();
    let output_data = output.as_slice_mut().unwrap();

    let plane_group_size = 32 * blocks;

    for outer_idx in 0..prefix.iter().product::<usize>() {
        let input_base = outer_idx * plane_group_size;
        let output_base = outer_idx * elements;

        for block in 0..blocks {
            let count = (elements - block * 32).min(32);

            for i in 0..count {
                let mut value = 0u32;

                for bit in 0..32 {
                    let plane = input_data[
                        input_base + bit * blocks + block
                    ];

                    value |= ((plane >> i) & 1) << bit;
                }

                output_data[output_base + block * 32 + i] = value as i32;
            }
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
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
                let data_strat = prop::collection::vec(any::<i32>(), total_elements);

                data_strat.prop_map(move |data| {
                    ArrayD::from_shape_vec(IxDyn(&shape), data)
                        .expect("Failed to create N-dimensional array from shape and data")
                })
            })
        })
    }

    proptest! {
        /// Tests that converting an arbitrary ndarray (with >= 2 dimensions) 
        /// to bitplanes and back restores the original array exactly.
        #[test]
        fn test_bitplanes_roundtrip(
            // Generates arrays with 2 to 4 dimensions, axis lengths up to 20
            arr in arb_nd_array(2, 4, 20)
        ) {
            let shape = arr.shape();
            let ndim = shape.len();
            
            // Extract the original H and W expected by `from_bitplanes`
            let original_h = shape[ndim - 2];
            let original_w = shape[ndim - 1];

            // 1. Transform to bitplanes
            let bitplanes = to_bitplanes(&arr);

            // 2. Verify bitplane dimensions: [..., 32, ceil((H * W) / 32)]
            let expected_compressed_len = (original_h * original_w + 31) / 32;
            let bp_shape = bitplanes.shape();
            let bp_ndim = bp_shape.len();
            
            prop_assert_eq!(bp_ndim, ndim);
            prop_assert_eq!(bp_shape[bp_ndim - 2], 32);
            prop_assert_eq!(bp_shape[bp_ndim - 1], expected_compressed_len);

            // 3. Roundtrip back to original shape and values
            let restored = from_bitplanes(&bitplanes, original_h, original_w);

            // 4. Assert exact equality
            prop_assert_eq!(restored, arr);
        }
    }
}
