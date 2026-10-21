//! Streaming comparison of DDM files by timestamp and channel.
use std::{fmt::Display, os::unix::fs::MetadataExt as _, path::Path};

use ddm_core::l0b::{DDMFrame, L0bPacket};
use human_format::Formatter;
use itertools::process_results;
use ndarray::{Array2, Axis, Slice};
use prodash::{NestedProgress as _, Progress as _};
use rayon::iter::{IntoParallelIterator as _, ParallelIterator as _};

use crate::{
    encoding::decode,
    file::{DDMFileInfo, load},
};

/// File metadata and aggregate comparison against the reference column.
#[derive(Debug)]
pub struct DDMComparisonColumn {
    /// File metadata gathered while reading this input.
    pub file: DDMFileInfo,
    /// Aggregate frame and pixel statistics; absent for the reference file.
    pub metrics: Option<DDMComparisonMetrics>,
}

/// Aggregate metrics for one input compared with the reference file.
#[derive(Debug, Default)]
pub struct DDMComparisonMetrics {
    /// Number of matching timestamp/channel frames.
    pub matched_frames: usize,
    /// Aggregate MSE across pixels in all matching frames.
    pub mse: Option<f64>,
    /// Aggregate RMSE.
    pub rmse: Option<f64>,
    /// Aggregate PSNR in dB, using the u16 maximum as peak signal.
    pub psnr: Option<f64>,
}

/// Aggregate file metadata and comparisons, with the first file as the reference.
#[derive(Debug)]
pub struct DDMComparisonInfo {
    /// The reference file followed by each compared file.
    pub columns: Vec<DDMComparisonColumn>,
}

impl Display for DDMComparisonInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use comfy_table::Table;
        let mut bytes = Formatter::new();
        bytes.with_decimals(2).with_units("bytes");
        let mut table = Table::new();
        let mut header = vec!["Property".to_owned()];
        header.extend(
            self.columns
                .iter()
                .map(|column| column.file.file_name.clone().to_string_lossy().into_owned()),
        );
        table.set_header(header);

        self.add_row(&mut table, "File size", |column| {
            #[expect(
                clippy::cast_precision_loss,
                reason = "File size display precision loss is acceptable"
            )]
            bytes.format(column.file.file_size as f64)
        });
        self.add_row(&mut table, "Matched frames", |column| {
            column.metrics.as_ref().map_or_else(
                || "Reference".into(),
                |metrics| metrics.matched_frames.to_string(),
            )
        });
        self.add_row(&mut table, "Aggregate MSE", |column| {
            show_metric(column.metrics.as_ref().and_then(|metrics| metrics.mse))
        });
        self.add_row(&mut table, "Aggregate RMSE", |column| {
            show_metric(column.metrics.as_ref().and_then(|metrics| metrics.rmse))
        });
        self.add_row(&mut table, "Aggregate PSNR (dB)", |column| {
            show_metric(column.metrics.as_ref().and_then(|metrics| metrics.psnr))
        });
        write!(f, "{table}")
    }
}

impl DDMComparisonInfo {
    fn add_row(
        &self,
        table: &mut comfy_table::Table,
        label: &str,
        value: impl Fn(&DDMComparisonColumn) -> String,
    ) {
        let mut row = vec![label.to_owned()];
        row.extend(self.columns.iter().map(value));
        table.add_row(row);
    }
}

fn show_metric(metric: Option<f64>) -> String {
    metric.map_or_else(
        || "None".into(),
        |value| {
            if value.is_infinite() {
                "∞".into()
            } else {
                format!("{value:.6}")
            }
        },
    )
}

