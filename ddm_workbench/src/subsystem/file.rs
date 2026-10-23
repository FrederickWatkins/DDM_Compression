//! File subsystem
use std::{fs::File, io::BufReader, sync::Arc};

use anyhow::anyhow;
use prodash::NestedProgress;
use rfd::AsyncFileDialog;
use tokio::task::{JoinHandle, spawn_blocking};

use crate::L0bFileData;

/// Possible file types to select
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum FileType {
    /// Raw L0b data
    L0b,
}

/// Possible data types to load
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataType {
    /// Loaded and mapped L0b data
    L0b(Arc<L0bFileData>),
}

/// Load L0b data
/// 
/// # Errors
/// Errors if any of the underlying processes error
#[tracing::instrument(skip_all, level = "trace")]
pub async fn load_l0b<P>(
    progress: &mut P
) -> anyhow::Result<impl Iterator<Item = anyhow::Result<JoinHandle<anyhow::Result<(String, L0bFileData)>>>>>
where P: NestedProgress, P::SubProgress: 'static {
    let files = AsyncFileDialog::new()
        .add_filter("L0b", &["l0b.ddm"])
        .add_filter("*", &["*"])
        .set_title("Open L0b file")
        .pick_files()
        .await;
    let Some(paths) = files else {
        return Err(anyhow!("No file selected"));
    };

    Ok(paths.into_iter().map(|path| {
        let file_name = path.file_name();
        let mut bar = progress.add_child(format!("load {file_name}"));
        let reader = BufReader::new(File::open(path.path())?);
        Ok(spawn_blocking(move || Ok((file_name, ddm_app::load_l0b(reader, &mut bar)?))))
    }))
}
