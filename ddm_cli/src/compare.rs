use std::{
    fmt::Display,
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    sync::Arc,
    thread::{self, JoinHandle},
};

use ddm_app::{AggregateComparisonResult, L0bFileData};
use prodash::{NestedProgress, Progress};

use crate::{
    HEIGHT, WIDTH,
    paths::display_file_name,
    table::{ComparisonInfoEntry, FileInfoEntry, format_comparison_info},
};

struct FileLoadTask {
    name: String,
    size: u64,
    loaded: JoinHandle<anyhow::Result<L0bFileData<WIDTH, HEIGHT>>>,
}

struct ComparisonTask {
    file: FileInfoEntry,
    comparison: JoinHandle<Option<AggregateComparisonResult>>,
}

/// Compare encoding and error of files. First file is the reference
/// 
/// # Errors
/// If any of the source files can't be opened or contain invalid data
pub fn compare<I: IntoIterator<Item = impl AsRef<Path>>, P>(
    files: I,
    progress: &mut P,
) -> anyhow::Result<impl Display>
where
    P: NestedProgress,
    P::SubProgress: Send + 'static,
{
    let mut reference_task = None;
    let mut other_tasks = Vec::new();

    for file in files {
        let path: PathBuf = file.as_ref().to_owned();
        let name = display_file_name(&path);
        let size = std::fs::metadata(&path)?.len();
        let mut file_progress = progress.add_child(&name);
        let loaded = thread::spawn(move || {
            let reader = BufReader::new(File::open(path)?);
            ddm_app::load_l0b::<WIDTH, HEIGHT, _>(reader, &mut file_progress)
        });
        let task = FileLoadTask { name, size, loaded };
        if reference_task.is_none() {
            reference_task = Some(task);
        } else {
            other_tasks.push(task);
        }
    }

    let reference_task =
        reference_task.ok_or_else(|| anyhow::anyhow!("At least one input file is required"))?;
    let reference = reference_task
        .loaded
        .join()
        .map_err(|_| anyhow::anyhow!("Reference loading thread panicked"))??;
    let reference_frames = Arc::new(reference.frames);

    let mut report = Vec::with_capacity(other_tasks.len() + 1);
    report.push(ComparisonInfoEntry {
        file: FileInfoEntry {
            name: reference_task.name,
            size: reference_task.size,
            info: reference.info,
        },
        comparison: None,
    });
    let mut comparisons = Vec::with_capacity(other_tasks.len());

    for task in other_tasks {
        let loaded = task
            .loaded
            .join()
            .map_err(|_| anyhow::anyhow!("File loading thread panicked"))??;
        let mut compare_progress = progress.add_child(format!("Compare {}", task.name));
        compare_progress.init(None, None);
        let reference = Arc::clone(&reference_frames);
        let comparison = thread::spawn(move || {
            ddm_app::compare(&reference, &loaded.frames, &mut compare_progress)
        });
        comparisons.push(ComparisonTask {
            file: FileInfoEntry {
                name: task.name,
                size: task.size,
                info: loaded.info,
            },
            comparison,
        });
    }

    for task in comparisons {
        let comparison = task
            .comparison
            .join()
            .map_err(|_| anyhow::anyhow!("Compare thread panicked"))?;
        report.push(ComparisonInfoEntry {
            file: task.file,
            comparison,
        });
    }

    Ok(format_comparison_info(&report))
}
