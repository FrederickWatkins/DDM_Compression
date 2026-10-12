use std::io::{Read, Write};

use anyhow::Result;
use bitstream_io::{BitRead, BitWrite};
use cabac::{CabacReader, CabacWriter, h265::*};

const DDM_HEIGHT: usize = 52;
const DDM_WIDTH: usize = 20;

type DDMFrame = [[u16; DDM_WIDTH]; DDM_HEIGHT];
type SignedFrame = [[i32; DDM_WIDTH]; DDM_HEIGHT];

type WaveletSubband = [[i32; DDM_WIDTH / 2]; DDM_HEIGHT / 2];

struct WaveletSubbands {
    ll: WaveletSubband,
    lh: WaveletSubband,
    hl: WaveletSubband,
    hh: WaveletSubband,
}

pub fn encode<W: Write>(
    frames: &[DDMFrame],
    writer: &mut W,
    use_time_delta: bool,
    q_ll: i32,
    q_detail: i32,
) -> Result<()> {
    // 1. Convert u16 frames to signed i32 frames
    let mut signed_frames: Vec<SignedFrame> = frames
        .iter()
        .map(|frame| {
            let mut sf = [[0i32; DDM_WIDTH]; DDM_HEIGHT];
            for y in 0..DDM_HEIGHT {
                for x in 0..DDM_WIDTH {
                    sf[y][x] = frame[y][x] as i32;
                }
            }
            sf
        })
        .collect();

    if use_time_delta {
        time_delta_code_i32(&mut signed_frames);
    }

    // 2. Apply 2D DWT and Quantize
    let all_subbands: Vec<WaveletSubbands> = signed_frames
        .iter()
        .map(|sf| {
            let mut subbands = dwt53_2d(sf);
            quantize_subband(&mut subbands.ll, q_ll);
            quantize_subband(&mut subbands.lh, q_detail);
            quantize_subband(&mut subbands.hl, q_detail);
            quantize_subband(&mut subbands.hh, q_detail);
            subbands
        })
        .collect();

    // 4. Encode via CABAC
    let mut ctx = [H265Context::default(); 4];
    let mut writer = H265Writer::new(writer);
    for subbands in all_subbands {
        encode_subbands(subbands, &mut ctx, &mut writer)?;
    }
    writer.finish()?;
    Ok(())
}

pub fn decode<R: Read>(
    reader: &mut R,
    use_time_delta: bool,
    q_ll: i32,
    q_detail: i32,
) -> Result<Vec<DDMFrame>> {
    let mut reader = H265Reader::new(reader)?;
    let mut ctx = [H265Context::default(); 4];
    let mut all_subbands = Vec::new();

    while let Ok(mut subbands) = decode_subbands(&mut ctx, &mut reader) {
        // Dequantize subbands
        dequantize_subband(&mut subbands.ll, q_ll);
        dequantize_subband(&mut subbands.lh, q_detail);
        dequantize_subband(&mut subbands.hl, q_detail);
        dequantize_subband(&mut subbands.hh, q_detail);
        all_subbands.push(subbands);
    }

    // 6. Inverse 2D DWT to recover signed frames
    let mut signed_frames = Vec::with_capacity(all_subbands.len());
    for subbands in all_subbands {
        signed_frames.push(idwt53_2d(subbands));
    }

    if use_time_delta {
        inverse_time_delta_code_i32(&mut signed_frames);
    }

    // 7. Convert back to u16 DDMFrames
    let mut frames = Vec::with_capacity(signed_frames.len());
    for signed_frame in signed_frames {
        let mut frame = [[0u16; DDM_WIDTH]; DDM_HEIGHT];
        for y in 0..DDM_HEIGHT {
            for x in 0..DDM_WIDTH {
                frame[y][x] = signed_frame[y][x].clamp(0, u16::MAX as i32) as u16;
            }
        }
        frames.push(frame);
    }

    Ok(frames)
}

fn time_delta_code_ll(subbands_vec: &mut [WaveletSubbands]) {
    let mut prev_ll = [[0i32; DDM_WIDTH / 2]; DDM_HEIGHT / 2];
    for subbands in subbands_vec.iter_mut() {
        let current_ll = subbands.ll;
        for y in 0..(DDM_HEIGHT / 2) {
            for x in 0..(DDM_WIDTH / 2) {
                subbands.ll[y][x] = current_ll[y][x] - prev_ll[y][x];
            }
        }
        prev_ll = current_ll;
    }
}

