use ddm_app::L0bFileData;
use slint::{Image, ModelRc, Rgb8Pixel, SharedPixelBuffer, VecModel};

use crate::subsystem::file::CHANNEL_COUNT;

pub(crate) fn images_at<const WIDTH: usize, const HEIGHT: usize>(
    data: &L0bFileData<WIDTH, HEIGHT>,
    timestamp: u32,
) -> Vec<Image> {
    (0..CHANNEL_COUNT)
        .map(|channel| {
            data.frames
                .get((channel, timestamp))
                .and_then(Option::as_ref)
                .map(|frame| ddm_to_image(&frame.pixel_data()))
                .unwrap_or_default()
        })
        .collect()
}

pub(crate) fn model(images: Vec<Image>) -> ModelRc<Image> {
    ModelRc::new(VecModel::from(images))
}

pub(crate) fn empty_model() -> ModelRc<Image> {
    model(vec![Image::default(); usize::from(CHANNEL_COUNT)])
}

fn ddm_to_image(pixels: &ndarray::ArrayView2<'_, u16>) -> Image {
    let width = pixels.ncols().saturating_mul(2);
    let height = pixels.nrows();
    let Ok(width_u32) = u32::try_from(width) else {
        return Image::default();
    };
    let Ok(height_u32) = u32::try_from(height) else {
        return Image::default();
    };

    let Some(minimum) = pixels.iter().copied().min() else {
        return Image::default();
    };
    let maximum = pixels.iter().copied().max().unwrap_or(minimum);
    let range = f32::from(maximum - minimum);
    let log_range = range.ln_1p();

    let mut output = SharedPixelBuffer::<Rgb8Pixel>::new(width_u32, height_u32);
    let output_pixels = output.make_mut_slice();
    for ((row, column), value) in pixels.indexed_iter() {
        let scaled = if range == 0.0 {
            0.5
        } else {
            (f32::from(*value - minimum).ln_1p() / log_range).clamp(0.0, 1.0)
        };
        let color = inferno_color(scaled);
        let start = row
            .saturating_mul(width)
            .saturating_add(column.saturating_mul(2));
        if let Some(pixel) = output_pixels.get_mut(start) {
            *pixel = color;
        }
        if let Some(pixel) = output_pixels.get_mut(start.saturating_add(1)) {
            *pixel = color;
        }
    }

    Image::from_rgb8(output)
}

fn inferno_color(value: f32) -> Rgb8Pixel {
    const STOPS: [(u8, u8, u8); 9] = [
        (0, 0, 4),
        (31, 12, 72),
        (85, 15, 109),
        (136, 34, 106),
        (186, 54, 85),
        (227, 89, 51),
        (249, 140, 10),
        (249, 201, 50),
        (252, 255, 164),
    ];

    #[expect(
        clippy::cast_precision_loss,
        reason = "The palette contains a fixed, small number of color stops"
    )]
    let position = value.clamp(0.0, 1.0) * (STOPS.len().saturating_sub(1)) as f32;
    let last_segment = STOPS.len().saturating_sub(2);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "The interpolated position is clamped to the palette bounds"
    )]
    let segment = (position.floor() as usize).min(last_segment);
    #[expect(
        clippy::cast_precision_loss,
        reason = "The palette segment is within a fixed nine-stop table"
    )]
    let blend = position - segment as f32;
    let Some(&(start_red, start_green, start_blue)) = STOPS.get(segment) else {
        return Rgb8Pixel { r: 0, g: 0, b: 0 };
    };
    let Some(&(end_red, end_green, end_blue)) = STOPS.get(segment.saturating_add(1)) else {
        return Rgb8Pixel { r: 0, g: 0, b: 0 };
    };

    Rgb8Pixel {
        r: blend_channel(start_red, end_red, blend),
        g: blend_channel(start_green, end_green, blend),
        b: blend_channel(start_blue, end_blue, blend),
    }
}

fn blend_channel(start: u8, end: u8, blend: f32) -> u8 {
    let value = (f32::from(start) + (f32::from(end) - f32::from(start)) * blend)
        .round()
        .clamp(0.0, 255.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "The result is rounded and clamped to the u8 range before conversion"
    )]
    let channel = value as u8;
    channel
}
