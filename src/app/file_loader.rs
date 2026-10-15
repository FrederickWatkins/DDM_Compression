use iced::Task;
use std::{fs::File, io::BufReader, path::PathBuf};

use crate::app::message::Message;

pub fn pick_l0b() -> Task<Message> {
    Task::perform(
        async {
            rfd::AsyncFileDialog::new()
                .add_filter("L0B data", &["ddm"])
                .add_filter("*", &["*"])
                .pick_file()
                .await
                .map(|file| file.path().to_owned())
        },
        Message::L0bSelected,
    )
}

pub fn load_l0b(path: PathBuf) -> Task<Message> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                let file = File::open(path).map_err(|e| e.to_string())?;
                let mut reader = BufReader::new(file);

                // Collect the iterator of results into a single Result<Vec<L0bPacket>,
                // anyhow::Error>
                crate::l0b_packet::decode_all(&mut reader)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| e.to_string())
            })
            .await
            .map_err(|e| e.to_string())? // Handles the JoinError from spawn_blocking, then the inner Result string error
        },
        Message::L0bLoaded,
    )
}