/// Compare every input to the first file while interleaving their lazy frame streams.
///
/// L0b is detected from its four-byte packet magic. Other inputs are decoded as a single
/// Postcard frame. L0b frames must be ordered by GPS second; channels within a second may appear
/// in any order. Only frames from each current timestamp are buffered.
///
/// # Errors
/// Returns an error for fewer than two inputs, unreadable files, invalid encodings, out-of-order
/// frames, duplicate channels within a timestamp, or incompatible matching frame shapes.
pub fn compare<const WIDTH: usize, const HEIGHT: usize, P>(
    files: Vec<P>,
    progress: &mut impl prodash::NestedProgress,
) -> anyhow::Result<DDMComparisonInfo>
where
    P: AsRef<Path>,
{
    let mut load_bar = progress.add_child("Load files");
    let paths: Vec<_> = files
        .iter()
        .map(|file| {
            let path = file.as_ref().to_path_buf();
            (path.clone(), load_bar.add_child(path.to_string_lossy()))
        })
        .collect();
    anyhow::ensure!(
        paths.len() >= 2,
        "compare requires at least two input files"
    );
    let mut output = DDMComparisonInfo { columns: vec![] };

    let decoded: Vec<_> = paths
        .into_par_iter()
        .map(|(path, mut bar)| {
            let mut read_bar = bar.add_child("Read packets");
            let mut decode_bar = bar.add_child("Decode frames");

            let packets = load::<L0bPacket<WIDTH, HEIGHT>>(path.to_path_buf(), &mut read_bar)?;
            let output = process_results(packets, |packets| {
                let frames: Vec<DDMFrame<WIDTH, HEIGHT>> =
                    decode(packets, &mut decode_bar).collect::<Result<_, _>>()?;
                let num_frames = frames.len();

                let start_time = frames
                    .iter()
                    .map(|frame| frame.metadata().gps_seconds())
                    .min()
                    .unwrap();
                let end_time = frames
                    .iter()
                    .map(|frame| frame.metadata().gps_seconds())
                    .max()
                    .unwrap();
                let num_timestamps = (end_time - start_time) + 1;
                let mut output: Array2<Option<DDMFrame<WIDTH, HEIGHT>>> =
                    Array2::from_elem([16usize, num_timestamps as usize], None);

                for frame in frames {
                    let channel = frame.metadata().channel();
                    let timestamp = frame.metadata().gps_seconds();
                    output[(channel as usize, (timestamp - start_time) as usize)] = Some(frame);
                }

                Ok::<_, anyhow::Error>((output, start_time, num_frames))
            })??;
            bar.done("Done".to_string());
            Ok::<_, anyhow::Error>(output)
        })
        .collect::<Result<_, _>>()?;

    let reference_file = &decoded[0];
    let test_files = &decoded[1..];

    let metadata = std::fs::metadata(files[0].as_ref().to_path_buf())?;
    output.columns.push(DDMComparisonColumn {
        file: DDMFileInfo {
            file_name: files[0]
                .as_ref()
                .to_path_buf()
                .file_name()
                .unwrap_or(&files[0].as_ref().to_path_buf().into_os_string())
                .into(),
            file_size: metadata.size(),
            num_frames: reference_file.2,
        },
        metrics: None
    });

    for (file, filename) in test_files.iter().zip(files[1..].iter()) {
        let mut matched_frames = 0;
        let mut total_error = 0.0;
        let reference;
        let test_file;
        if file.1 >= reference_file.1 {
            reference = reference_file.0.view();
            test_file = file
                .0
                .slice_axis(Axis(1), Slice::from((file.1 - reference_file.1) as isize..));
        } else {
            test_file = file.0.view();
            reference = reference_file
                .0
                .slice_axis(Axis(1), Slice::from((file.1 - reference_file.1) as isize..));
        }
        for (reference_time, test_time) in reference.axis_iter(Axis(1)).zip(test_file.axis_iter(Axis(1))) {

        for (reference_frame, test_frame) in reference_time.iter().zip(test_time.iter()) {
            if let (Some(reference_frame), Some(test_frame)) = (&reference_frame, &test_frame) {
                matched_frames += 1;
                total_error += squared_error(
                    reference_frame
                        .pixel_data()
                        .iter()
                        .zip(test_frame.pixel_data().iter()),
                )
            }}
        }
        let mse = total_error / (matched_frames * WIDTH * HEIGHT) as f64;
        let psnr = psnr(mse);
        let metadata = std::fs::metadata(filename)?;
        output.columns.push(DDMComparisonColumn {
            file: DDMFileInfo {
                file_name: filename
                    .as_ref()
                    .to_path_buf()
                    .file_name()
                    .unwrap_or(&filename.as_ref().to_path_buf().into_os_string())
                    .into(),
                file_size: metadata.size(),
                num_frames: file.2,
            },
            metrics: Some(DDMComparisonMetrics {
                matched_frames,
                mse: Some(mse),
                rmse: Some(mse.sqrt()),
                psnr: Some(psnr),
            }),
        })
    }

    Ok(output)
}

fn squared_error<'a>(pairs: impl Iterator<Item = (&'a u16, &'a u16)>) -> f64 {
    pairs
        .map(|(a, b)| {
            let delta = f64::from(*a) - f64::from(*b);
            delta * delta
        })
        .sum()
}
fn psnr(mse: f64) -> f64 {
    if mse == 0.0 {
        f64::INFINITY
    } else {
        20.0 * f64::from(u16::MAX).log10() - 10.0 * mse.log10()
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        #![proptest_config(ProptestConfig { cases: 32, ..ProptestConfig::default() })]
        #[test]
        fn identical_pixels_have_zero_aggregate_error(pixels in proptest::collection::vec(any::<u16>(), 1..128)) {
            let pairs = pixels.iter().map(|value| (value, value));
            prop_assert!(squared_error(pairs).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn psnr_is_infinite_for_identical_data() {
        assert!(psnr(0.0).is_infinite());
    }

    #[test]
    fn psnr_uses_u16_peak() {
        assert!((psnr(1.0) - 96.329_466_075_304_99).abs() < 1e-10);
    }
}