fn inverse_time_delta_code_ll(subbands_vec: &mut [WaveletSubbands]) {
    let mut prev_ll = [[0i32; DDM_WIDTH / 2]; DDM_HEIGHT / 2];
    for subbands in subbands_vec.iter_mut() {
        let mut current_ll = subbands.ll;
        for y in 0..(DDM_HEIGHT / 2) {
            for x in 0..(DDM_WIDTH / 2) {
                current_ll[y][x] = current_ll[y][x] + prev_ll[y][x];
            }
        }
        subbands.ll = current_ll;
        prev_ll = current_ll;
    }
}

fn time_delta_code_i32(frames: &mut [SignedFrame]) {
    let mut prev_frame = [[0i32; DDM_WIDTH]; DDM_HEIGHT];
    for frame in frames.iter_mut() {
        let current = *frame;
        for y in 0..DDM_HEIGHT {
            for x in 0..DDM_WIDTH {
                let curr = frame[y][x];
                let prev = prev_frame[y][x];
                frame[y][x] = curr - prev; // Pure signed subtraction
            }
        }
        prev_frame = current;
    }
}

fn inverse_time_delta_code_i32(frames: &mut [SignedFrame]) {
    let mut prev_frame = [[0i32; DDM_WIDTH]; DDM_HEIGHT];
    for frame in frames.iter_mut() {
        for y in 0..DDM_HEIGHT {
            for x in 0..DDM_WIDTH {
                let prev = prev_frame[y][x];
                frame[y][x] = frame[y][x] + prev; // Pure signed addition
            }
        }
        prev_frame = *frame;
    }
}

fn dwt53(arr: &[i32]) -> (Vec<i32>, Vec<i32>) {
    let n = arr.len();
    if n == 0 {
        return (vec![], vec![]);
    }
    if n == 1 {
        return (vec![arr[0]], vec![]);
    }

    let mut even: Vec<i32> = arr.iter().step_by(2).copied().collect();
    let mut odd: Vec<i32> = arr.iter().skip(1).step_by(2).copied().collect();

    // Predict step
    for i in 0..odd.len() {
        let left = even[i];
        let right = if i + 1 < even.len() { even[i + 1] } else { even[i] };
        odd[i] -= (left + right) / 2;
    }

    // Update step
    for i in 0..even.len() {
        let left_d = if i > 0 { odd[i - 1] } else { odd[0] };
        let right_d = if i < odd.len() { odd[i] } else { odd[odd.len() - 1] };
        even[i] += (left_d + right_d + 2) / 4;
    }

    (even, odd)
}

fn idwt53(low: &[i32], high: &[i32]) -> Vec<i32> {
    let mut even = low.to_vec();
    let mut odd = high.to_vec();

    if even.is_empty() {
        return vec![];
    }
    if odd.is_empty() {
        return even;
    }

    // Undo update step
    for i in 0..even.len() {
        let left_d = if i > 0 { odd[i - 1] } else { odd[0] };
        let right_d = if i < odd.len() { odd[i] } else { odd[odd.len() - 1] };
        even[i] -= (left_d + right_d + 2) / 4;
    }

    // Undo predict step
    for i in 0..odd.len() {
        let left = even[i];
        let right = if i + 1 < even.len() { even[i + 1] } else { even[i] };
        odd[i] += (left + right) / 2;
    }

    // Interleave
    let mut res = Vec::with_capacity(even.len() + odd.len());
    let mut ev_it = even.into_iter();
    let mut od_it = odd.into_iter();
    loop {
        match ev_it.next() {
            Some(v) => res.push(v),
            None => break,
        }
        match od_it.next() {
            Some(v) => res.push(v),
            None => break,
        }
    }
    res
}

fn transpose<T: Copy>(matrix: &[Vec<T>]) -> Vec<Vec<T>> {
    let height = matrix.len();
    let width = matrix[0].len();
    let mut transposed = vec![vec![matrix[0][0]; height]; width];
    for r in 0..height {
        for c in 0..width {
            transposed[c][r] = matrix[r][c];
        }
    }
    transposed
}

fn transform_grid(grid: &[Vec<i32>]) -> (Vec<Vec<i32>>, Vec<Vec<i32>>) {
    let mut lows = Vec::with_capacity(grid.len());
    let mut highs = Vec::with_capacity(grid.len());
    for row in grid {
        let (l, h) = dwt53(row);
        lows.push(l);
        highs.push(h);
    }
    (lows, highs)
}

fn inverse_transform_grid(lows: &[Vec<i32>], highs: &[Vec<i32>]) -> Vec<Vec<i32>> {
    let mut grid = Vec::with_capacity(lows.len());
    for (l, h) in lows.iter().zip(highs.iter()) {
        grid.push(idwt53(l, h));
    }
    grid
}

