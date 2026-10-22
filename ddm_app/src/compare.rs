use std::fmt::{self, Display};

use ddm_core::l0b::DDMFrame;
use ndarray::Zip;
use prodash::Progress;
use tracing::debug;

use crate::DDMFrameMap;

/// Comparison result over multiple frames with average and worst case
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct AggregateComparisonResult {
    /// Worst case
    pub worst_case: ComparisonResult,
    /// Average case
    pub mean: ComparisonResult,
}

/// Comparison result over a single frame
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct ComparisonResult {
    mse: f64,
}

impl ComparisonResult {
    /// Mean squared error
    pub fn mse(&self) -> f64 {
        self.mse
    }

    /// Root mean squared error
    pub fn rmse(&self) -> f64 {
        (self.mse).sqrt()
    }

    /// Peak signal to noise ratio
    pub fn psnr(&self) -> f64 {
        if self.mse.abs() == 0.0 {
            f64::INFINITY
        } else {
            10.0 * (f64::from(u16::MAX).powi(2) / self.mse).log10()
        }
    }
}

impl Display for ComparisonResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn format_float(value: f64, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            if value == f64::INFINITY {
                write!(f, "∞")
            } else if value == f64::NEG_INFINITY {
                write!(f, "-∞")
            } else {
                write!(f, "{value}")
            }
        }
        write!(f, "MSE: ")?;
        format_float(self.mse(), f)?;
        write!(f, "\nRMSE: ")?;
        format_float(self.rmse(), f)?;
        write!(f, "\nPSNR: ")?;
        format_float(self.psnr(), f)
    }
}

/// Compare two sets of DDM frames
#[tracing::instrument(skip_all, level = "trace")]
pub fn compare<const WIDTH: usize, const HEIGHT: usize>(
    reference: &DDMFrameMap<WIDTH, HEIGHT>,
    comparison: &DDMFrameMap<WIDTH, HEIGHT>,
    progress: &mut dyn Progress,
) -> Option<AggregateComparisonResult> {
    debug!(
        thread_id = ?thread_id::get(),
        "Started comparing frames"
    );
    let mut total_frames = 0_usize;
    let start_time = reference.start_time().max(comparison.start_time());
    let reference_end = reference
        .start_time()
        .checked_add(reference.num_timestamps())?;
    let comparison_end = comparison
        .start_time()
        .checked_add(comparison.num_timestamps())?;
    let end_time = reference_end.min(comparison_end).checked_sub(1)?;
    if end_time < start_time {
        return None;
    }
    let mut results = vec![];
    for gps_seconds in start_time..=end_time {
        for channel in 0..16 {
            progress.inc();
            if let (Some(Some(reference_frame)), Some(Some(comparison_frame))) = (
                reference.get((channel, gps_seconds)),
                comparison.get((channel, gps_seconds)),
            ) {
                total_frames = total_frames.saturating_add(1);
                results.push(compare_frames(reference_frame, comparison_frame));
            }
        }
    }
    let worst_case = results
        .iter()
        .copied()
        .max_by(|a, b| a.mse.total_cmp(&b.mse))?;
    #[expect(
        clippy::cast_precision_loss,
        reason = "The number of frames cannot practically exceed f64's exact integer range"
    )]
    let mean = ComparisonResult {
        mse: results.iter().map(|result| result.mse).sum::<f64>() / results.len() as f64,
    };
    debug!(
        thread_id = ?thread_id::get(),
        "Finished comparing {total_frames} frames"
    );
    Some(AggregateComparisonResult { worst_case, mean })
}

/// Compare two individual DDM frames
pub fn compare_frames<const WIDTH: usize, const HEIGHT: usize>(
    reference: &DDMFrame<WIDTH, HEIGHT>,
    comparison: &DDMFrame<WIDTH, HEIGHT>,
) -> ComparisonResult {
    let mut squared_error = 0.0;

    Zip::from(reference.pixel_data())
        .and(comparison.pixel_data())
        .for_each(|&reference, &comparison| {
            let difference = f64::from(reference) - f64::from(comparison);
            squared_error += difference * difference;
        });

    #[expect(
        clippy::cast_precision_loss,
        reason = "The fixed frame dimensions are used as a floating point pixel count"
    )]
    ComparisonResult {
        mse: squared_error / (WIDTH as f64).mul_add(HEIGHT as f64, 0.0),
    }
}
