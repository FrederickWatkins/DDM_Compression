pub fn dwt53(input: &[i32]) -> Vec<i32> {
    let n = input.len();
    if n < 3 {
        return input.iter().map(|&x| {x}).collect();
    }

    let low_len = (n + 1) / 2;
    let high_len = n / 2;

    let even: Vec<i32> = input.iter().step_by(2).map(|&x| x as i32).collect();
    let odd: Vec<i32> = input.iter().skip(1).step_by(2).map(|&x| x as i32).collect();

    let mut detail = vec![0i32; high_len];

    for i in 0..high_len {
        // Symmetric reflection: the final even sample reflects at the edge.
        let next_even = even[reflect_index(i as isize + 1, low_len)];
        detail[i] = odd[i] - (even[i] + next_even).div_euclid(2);
    }

    let mut smooth = vec![0i32; low_len];

    for i in 0..low_len {
        if high_len == 0 {
            smooth[i] = even[i];
        } else {
            let left = detail[reflect_index(i as isize - 1, high_len)];
            let right = detail[reflect_index(i as isize, high_len)];

            smooth[i] = even[i] + (left + right + 2).div_euclid(4);
        }
    }

    smooth.into_iter().chain(detail).collect()
}

pub fn idwt53(input: &[i32]) -> Vec<i32> {
    let n = input.len();

    if n < 3 {
        return input.iter().map(|&x| {x}).collect();
    }

    let low_len = (n + 1) / 2;
    let high_len = n / 2;

    let smooth = &input[..low_len];
    let detail = &input[low_len..];

    // Undo the update step using the same symmetric reflection.
    let mut even = vec![0i32; low_len];

    for i in 0..low_len {
        let left = detail[reflect_index(i as isize - 1, high_len)];
        let right = detail[reflect_index(i as isize, high_len)];

        even[i] = smooth[i] - (left + right + 2).div_euclid(4);
    }

    // Undo the predict step using symmetric reflection.
    let mut odd = vec![0i32; high_len];

    for i in 0..high_len {
        let next_even = even[reflect_index(i as isize + 1, low_len)];
        odd[i] = detail[i] + (even[i] + next_even).div_euclid(2);
    }

    // Interleave the even and odd samples.
    let mut output = Vec::with_capacity(n);

    for i in 0..low_len {
        output.push(even[i] as i32);

        if i < high_len {
            output.push(odd[i] as i32);
        }
    }

    output
}

/// Maps an index onto a finite array using symmetric edge reflection.
///
/// The edge samples are repeated: for length 4, the index sequence is
/// ..., 2, 1, 0, 0, 1, 2, 3, 3, 2, 1, ...
fn reflect_index(index: isize, len: usize) -> usize {
    assert!(len > 0);

    if len == 1 {
        return 0;
    }

    let period = 2 * len as isize;
    let index = index.rem_euclid(period);

    if index < len as isize {
        index as usize
    } else {
        (period - 1 - index) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn dwt53_round_trip(input in prop::collection::vec(0u16 as i32..=u16::MAX as i32, 0..256)) {
            let encoded = dwt53(&input);
            let decoded = idwt53(&encoded);

            prop_assert_eq!(decoded, input);
        }
    }
}