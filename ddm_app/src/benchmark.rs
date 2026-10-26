use std::io::Cursor;

use prodash::NestedProgress;

use crate::{AggregateComparisonResult, DDMFrameMap, L0bFormatConfig, compare, load_l0b, save_l0b};

/// Encode and decode a round trip with a given format and compare the results
///
/// # Errors
/// If any of the subprosses error
pub fn benchmark<const WIDTH: usize, const HEIGHT: usize, P>(
    reference: &DDMFrameMap<WIDTH, HEIGHT>,
    format: L0bFormatConfig,
    progress: &mut P,
) -> eyre::Result<(usize, AggregateComparisonResult)>
where
    P: NestedProgress,
    P::SubProgress: 'static,
{
    let mut save_progress = progress.add_child("Encode frames to stream");
    let mut load_progress = progress.add_child("Decode frames from stream");
    let mut compare_progress = progress.add_child("Compare frames");

    let mut buf = vec![];
    let mut writer = Cursor::new(&mut buf);
    save_l0b(&mut writer, reference, format, &mut save_progress)?;
    let len = buf.len();
    let mut reader = Cursor::new(&mut buf);
    let frames = load_l0b(&mut reader, &mut load_progress)?.frames;
    if let Some(comparison) = compare(reference, &frames, &mut compare_progress) {
        Ok((len, comparison))
    } else {
        Err(eyre::eyre!("Comparison returned no results"))
    }
}
