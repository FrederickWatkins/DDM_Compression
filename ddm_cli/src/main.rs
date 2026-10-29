//! DDM CLI tools
#![allow(dead_code, reason = "Might use in future")]
use std::env;

use color_eyre::eyre;
use ddm_core::coding::WaveletPixelData;
use ndarray::Array2;

mod backtrace;
use backtrace::install_error_reporting as install_eyre_hook;

mod dwt_image;
use dwt_image::FrameWriter;

use crate::dwt_image::{CHANNELS, DEFAULT_DUMP_EVERY, STAGES, channel_pixels, dwt_mallat_lockstep};

fn main() -> eyre::Result<()> {
    install_eyre_hook()?;

    let mut args = env::args().skip(1);
    let input = args
        .next()
        .ok_or(eyre::eyre!("usage: wavelet_demo <input.png> [dump_every]"))?;
    let dump_every = match args.next() {
        Some(n) => n.parse()?,
        None => DEFAULT_DUMP_EVERY,
    };

    let original = image::open(&input)?.to_rgb8();
    let mut channels: Vec<_> = (0..CHANNELS)
        .map(|c| WaveletPixelData::new(channel_pixels(&original, c)))
        .collect();

    let mut frames = FrameWriter::new("images", dump_every)?;
    frames.dump(&channels)?; // untransformed starting frame
    dwt_mallat_lockstep(&mut channels, STAGES, |chs| frames.on_lane(chs))?;
    frames.dump(&channels)?; // fully transformed final frame

    println!("wrote {} frames to images/", frames.files_written);
    Ok(())
}

fn print_ndarray_8bit(arr: &Array2<u16>) {
    let (rows, cols) = arr.dim();

    // Iterate through rows with a step of 2 to pair vertical pixels using '▀'
    let mut r = 0;
    while r < rows {
        for c in 0..cols {
            let top_val = arr[[r, c]];
            let top_color = map_to_8bit(top_val);

            if r + 1 < rows {
                let bot_val = arr[[r + 1, c]];
                let bot_color = map_to_8bit(bot_val);

                // \x1b[38;5;{n}m sets foreground color (top half)
                // \x1b[48;5;{n}m sets background color (bottom half)
                print!("\x1b[38;5;{top_color}m\x1b[48;5;{bot_color}m▀");
            } else {
                // Handle odd-numbered total row count
                print!("\x1b[38;5;{top_color}m▀");
            }
        }
        // Reset terminal colors at the end of the line
        println!("\x1b[0m");
        r += 2;
    }
}

/// Maps a u16 value between 0 and 2048 to an 8-bit ANSI color code (0-255)
#[expect(clippy::cast_possible_truncation, reason = "Value is clamped")]
fn map_to_8bit(val: u16) -> u8 {
    let normalized = val.min(4096) / (4096 / 256);
    normalized as u8
}
