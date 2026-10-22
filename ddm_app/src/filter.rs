//! DDM frame filters and range parsing.
use std::{fmt::Display, str::FromStr};

use anyhow::anyhow;
#[cfg(feature = "clap")]
use clap::Args;
use ddm_core::l0b::{DDMFrame, DDMFrameMetadata};
use prodash::Progress;
use ranges::{Domain, Ranges};
use tracing::debug;

use crate::pipeline::{PipelineInput, PipelineInputIter, PipelineOutput};

/// Parse comma-separated values and inclusive ranges, e.g. `1,2-4,5`.
///
/// # Errors
/// Returns an error if `input` is not a valid list of values and ranges.
pub fn parse_ranges<T>(input: &str) -> Result<Ranges<T>, anyhow::Error>
where
    T: Domain + FromStr + Copy,
    <T as FromStr>::Err: Display,
{
    if input.trim().is_empty() {
        return Ok(Ranges::full());
    }
    let mut ranges = Ranges::new();
    for part in input.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return Err(anyhow!("empty entry in range list"));
        }
        if let Some((start, end)) = part.split_once('-') {
            let start = start.trim();
            let end = end.trim();
            if start.is_empty() || end.is_empty() {
                return Err(anyhow!("invalid range `{part}`; expected START-END"));
            }
            let start = start
                .parse::<T>()
                .map_err(|e| anyhow!("invalid range start in `{part}`: {e}"))?;
            let end = end
                .parse::<T>()
                .map_err(|e| anyhow!("invalid range end in `{part}`: {e}"))?;
            if start > end {
                return Err(anyhow!("range start exceeds end in `{part}`"));
            }
            ranges.insert(start..=end);
        } else {
            let value = part
                .parse::<T>()
                .map_err(|e| anyhow!("invalid value `{part}`: {e}"))?;
            ranges.insert(value..=value);
        }
    }
    Ok(ranges)
}

/// Filters for DDM frames.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "clap", derive(Args))]
#[cfg_attr(feature = "clap", command(next_help_heading = "DDM filters"))]
pub struct DDMFilters {
    /// Channel filter.
    #[cfg_attr(feature = "clap", arg(long, short, default_value = "", hide_default_value = true, value_parser = parse_ranges::<u8>))]
    pub channel: Ranges<u8>,
    /// GPS timestamp filter.
    #[cfg_attr(feature = "clap", arg(long("seconds"), short('s'), default_value = "", hide_default_value = true, value_parser = parse_ranges::<u32>))]
    pub gps_seconds: Ranges<u32>,
}

impl DDMFilters {
    /// Test whether a DDM frame matches the metadata
    pub fn contains(&self, metadata: &DDMFrameMetadata) -> bool {
        self.channel.contains(&metadata.channel())
            && self.gps_seconds.contains(&metadata.gps_seconds())
    }
}

/// Keep only frames that match the selected channel and GPS second ranges.
///
/// # Errors
/// Passes up errors from the pipeline output
#[tracing::instrument(skip_all, level = "trace")]
pub fn filter<
    const WIDTH: usize,
    const HEIGHT: usize,
    I: PipelineInput<Item = DDMFrame<WIDTH, HEIGHT>>,
    O: PipelineOutput<Item = DDMFrame<WIDTH, HEIGHT>>,
>(
    input: &mut I,
    output: &mut O,
    filter: &DDMFilters,
    progress: &mut dyn Progress,
) -> anyhow::Result<()> {
    debug!(
        thread_id = ?thread_id::get(),
        "Started filtering frames"
    );
    let mut total_frames = 0_usize;
    let mut kept_frames = 0_usize;
    let input = PipelineInputIter::new(input);
    for frame in input {
        total_frames = total_frames.saturating_add(1);
        progress.inc();
        if filter.contains(&frame.metadata()) {
            kept_frames = kept_frames.saturating_add(1);
            output.send(frame)?;
        }
    }
    debug!(
        thread_id = ?thread_id::get(),
        "Finished filtering {total_frames} frames; kept {kept_frames}"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        #![proptest_config(ProptestConfig { cases: 64, .. ProptestConfig::default() })]
        #[test]
        fn range_includes_both_endpoints(a in any::<u8>(), b in any::<u8>()) {
            let start = a.min(b);
            let end = a.max(b);
            let range = parse_ranges::<u8>(&format!("{start}-{end}")).map_err(|e| TestCaseError::fail(e.to_string()))?;
            prop_assert!(range.contains(&start));
            prop_assert!(range.contains(&end));
        }
        #[test]
        fn blank_range_covers_domain(input in "\\s*") {
            let ranges = parse_ranges::<u8>(&input).map_err(|e| TestCaseError::fail(e.to_string()))?;
            prop_assert!((0..=u8::MAX).all(|value| ranges.contains(&value)));
        }
    }
}
