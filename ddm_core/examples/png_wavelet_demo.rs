//! Demo that applies a 1 to 3 stage Le Gall 5/3 Mallat transform to each RGB
//! channel of a PNG. It writes each result as its own PNG, plus one combined
//! image with the original and all three results side by side.

use std::{
    env,
    error::Error,
    path::{Path, PathBuf},
};

use ddm_core::coding::wavelet::{MallatSubband, WaveletPixelData, WaveletSubband};
use image::{GenericImage, Rgb, RgbImage};
use ndarray::{Array2, ArrayView2};

/// Detail coefficients are small and centred on zero, so they are amplified for display.
const DETAIL_GAIN: i32 = 4;
const MAX_STAGES: usize = 3;
const CHANNELS: usize = 3;

fn main() -> Result<(), Box<dyn Error>> {
    let input = env::args()
        .nth(1)
        .ok_or("usage: wavelet_demo <input.png>")?;
    let original = image::open(&input)?.to_rgb8();

    let mut channels = (0..CHANNELS)
        .map(|channel| WaveletPixelData::new(channel_pixels(&original, channel)))
        .collect::<Vec<_>>();

    let mut renders = Vec::with_capacity(MAX_STAGES);

    // Each pass transforms the LL band of the previous one, so the stages accumulate.
    for stages in 1..=MAX_STAGES {
        for channel in &mut channels {
            channel.dwt_mallat(1);
        }
        let rendered = render(&channels, stages);
        let output = output_path(&input, &format!("dwt{stages}"));
        rendered.save(&output)?;
        println!("wrote {}", output.display());
        renders.push(rendered);
    }

    let combined = side_by_side(original, renders)?;
    let output = output_path(&input, "all");
    combined.save(&output)?;
    println!("wrote {}", output.display());
    Ok(())
}

/// Extracts one colour channel of `image` as an array indexed `[row, col]`.
fn channel_pixels(image: &RgbImage, channel: usize) -> Array2<i32> {
    let (width, height) = image.dimensions();
    Array2::from_shape_fn((height as usize, width as usize), |(row, col)| {
        i32::from(image.get_pixel(col as u32, row as u32).0[channel])
    })
}

/// The whole coefficient array, which is the stage 0 LL band.
fn full_view(wavelet: &WaveletPixelData) -> ArrayView2<'_, i32> {
    wavelet
        .get(&MallatSubband {
            stage: 0,
            subband: WaveletSubband::LL,
        })
        .expect("stage 0 LL always exists")
}

/// `foo.png` -> `foo_<suffix>.png`, next to the input.
fn output_path(input: &str, suffix: &str) -> PathBuf {
    let stem = Path::new(input)
        .file_stem()
        .map_or_else(|| "image".into(), |s| s.to_string_lossy());
    Path::new(input).with_file_name(format!("{stem}_{suffix}.png"))
}

/// Maps a coefficient to 8-bit: the final LL band is shown as-is, and every
/// detail band is centred on mid-grey and amplified.
fn shade(value: i32) -> u8 {
    value.clamp(0, 255) as u8
}

/// Combines the transformed channels into one colour image.
fn render(channels: &[WaveletPixelData], stages: usize) -> RgbImage {
    let views: Vec<_> = channels.iter().map(full_view).collect();
    let (rows, cols) = views[0].dim();

    RgbImage::from_fn(cols as u32, rows as u32, |x, y| {
        let (row, col) = (y as usize, x as usize);
        let mut pixel = [0u8; CHANNELS];
        for (out, view) in pixel.iter_mut().zip(&views) {
            *out = shade(view[[row, col]]);
        }
        Rgb(pixel)
    })
}

/// Places `first` and then `rest` left to right in a single image.
///
/// All images must have the same dimensions.
fn side_by_side(first: RgbImage, rest: Vec<RgbImage>) -> Result<RgbImage, image::ImageError> {
    let (width, height) = first.dimensions();
    let count = u32::try_from(rest.len() + 1).expect("image count fits in u32");
    let mut combined = RgbImage::new(width * count, height);

    for (index, tile) in std::iter::once(first).chain(rest).enumerate() {
        let x = width * u32::try_from(index).expect("image count fits in u32");
        combined.copy_from(&tile, x, 0)?;
    }
    Ok(combined)
}
