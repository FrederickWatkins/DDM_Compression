use std::path::PathBuf;

use ddm_compression::pixel_data::{encode, decode};
use anyhow::Result;
use ndarray::Array3;
use serde::de::DeserializeOwned;
use tracing::info;

pub fn load_archive<T: DeserializeOwned>(path: PathBuf) -> anyhow::Result<T> {
    info!("Loading archive {:?}", path);
    let encoded = std::fs::read(&path)?;
    let decompressed = zstd::decode_all(encoded.as_slice())?;

    let (data, _) =
        bincode::serde::decode_from_slice::<T, _>(&decompressed, bincode::config::standard())?;

    info!("Loaded archive {:?}", path);
    Ok(data)
}

fn main() -> Result<()> {
    // Initialize tracing if desired (optional)
    tracing_subscriber::fmt::init();

    let path = PathBuf::from("data/channel4_f0_n128.bin.zst");
    
    // 1. Load the Array3<u16> from the archive
    let frames_array: Array3<u16> = load_archive(path)?;
    let shape = frames_array.shape();
    
    assert_eq!(shape.len(), 3, "Expected a 3D array of shape (frames, height, width)");
    assert_eq!(shape[1], 52, "Frame height must be 52");
    assert_eq!(shape[2], 20, "Frame width must be 20");

    let n_frames = shape[0];

    // 2. Convert Array3<u16> to Vec<[[u16; 20]; 52]>
    let mut ddm_frames = Vec::with_capacity(n_frames);
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
    encode(&mut ddm_frames, &mut encoded_data)?;
    let encoded_size = encoded_data.len();

    // 4. Print size differences and savings
    println!("=== Compression Statistics ===");
    println!("Uncompressed raw size: {} bytes", uncompressed_size);
    println!("Custom encoded size:   {} bytes", encoded_size);
    let diff = uncompressed_size as isize - encoded_size as isize;
    let savings_pct = (diff as f64 / uncompressed_size as f64) * 100.0;
    println!("Size difference:       {} bytes (Saved {:.2}%)", diff, savings_pct);

    // 5. Run Decode
    let decoded_frames = decode(&mut encoded_data.as_slice())?;

    // 6. Verify Round-Trip
    assert_eq!(
        original_frames, decoded_frames,
        "Round-trip verification failed: Decoded frames do not match original frames!"
    );
    println!("==============================");
    println!("Round-trip verified successfully: Lossless reconstruction confirmed!");

    Ok(())
}