use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use ddm_app::L0bFileData;
use prodash::progress::Discard;

pub(crate) const FRAME_WIDTH: usize = 20;
pub(crate) const FRAME_HEIGHT: usize = 52;
pub(crate) const CHANNEL_COUNT: u8 = 16;

pub(crate) struct LoadedFile {
    pub(crate) name: String,
    pub(crate) data: Arc<L0bFileData<FRAME_WIDTH, FRAME_HEIGHT>>,
}

pub(crate) async fn choose_l0b_files() -> Option<Vec<PathBuf>> {
    rfd::AsyncFileDialog::new()
        .add_filter("L0b DDM files", &["l0b.ddm"])
        .set_title("Open L0b files")
        .pick_files()
        .await
        .map(|files| {
            files
                .into_iter()
                .map(|file| file.path().to_path_buf())
                .collect()
        })
}

pub(crate) fn load_l0b_file(path: &Path) -> Result<LoadedFile> {
    let name = path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let file = File::open(path).with_context(|| format!("Could not open {}", path.display()))?;
    let mut progress = Discard;
    let data =
        ddm_app::load_l0b::<FRAME_WIDTH, FRAME_HEIGHT, _>(BufReader::new(file), &mut progress)
            .with_context(|| format!("Could not decode {}", path.display()))?;

    Ok(LoadedFile {
        name,
        data: Arc::new(data),
    })
}
