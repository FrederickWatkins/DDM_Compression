//! L0b conversion pipeline.
use std::path::PathBuf;

use ddm_core::{error::Error, l0b::L0bPacket};
use itertools::process_results;

use crate::{
    DDMFilters, L0bFormatConfig, encoding,
    file::{load, save},
};

/// Convert frames that meet filters and write them using the selected L0b format.
///
/// # Errors
/// Returns an error if the input is invalid or a read, encode, or write fails.
#[tracing::instrument(skip_all)]
pub fn convert<const WIDTH: usize, const HEIGHT: usize>(
    input: PathBuf,
    output: PathBuf,
    filters: &DDMFilters,
    format: L0bFormatConfig,
    progress: &mut impl prodash::NestedProgress,
) -> Result<(), Error> {
    let mut reading = progress.add_child("Read packets");
    let mut decoding = progress.add_child("Decode frames");
    let mut encoding = progress.add_child("Encode frames");
    let mut writing = progress.add_child("Write packets");
    let packets = load::<L0bPacket<WIDTH, HEIGHT>>(input, &mut reading)?;
    process_results(packets, |packets| {
        let frames = encoding::decode::<WIDTH, HEIGHT, _>(packets, &mut decoding);
        process_results(frames, |frames| {
            let frames = frames.filter(|frame| {
                filters.channel.contains(&frame.metadata().channel())
                    && filters
                        .gps_seconds
                        .contains(&frame.metadata().gps_seconds())
            });
            let packets = encoding::encode(frames, format, &mut encoding)?;
            process_results(packets, |packets| save(output, packets, &mut writing))?
        })?
    })??;
    Ok(())
}
