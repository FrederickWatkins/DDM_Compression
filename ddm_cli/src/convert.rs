use std::{
    fmt::Display,
    fs::File,
    io::{BufReader, BufWriter},
    path::Path,
};

use color_eyre::eyre::{self, Context as _};
use ddm_app::{DDMFilters, L0bFormatConfig};
use prodash::NestedProgress;

use crate::{
    HEIGHT, WIDTH,
    paths::display_file_name,
    table::{FileInfoEntry, format_file_info},
};

/// Convert between L0b files
///
/// # Errors
/// If the source file can't be opened or the destination can't be created, or if the source file
/// doesn't contain valid L0b data
pub fn convert<P>(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    filters: &DDMFilters,
    format: &L0bFormatConfig,
    progress: &mut P,
) -> eyre::Result<impl Display>
where
    P: NestedProgress,
    P::SubProgress: Send + 'static,
{
    let input_name = display_file_name(input.as_ref());
    let output_name = display_file_name(output.as_ref());
    let input_size = std::fs::metadata(input.as_ref())
        .wrap_err_with(|| format!("Error opening file {}", input.as_ref().display()))?
        .len();
    let mut conversion_progress =
        progress.add_child(format!("Convert {input_name} into {output_name}"));
    let reader = BufReader::new(File::open(input.as_ref())?);
    let writer = BufWriter::new(File::create(output.as_ref())?);
    let result = ddm_app::convert_l0b::<WIDTH, HEIGHT, _>(
        reader,
        writer,
        filters,
        format,
        &mut conversion_progress,
    )?;
    let output_size = std::fs::metadata(output.as_ref())?.len();
    let files = [
        FileInfoEntry {
            name: format!("Original ({input_name})"),
            size: input_size,
            info: result.source,
        },
        FileInfoEntry {
            name: format!("Converted ({output_name})"),
            size: output_size,
            info: result.destination,
        },
    ];
    Ok(format_file_info(&files))
}