fn to_subband(input: &[Vec<i32>]) -> WaveletSubband {
    let mut output = [[0i32; DDM_WIDTH / 2]; DDM_HEIGHT / 2];
    for (dst, src) in output.iter_mut().zip(input.iter()) {
        dst.copy_from_slice(src);
    }
    output
}

fn from_subband(subband: &WaveletSubband) -> Vec<Vec<i32>> {
    subband.iter().map(|row| row.to_vec()).collect()
}

fn dwt53_2d(frame: &SignedFrame) -> WaveletSubbands {
    let grid: Vec<Vec<i32>> = frame
        .iter()
        .map(|row| row.to_vec())
        .collect();

    // 1. Horizontal transform (on rows)
    let (low_h, high_h) = transform_grid(&grid);

    // 2. Vertical transform (on columns, handled cleanly by transposing first)
    let (ll_t, lh_t) = transform_grid(&transpose(&low_h));
    let (hl_t, hh_t) = transform_grid(&transpose(&high_h));

    WaveletSubbands {
        ll: to_subband(&transpose(&ll_t)),
        lh: to_subband(&transpose(&lh_t)),
        hl: to_subband(&transpose(&hl_t)),
        hh: to_subband(&transpose(&hh_t)),
    }
}

fn idwt53_2d(data: WaveletSubbands) -> SignedFrame {
    // 1. Inverse vertical transform (via transpose)
    let low_h = transpose(&inverse_transform_grid(
        &transpose(&from_subband(&data.ll)),
        &transpose(&from_subband(&data.lh)),
    ));
    let high_h = transpose(&inverse_transform_grid(
        &transpose(&from_subband(&data.hl)),
        &transpose(&from_subband(&data.hh)),
    ));

    // 2. Inverse horizontal transform (on rows)
    let grid = inverse_transform_grid(&low_h, &high_h);

    let mut frame = [[0i32; DDM_WIDTH]; DDM_HEIGHT];
    for (y, row) in grid.iter().enumerate() {
        for (x, &val) in row.iter().enumerate() {
            frame[y][x] = val;
        }
    }
    frame
}

fn encode_subbands<W: Write>(
    data: WaveletSubbands,
    ctx: &mut [H265Context; 4],
    writer: &mut H265Writer<W>,
) -> Result<()> {
    encode_subband(&data.hh, &mut ctx[0], writer)?;
    encode_subband(&data.lh, &mut ctx[1], writer)?;
    encode_subband(&data.hl, &mut ctx[2], writer)?;
    encode_subband(&data.ll, &mut ctx[3], writer)?;
    Ok(())
}

fn encode_subband<W: Write>(
    data: &WaveletSubband,
    ctx: &mut H265Context,
    writer: &mut H265Writer<W>,
) -> Result<()> {
    let max_val = data.iter().flatten().map(|&c| c.abs()).max().unwrap_or(0);
    let max_bit = if max_val == 0 {
        0
    } else {
        31 - max_val.leading_zeros() as i32
    };

    for i in (0..5).rev() {
        writer.put((max_bit >> i) & 1 == 1, ctx)?;
    }

    for row in data {
        for &coef in row {
            writer.put(coef < 0, ctx)?;
        }
    }

    for bit in (0..=max_bit).rev() {
        for row in data {
            for &coef in row {
                writer.put((coef.abs() >> bit) & 1 == 1, ctx)?;
            }
        }
    }
    Ok(())
}

fn decode_subbands<R: Read>(
    ctx: &mut [H265Context; 4],
    reader: &mut H265Reader<R>,
) -> Result<WaveletSubbands> {
    let hh = decode_subband(&mut ctx[0], reader)?;
    let lh = decode_subband(&mut ctx[1], reader)?;
    let hl = decode_subband(&mut ctx[2], reader)?;
    let ll = decode_subband(&mut ctx[3], reader)?;
    Ok(WaveletSubbands { ll, lh, hl, hh })
}

