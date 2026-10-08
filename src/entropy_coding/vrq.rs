use ndarray::{ArrayD, ArrayViewD, IxDyn};

/// Variable-rate quantise an arbitrary-dimensional i16 array.
///
/// Encoding:
///   1 byte:  0xxxxxxx
///   2 bytes: 1xxxxxxx 0xxxxxxx
///   3 bytes: 1xxxxxxx 1xxxxxxx xxxxxxxx
///
/// The payload is a two's-complement signed integer with respectively
/// 7, 14, or 22 bits.
pub fn vrq_encode(array: ArrayViewD<'_, i16>) -> Vec<u8> {
    let mut output = Vec::with_capacity(array.len() * 2);

    for &value in array.iter() {
        let value = value as i32;

        if (-64..=63).contains(&value) {
            // 7-bit two's complement.
            //
            // Masking is important for negative values:
            // -1 -> 0x7f
            // -64 -> 0x40
            output.push((value as u8) & 0x7f);
        } else if (-8192..=8191).contains(&value) {
            // 14-bit two's complement.
            let encoded = (value as u16) & 0x3fff;

            // First byte has the continuation flag set.
            output.push(0x80 | ((encoded >> 7) as u8 & 0x7f));

            // Second byte has no continuation flag.
            output.push((encoded & 0x7f) as u8);
        } else {
            // 22-bit two's complement.
            //
            // All i16 values fit here.
            let encoded = (value as u32) & 0x3f_ffff;

            // First byte: continuation flag + 7 data bits.
            output.push(0x80 | ((encoded >> 15) as u8 & 0x7f));

            // Second byte: continuation flag + 7 data bits.
            output.push(0x80 | ((encoded >> 8) as u8 & 0x7f));

            // Third byte: all 8 bits are data.
            output.push((encoded & 0xff) as u8);
        }
    }

    output
}

/// Decode a variable-rate quantised byte stream into an arbitrary-dimensional
/// i16 ndarray.
///
/// `shape` must describe the shape of the original array.
pub fn vrq_decode(bytes: &[u8], shape: &[usize]) -> Result<ArrayD<i16>, &'static str> {
    let element_count = shape
        .iter()
        .try_fold(1usize, |acc, &n| acc.checked_mul(n))
        .ok_or("array shape is too large")?;

    let mut values = Vec::with_capacity(element_count);
    let mut pos = 0;

    while pos < bytes.len() {
        if values.len() == element_count {
            return Err("too many encoded values");
        }

        let first = bytes[pos];
        pos += 1;

        if first & 0x80 == 0 {
            // -------------------------------------------------------------
            // 1 byte: 7-bit signed integer
            // -------------------------------------------------------------
            let raw = first & 0x7f;

            // Sign extend 7 -> 16 bits.
            let value = if raw & 0x40 != 0 {
                (raw as i16) | !0x7f
            } else {
                raw as i16
            };

            values.push(value);
        } else {
            if pos >= bytes.len() {
                return Err("truncated 2-byte value");
            }

            let second = bytes[pos];
            pos += 1;

            if second & 0x80 == 0 {
                // ---------------------------------------------------------
                // 2 bytes: 14-bit signed integer
                // ---------------------------------------------------------
                let raw =
                    (((first & 0x7f) as u16) << 7)
                    | (second & 0x7f) as u16;

                // Sign extend 14 -> 16 bits.
                let value = if raw & 0x2000 != 0 {
                    (raw as i16) | !0x3fff
                } else {
                    raw as i16
                };

                values.push(value);
            } else {
                // ---------------------------------------------------------
                // 3 bytes: 22-bit signed integer
                // ---------------------------------------------------------
                if pos >= bytes.len() {
                    return Err("truncated 3-byte value");
                }

                let third = bytes[pos];
                pos += 1;

                let raw =
                    (((first & 0x7f) as u32) << 15)
                    | (((second & 0x7f) as u32) << 8)
                    | third as u32;

                // Sign extend 22 -> 16 bits.
                //
                // Since the input is supposed to be i16, a valid encoded
                // 22-bit value must fit in i16.
                if raw & 0x20_0000 != 0 {
                    // Negative 22-bit value.
                    let extended = raw | 0xffc0_0000;
                    let value = extended as i32;

                    if !(i16::MIN as i32..=i16::MAX as i32).contains(&value) {
                        return Err("decoded 22-bit value does not fit in i16");
                    }

                    values.push(value as i16);
                } else {
                    values.push(raw as i16);
                }
            }
        }
    }

    if values.len() != element_count {
        return Err("not enough encoded values");
    }

    ArrayD::from_shape_vec(IxDyn(shape), values)
        .map_err(|_| "decoded data does not match requested shape")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::{ArrayD, IxDyn};
    use proptest::prelude::*;

    fn arb_nd_array() -> impl Strategy<Value = ArrayD<i16>> {
        // Generate 1–4 dimensions, each with a length of 0–8.
        (1usize..=4).prop_flat_map(|ndim| {
            prop::collection::vec(0usize..=8, ndim).prop_flat_map(|shape| {
                let len = shape.iter().product::<usize>();

                prop::collection::vec(any::<i16>(), len).prop_map(move |data| {
                    ArrayD::from_shape_vec(IxDyn(&shape), data).unwrap()
                })
            })
        })
    }

    proptest! {
        #[test]
        fn vrq_roundtrip(input in arb_nd_array()) {
            let encoded = vrq_encode(input.view());

            let decoded = vrq_decode(&encoded, input.shape())
                .expect("VRQ decoding failed");

            prop_assert_eq!(decoded.shape(), input.shape());
            prop_assert_eq!(decoded, input);
        }
    }
}