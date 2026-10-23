use std::{fs::File, io::BufReader, path::PathBuf};

use anyhow::anyhow;
use ddm_app::{DDMFrameMap, L0bFileData};
use prodash::NestedProgress;
use rfd::AsyncFileDialog;
use tokio::task::{JoinHandle, spawn_blocking};

use crate::{HEIGHT, WIDTH};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum FileType {
    L0b,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataType {
    L0b(L0bFileData<WIDTH, HEIGHT>),
}

pub async fn load_l0b<P>(
    progress: &mut P
) -> anyhow::Result<impl Iterator<Item = anyhow::Result<JoinHandle<anyhow::Result<L0bFileData<WIDTH, HEIGHT>>>>>>
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
        Ok(spawn_blocking(move || ddm_app::load_l0b(reader, &mut bar)))
    }))
}
