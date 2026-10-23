use iced::widget::image;
use ndarray::{Array2, ArrayView2};

/// View image from rgb ndarray
/// 
/// # Errors
/// If too many pixels to fit in image handle
pub fn image_handle(pixels: &ArrayView2<[u8; 3]>) -> anyhow::Result<image::Handle> {
    let width = u32::try_from(pixels.ncols())?;
    let height = u32::try_from(pixels.nrows())?;
    let mut rgba = Vec::with_capacity(pixels.len().saturating_mul(4));

    for pixel in pixels {
        rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], u8::MAX]);
    }

    Ok(image::Handle::from_rgba(width, height, rgba))
}

/// Convert a `u16` image into an RGB heatmap using adaptive logarithmic scaling.
///
/// Each image is independently scaled between its minimum and maximum values.
pub fn ddm_to_image(
    pixels: &ArrayView2<'_, u16>,
) -> Array2<[u8; 3]> {
    let (height, width) = pixels.dim();
    let mut out = Array2::default((height, width * 2));

    let Some(min) = pixels.iter().copied().min() else {
        return out;
    };

    let max = pixels.iter().copied().max().unwrap_or(min);
    let range = f32::from(max - min);
    let log_range = range.ln_1p();

    for ((y, x), &value) in pixels.indexed_iter() {
        let t = if range == 0.0 {
            0.5
        } else {
            (f32::from(value - min).ln_1p() / log_range)
                .clamp(0.0, 1.0)
        };

        let colour = inferno_colour(t);
        let dst_x = x * 2;

        out[[y, dst_x]] = colour;
        out[[y, dst_x + 1]] = colour;
    }

    out
}

/// Map a normalised value in `0.0..=1.0` to an Inferno-inspired RGB colour.
fn inferno_colour(t: f32) -> [u8; 3] {
    const STOPS: [[u8; 3]; 9] = [
        [0,   0,   4],
        [31,  12,  72],
        [85,  15,  109],
        [136, 34,  106],
        [186, 54,  85],
        [227, 89,  51],
        [249, 140, 10],
        [249, 201, 50],
        [252, 255, 164],
    ];

    let position = t.clamp(0.0, 1.0) * (STOPS.len() - 1) as f32;
    #[expect(clippy::cast_sign_loss, reason="Blending guaranteed to be positive")]
    let index = (position.floor() as usize).min(STOPS.len() - 2);
    #[expect(clippy::cast_possible_truncation, reason="Blending guaranteed to be positive")]
    let blend = position - index as f32;

    #[expect(clippy::cast_sign_loss, clippy::cast_possible_truncation, clippy::indexing_slicing, reason="Blending guaranteed to be positive")]
    std::array::from_fn(|channel| {
        let a = f32::from(STOPS[index][channel]);
        let b = f32::from(STOPS[index + 1][channel]);

        (a + (b - a) * blend).round() as u8
    })
}