use std::{
    fmt::Display,
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    thread::{self, JoinHandle},
};

use ddm_app::L0bFileInfo;
use prodash::NestedProgress;

use crate::{
    HEIGHT, WIDTH,
    paths::display_file_name,
    table::{FileInfoEntry, format_file_info},
};

struct InspectionTask {
    name: String,
    size: u64,
    inspection: JoinHandle<anyhow::Result<L0bFileInfo>>,
}

/// View metadata and content
///
/// # Errors
/// If any of the files don't exist or don't contain valid L0b data
pub fn inspect<I: IntoIterator<Item = impl AsRef<Path>>, P>(
    input: I,
    progress: &mut P,
) -> anyhow::Result<impl Display>
where
    P: NestedProgress,
    P::SubProgress: Send + 'static,
{
    let mut tasks = Vec::new();
    for file in input {
        let path: PathBuf = file.as_ref().to_owned();
        let size = std::fs::metadata(&path)?.len();
        let name = display_file_name(&path);
        let mut file_progress = progress.add_child(format!("Inspect {name}"));
        let inspection = thread::spawn(move || {
            let reader = BufReader::new(File::open(path)?);
            ddm_app::inspect_l0b::<WIDTH, HEIGHT, _>(reader, &mut file_progress)
        });
        tasks.push(InspectionTask {
            name,
            size,
            inspection,
        });
    }

    let file_info = tasks
        .into_iter()
        .map(|task| {
            let info = task
                .inspection
                .join()
                .map_err(|_| anyhow::anyhow!("Inspection thread panicked"))??;
            Ok(FileInfoEntry {
                name: task.name,
                size: task.size,
                info,
            })
        })
        .collect::<Result<Vec<_>, anyhow::Error>>()?;

    Ok(format_file_info(&file_info))
}
