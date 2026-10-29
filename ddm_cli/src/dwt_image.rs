use std::{
    fs::{self, File},
    io::BufWriter,
    path::PathBuf,
};

use color_eyre::eyre;
use ddm_core::coding::{
    WaveletPixelData,
    wavelet::{MallatSubband, WaveletSubband, dwt_53_1d},
};
use image::{
    ExtendedColorType, ImageEncoder, Rgb, RgbImage,
    codecs::png::{CompressionType, FilterType, PngEncoder},
};
use ndarray::{Array2, ArrayView2, Axis};

pub const CHANNELS: usize = 3;
pub const STAGES: usize = 3;
pub const DEFAULT_DUMP_EVERY: usize = 16;

/// Extracts one colour channel as an `[row, col]` array.
#[expect(
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    reason = "Don't care if it breaks tbh"
)]
pub fn channel_pixels(image: &RgbImage, channel: usize) -> Array2<i32> {
    let (width, height) = image.dimensions();
    Array2::from_shape_fn((height as usize, width as usize), |(row, col)| {
        i32::from(image.get_pixel(col as u32, row as u32).0[channel])
    })
}

/// Same transform as `dwt_mallat`, but runs every channel one lane at a time
/// and calls `after_lane` after each step.
///
/// All channels must have identical dimensions.
pub fn dwt_mallat_lockstep(
    channels: &mut [WaveletPixelData],
    stages: usize,
    mut after_lane: impl FnMut(&[WaveletPixelData]) -> eyre::Result<()>,
) -> eyre::Result<()> {
    for stage in 0..stages {
        let region = MallatSubband {
            stage: u8::try_from(stage)?,
            subband: WaveletSubband::LL,
        };

        for lane_axis in [Axis(0), Axis(1)] {
            // Lanes running along `lane_axis` are indexed along the other axis.
            let index_axis = Axis(1 - lane_axis.index());
            let n_lanes = channels
                .first()
                .ok_or(eyre::eyre!("Channels empty"))?
                .get(region)
                .ok_or(eyre::eyre!("Channels empty"))?
                .len_of(index_axis);

            for i in 0..n_lanes {
                for channel in channels.iter_mut() {
                    let mut view = channel
                        .get_mut(region)
                        .ok_or(eyre::eyre!("Subband empty"))?;
                    dwt_53_1d(&mut view.index_axis_mut(index_axis, i))?;
                }
                after_lane(channels)?;
            }
        }
    }
    Ok(())
}

/// Writes every `every`th lane step as a numbered PNG (`dwt_0.png`, `dwt_1.png`, ...).
/// File numbers are contiguous, as ffmpeg's `%d` pattern requires.
pub struct FrameWriter {
    dir: PathBuf,
    every: usize,
    lanes_seen: usize,
    pub files_written: usize,
}

impl FrameWriter {
    pub fn new(dir: &str, every: usize) -> eyre::Result<Self> {
        eyre::ensure!(every > 0, eyre::eyre!("dump_every must be at least 1"));
        fs::create_dir_all(dir)?;
        Ok(Self {
            dir: dir.into(),
            every,
            lanes_seen: 0,
            files_written: 0,
        })
    }

    pub fn on_lane(&mut self, channels: &[WaveletPixelData]) -> eyre::Result<()> {
        self.lanes_seen += 1;
        if self.lanes_seen.is_multiple_of(self.every) {
            self.dump(channels)?;
        }
        Ok(())
    }

    pub fn dump(&mut self, channels: &[WaveletPixelData]) -> eyre::Result<()> {
        let path = self.dir.join(format!("dwt_{}.png", self.files_written));
        save_png_fast(&render(channels)?, &path)?;
        self.files_written += 1;
        Ok(())
    }
}

/// The whole coefficient array, which is the stage 0 LL band.
fn full_view(wavelet: &WaveletPixelData) -> eyre::Result<ArrayView2<'_, i32>> {
    wavelet
        .get(MallatSubband {
            stage: 0,
            subband: WaveletSubband::LL,
        })
        .ok_or(eyre::eyre!("Pixel data empty"))
}

/// Maps a coefficient to 8 bits.
#[expect(clippy::cast_sign_loss, reason = "Clamped manually")]
fn shade(value: i32) -> u8 {
    value.clamp(0, 255) as u8
}

/// Composites the transformed channels back into one RGB image.
fn render(channels: &[WaveletPixelData]) -> eyre::Result<RgbImage> {
    let views = channels
        .iter()
        .map(full_view)
        .collect::<eyre::Result<Vec<_>>>()?;
    let (rows, cols) = views.first().ok_or(eyre::eyre!("Views empty"))?.dim();

    Ok(RgbImage::from_fn(
        u32::try_from(cols)?,
        u32::try_from(rows)?,
        |x, y| {
            let idx = [y as usize, x as usize];
            let mut pixel = [0u8; CHANNELS];
            for (out, view) in pixel.iter_mut().zip(&views) {
                *out = shade(view[idx]);
            }
            Rgb(pixel)
        },
    ))
}

/// PNG encoding dominates runtime on big images, so use the fast settings.
fn save_png_fast(img: &RgbImage, path: &PathBuf) -> eyre::Result<()> {
    let writer = BufWriter::new(File::create(path)?);
    PngEncoder::new_with_quality(writer, CompressionType::Fast, FilterType::Sub).write_image(
        img.as_raw(),
        img.width(),
        img.height(),
        ExtendedColorType::Rgb8,
    )?;
    Ok(())
}

fn dwt_53_2d_dump(
    wavelets: &mut WaveletPixelData,
    stage: usize,
    counter: &mut usize,
) -> eyre::Result<()> {
    let region = MallatSubband {
        stage: u8::try_from(stage)?,
        subband: WaveletSubband::LL,
    };
    let full = MallatSubband {
        stage: 0,
        subband: WaveletSubband::LL,
    };

    for axis in 0..2 {
        let other = Axis(1 - axis);
        let n_lanes = wavelets
            .get(region)
            .ok_or(eyre::eyre!("Wavelet pixel data empty"))?
            .len_of(other);

        for i in 0..n_lanes {
            {
                let mut view = wavelets
                    .get_mut(region)
                    .ok_or(eyre::eyre!("Wavelet pixel data empty"))?;
                let mut lane = view.index_axis_mut(other, i);
                dwt_53_1d(&mut lane)?;
            } // mutable borrow released

            // Whole image, not just the current LL region
            save_inferno(
                &wavelets
                    .get(full)
                    .ok_or(eyre::eyre!("Wavelet pixel data empty"))?,
                &format!("images/dwt_{}.png", *counter),
            )?;
            *counter += 1;
        }
    }
    Ok(())
}

fn dwt_mallat_dump(
    wavelets: &mut WaveletPixelData,
    stages: usize,
    counter: &mut usize,
) -> eyre::Result<()> {
    for stage in 0..stages {
        dwt_53_2d_dump(wavelets, stage, counter)?;
    }
    Ok(())
}

fn save_inferno(view: &ArrayView2<i32>, path: &str) -> eyre::Result<()> {
    let (h, w) = view.dim();
    let mut img = RgbImage::new(u32::try_from(w)?, u32::try_from(h)?);

    for ((y, x), &v) in view.indexed_iter() {
        let t = f64::from(v.unsigned_abs().min(3000)) / 3000.0;
        let c = colorous::INFERNO.eval_continuous(t);
        img.put_pixel(u32::try_from(x)?, u32::try_from(y)?, Rgb([c.r, c.g, c.b]));
    }

    img.save(path)?;
    Ok(())
}
