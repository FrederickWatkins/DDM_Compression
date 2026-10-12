use std::path::PathBuf;

use anyhow::Result;
use ddm_compression::pixel_data::{decode, encode};
use ndarray::Array3;
use serde::de::DeserializeOwned;
use tracing::info;

const DDM_HEIGHT: usize = 52;
const DDM_WIDTH: usize = 20;
type DDMFrame = [[u16; DDM_WIDTH]; DDM_HEIGHT];

pub fn load_archive<T: DeserializeOwned>(path: PathBuf) -> anyhow::Result<T> {
    info!("Loading archive {:?}", path);
    let encoded = std::fs::read(&path)?;
    let decompressed = zstd::decode_all(encoded.as_slice())?;

    let (data, _) =
        bincode::serde::decode_from_slice::<T, _>(&decompressed, bincode::config::standard())?;

    info!("Loaded archive {:?}", path);
    Ok(data)
}

fn evaluate_error(original: &[DDMFrame], decoded: &[DDMFrame]) {
    assert_eq!(original.len(), decoded.len(), "Frame count mismatch");
    let mut total_squared_error = 0.0;
    let mut max_absolute_error = 0i32;
    let mut total_pixels = 0;
    let mut max_val = 0u16;

    for (orig_frame, dec_frame) in original.iter().zip(decoded.iter()) {
        for y in 0..DDM_HEIGHT {
            for x in 0..DDM_WIDTH {
                let o = orig_frame[y][x];
                let d = dec_frame[y][x];
                if o > max_val {
                    max_val = o;
                }
                let diff = (o as i32 - d as i32).abs();
                if diff > max_absolute_error {
                    max_absolute_error = diff;
                }
                total_squared_error += (diff as f64).powi(2);
                total_pixels += 1;
            }
        }
    }

    let mse = total_squared_error / total_pixels as f64;
    let rmse = mse.sqrt();
    let peak = max_val as f64;
    let psnr = if rmse == 0.0 {
        f64::INFINITY
    } else {
        20.0 * (peak / rmse).log10()
    };

    println!("=== Quantitative Evaluation ===");
    println!("MSE (Mean Squared Error):     {:.4}", mse);
    println!("RMSE (Root Mean Sq Error):    {:.4}", rmse);
    println!("Max Absolute Error:           {}", max_absolute_error);
    println!("PSNR (Peak Signal-to-Noise):  {:.2} dB", psnr);
    println!("===============================");
}

fn main() -> Result<()> {
    // Initialize tracing if desired (optional)
    tracing_subscriber::fmt::init();

    let path = PathBuf::from("data/channel0_f0_n32.bin.zst");

    // 1. Load the Array3<u16> from the archive
    let frames_array: Array3<u16> = load_archive(path)?;
    let shape = frames_array.shape();

    assert_eq!(
        shape.len(),
        3,
        "Expected a 3D array of shape (frames, height, width)"
    );
    assert_eq!(shape[1], 52, "Frame height must be 52");
    assert_eq!(shape[2], 20, "Frame width must be 20");

    let n_frames = shape[0];

    // 2. Convert Array3<u16> to Vec<[[u16; 20]; 52]>
    let mut ddm_frames = vec![];
    for view in frames_array.outer_iter() {
        let mut frame = [[0u16; 20]; 52];
        for y in 0..52 {
            for x in 0..20 {
                frame[y][x] = view[[y, x]];
            }
        }
        ddm_frames.push(frame);
    }

    let original_frames = ddm_frames.clone();
    let uncompressed_size = n_frames * 52 * 20 * std::mem::size_of::<u16>();

    // 3. Run Encode
    let mut encoded_data = Vec::new();
    encode(&mut ddm_frames, &mut encoded_data, 2, 8)?;
    let encoded_size = encoded_data.len();

    // 4. Print size differences and savings
    println!("=== Compression Statistics ===");
    println!("Uncompressed raw size: {} bytes", uncompressed_size);
    println!("Custom encoded size:   {} bytes", encoded_size);
    let diff = uncompressed_size as isize - encoded_size as isize;
    let savings_pct = (diff as f64 / uncompressed_size as f64) * 100.0;
    println!(
        "Size difference:       {} bytes (Saved {:.2}%)",
        diff, savings_pct
    );
    println!("===============================");

    // 5. Run Decode
    let decoded_frames = decode(&mut encoded_data.as_slice(), 2, 8)?;

    // 6. Run Quantitative Evaluation
    evaluate_error(&original_frames, &decoded_frames);
    evaluate_geophysical_features(&original_frames, &decoded_frames);

    println!("{:?}", original_frames[0][0]);
    println!("{:?}", decoded_frames[0][0]);

    Ok(())
}

fn evaluate_geophysical_features(original: &[DDMFrame], decoded: &[DDMFrame]) {
    let mut max_loc_errors = 0;
    let mut total_peak_power_err = 0.0;
    let mut total_les_err = 0.0;
    let n = original.len() as f64;

    for (orig_frame, dec_frame) in original.iter().zip(decoded.iter()) {
        // 1. Find Specular Peak (Max power coordinates)
        let (orig_peak_val, orig_peak_pos) = find_peak(orig_frame);
        let (dec_peak_val, dec_peak_pos) = find_peak(dec_frame);

        if orig_peak_pos != dec_peak_pos {
            max_loc_errors += 1;
        }

        let power_diff = (orig_peak_val as f64 - dec_peak_val as f64).abs();
        total_peak_power_err += power_diff / (orig_peak_val.max(1) as f64);

        // 2. Compute Leading Edge Slope (LES) proxy
        // LES is typically calculated along the delay axis at the Doppler slice of the peak
        let orig_les = compute_les(orig_frame, orig_peak_pos.1);
        let dec_les = compute_les(dec_frame, dec_peak_pos.1);
        let les_diff = (orig_les - dec_les).abs();
        total_les_err += les_diff / orig_les.max(1.0);
    }

    println!("=== DDM Geophysical Feature Impact ===");
    println!("Peak Coordinate Mismatches:   {} / {} frames", max_loc_errors, original.len());
    println!("Mean Peak Power Error:        {:.2}%", (total_peak_power_err / n) * 100.0);
    println!("Mean Leading Edge Slope (LES) Error: {:.2}%", (total_les_err / n) * 100.0);
    println!("========================================");
}

fn find_peak(frame: &DDMFrame) -> (u16, (usize, usize)) {
    let mut max_val = 0;
    let mut pos = (0, 0);
    for y in 0..DDM_HEIGHT {
        for x in 0..DDM_WIDTH {
            if frame[y][x] > max_val {
                max_val = frame[y][x];
                pos = (y, x);
            }
        }
    }
    (max_val, pos)
}

fn compute_les(frame: &DDMFrame, doppler_x: usize) -> f64 {
    // Simplified proxy for Leading Edge Slope along the delay axis (y) at the peak's Doppler column
    let mut leading_sum = 0.0;
    let mut count = 0;
    for y in 1..DDM_HEIGHT {
        let diff = (frame[y][doppler_x] as f64 - frame[y - 1][doppler_x] as f64).abs();
        leading_sum += diff;
        count += 1;
    }
    if count == 0 { 0.0 } else { leading_sum / count as f64 }
}