fn decode_subband<R: Read>(
    ctx: &mut H265Context,
    reader: &mut H265Reader<R>,
) -> Result<WaveletSubband> {
    let mut data = [[0i32; DDM_WIDTH / 2]; DDM_HEIGHT / 2];
    let sub_height = DDM_HEIGHT / 2;
    let sub_width = DDM_WIDTH / 2;
    let total_coeffs = sub_height * sub_width;

    let mut max_bit = 0i32;
    for i in (0..5).rev() {
        if reader.get(ctx)? {
            max_bit |= 1 << i;
        }
    }

    let mut negative = vec![false; total_coeffs];
    let mut idx = 0;
    for _ in 0..sub_height {
        for _ in 0..sub_width {
            negative[idx] = reader.get(ctx)?;
            idx += 1;
        }
    }

    let mut mags = vec![0i32; total_coeffs];
    for bit in (0..=max_bit).rev() {
        let mut idx = 0;
        for _ in 0..sub_height {
            for _ in 0..sub_width {
                let b = reader.get(ctx)?;
                if b {
                    mags[idx] |= 1 << bit;
                }
                idx += 1;
            }
        }
    }

    let mut idx = 0;
    for r in 0..sub_height {
        for c in 0..sub_width {
            let mut val = mags[idx];
            if negative[idx] {
                val = -val;
            }
            data[r][c] = val;
            idx += 1;
        }
    }

    Ok(data)
}

fn quantize_subband(subband: &mut WaveletSubband, q: i32) {
    if q <= 1 {
        return;
    }
    for row in subband.iter_mut() {
        for coef in row.iter_mut() {
            let sign = coef.signum();
            let abs_val = coef.abs();
            // Dead-zone truncation towards zero
            let quantized = abs_val / q;
            *coef = sign * quantized;
        }
    }
}

fn dequantize_subband(subband: &mut WaveletSubband, q: i32) {
    if q <= 1 {
        return;
    }
    for row in subband.iter_mut() {
        for coef in row.iter_mut() {
            *coef = *coef * q;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::ops::Range;

    fn arb_ddm_frame(values: impl Strategy<Value = u16>) -> impl Strategy<Value = DDMFrame> {
        prop::collection::vec(prop::collection::vec(values, DDM_WIDTH), DDM_HEIGHT).prop_map(|rows| {
            let rows: Vec<[u16; DDM_WIDTH]> = rows
                .into_iter()
                .map(|row| row.try_into().unwrap())
                .collect();
            rows.try_into().unwrap()
        })
    }

    fn arb_ddm_frame_series(
        length: Range<usize>,
        values: impl Strategy<Value = u16>,
    ) -> impl Strategy<Value = Vec<DDMFrame>> {
        prop::collection::vec(arb_ddm_frame(values), length)
    }

    proptest! {
        #[test]
        fn test_stage_time_delta_i32(frames in arb_ddm_frame_series(1..4, any::<u16>())) {
            let mut signed_frames: Vec<SignedFrame> = frames
                .iter()
                .map(|f| {
                    let mut sf = [[0i32; DDM_WIDTH]; DDM_HEIGHT];
                    for y in 0..DDM_HEIGHT {
                        for x in 0..DDM_WIDTH {
                            sf[y][x] = f[y][x] as i32;
                        }
                    }
                    sf
                })
                .collect();

            let original = signed_frames.clone();
            time_delta_code_i32(&mut signed_frames);
            inverse_time_delta_code_i32(&mut signed_frames);
            prop_assert_eq!(original, signed_frames);
        }

        #[test]
        fn test_stage_dwt53_1d(arr in prop::collection::vec((u16::MIN as i32)..(u16::MAX as i32), 2..30)) {
            let (low, high) = dwt53(&arr);
            let recovered = idwt53(&low, &high);
            prop_assert_eq!(arr, recovered);
        }

        #[test]
        fn test_stage_dwt53_2d(frame in arb_ddm_frame(any::<u16>())) {
            let signed_frame: SignedFrame = {
                let mut sf = [[0i32; DDM_WIDTH]; DDM_HEIGHT];
                for y in 0..DDM_HEIGHT {
                    for x in 0..DDM_WIDTH {
                        sf[y][x] = frame[y][x] as i32;
                    }
                }
                sf
            };
            let subbands = dwt53_2d(&signed_frame);
            let recovered = idwt53_2d(subbands);
            prop_assert_eq!(signed_frame, recovered);
        }

        #[test]
        fn test_full_codec_roundtrip_without_delta(original_frames in arb_ddm_frame_series(1..3, 0..1000u16)) {
            let mut encoded_data = Vec::new();
            encode(&original_frames, &mut encoded_data, false, 1, 1).unwrap();

            let decoded_frames = decode(&mut &encoded_data[..], false, 1, 1).unwrap();
            prop_assert_eq!(original_frames, decoded_frames);
        }

        #[test]
        fn test_full_codec_roundtrip_with_delta(original_frames in arb_ddm_frame_series(1..3, 0..1000u16)) {
            let mut encoded_data = Vec::new();
            encode(&original_frames, &mut encoded_data, true, 1, 1).unwrap();

            let decoded_frames = decode(&mut &encoded_data[..], true, 1, 1).unwrap();
            prop_assert_eq!(original_frames, decoded_frames);
        }
    }
}