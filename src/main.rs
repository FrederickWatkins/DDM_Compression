use std::path::PathBuf;

use ddm_compression::ddm_frame::DdmFrame;
use ddm_compression::visualisation::DdmViewer;

use clap::{Parser, Subcommand};
use ndarray::{Array3, Axis};
use serde::{Serialize, de::DeserializeOwned};
use tracing::info;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Open an interactive DDM viewer
    Visualise {
        /// DDM archive to open
        #[arg(long, short)]
        source: PathBuf,
    },
    /// Compare compression ratios of different pipelines
    Benchmark {
        /// ndarray archives to benchmark with
        #[arg(long, short, action = clap::ArgAction::Append, num_args = 1..)]
        sources: Vec<PathBuf>,
    },
    /// Extract a specified portion of a DDM archive to an ndarray
    Extract {
        /// DDM archive to extract from
        #[arg(long, short)]
        source: PathBuf,
        /// ndarray archive to write to
        #[arg(long, short)]
        dest: PathBuf,
        /// Channel to extract
        #[arg(long, short)]
        channel: usize,
        /// Index to extract from
        #[arg(long, short)]
        from: usize,
        /// Number of frames to extract
        #[arg(long, short)]
        num: usize,
    },
}

pub fn load_archive<T: DeserializeOwned>(path: PathBuf) -> anyhow::Result<T> {
    info!("Loading archive {:?}", path);
    let encoded = std::fs::read(&path)?;
    let decompressed = zstd::decode_all(encoded.as_slice())?;

    let (data, _) =
        bincode::serde::decode_from_slice::<T, _>(&decompressed, bincode::config::standard())?;

    info!("Loaded archive {:?}", path);
    Ok(data)
}

pub fn write_archive<T: Serialize>(path: PathBuf, data: &T) -> anyhow::Result<()> {
    info!("Writing archive {:?}", path);
    let encoded = bincode::serde::encode_to_vec(data, bincode::config::standard())?;

    let compressed = zstd::encode_all(encoded.as_slice(), 3)?;
    std::fs::write(&path, compressed)?;

    info!("Wrote archive {:?} to disk", path);
    Ok(())
}

fn visualise(path: PathBuf) -> anyhow::Result<()> {
    let frames: Vec<Vec<DdmFrame>> = load_archive(path)?;

    let native_options = eframe::NativeOptions::default();

    info!("Starting interactive viewer");
    eframe::run_native(
        "DDM Frame Viewer",
        native_options,
        Box::new(move |_cc| Ok(Box::new(DdmViewer::new(frames)))),
    )?;
    Ok(())
}

fn benchmark(sources: Vec<PathBuf>) -> anyhow::Result<()> {
    let test_frames: Vec<Array3<u16>> = sources.iter().map(|path: &PathBuf| load_archive(path.to_path_buf())).collect::<Result<Vec<Array3<u16>>, _>>()?;
    let table = ddm_compression::benchmark::benchmark_all(&test_frames)?;
    println!("{table}");
    Ok(())
}

fn extract(
    source: PathBuf,
    dest: PathBuf,
    channel: usize,
    from: usize,
    num: usize,
) -> anyhow::Result<()> {
    let frames: Vec<Vec<DdmFrame>> = load_archive(source)?;
    let channel_frames = &frames[channel];
    let sliced_frames = &channel_frames[from..from + num];
    let mut array = Array3::from_elem([num, 52, 20], 0u16);
    for (i, frame) in sliced_frames.iter().enumerate() {
        array
            .index_axis_mut(Axis(0), i)
            .assign(&ndarray::ArrayView2::from(&frame.pixel_data));
    }
    write_archive(dest, &array)?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().init();
    let args = Cli::parse();
    match args.command {
        Some(Commands::Visualise { source }) => visualise(source),
        Some(Commands::Benchmark { sources }) => benchmark(sources),
        Some(Commands::Extract {
            source,
            dest,
            channel,
            from,
            num,
        }) => extract(source, dest, channel, from, num),
        None => Err(anyhow::Error::msg("No command given")),
    }
}
