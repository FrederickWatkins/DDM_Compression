use crate::{app::message::Message, l0b_packet::L0bPacket};
use iced::Task;
use std::{fs::File, io::BufReader, path::PathBuf};

pub(super) fn pick_l0b() -> Task<Message> {
    tracing::debug!("opening L0B file picker");
    Task::perform(pick_l0b_path(), Message::L0bSelected)
}

async fn pick_l0b_path() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .add_filter("L0B data", &["ddm"])
        .add_filter("*", &["*"])
        .pick_file()
        .await
        .map(|file| file.path().to_owned())
}

pub(super) fn load_l0b(path: PathBuf) -> Task<Message> {
    Task::perform(load_l0b_packets(path.clone()), move |result| {
        Message::L0bLoaded(path, result)
    })
}

async fn load_l0b_packets(path: PathBuf) -> Result<Vec<L0bPacket>, String> {
    tokio::task::spawn_blocking(move || decode_l0b(path))
        .await
        .map_err(|error| error.to_string())?
}

fn decode_l0b(path: PathBuf) -> Result<Vec<L0bPacket>, String> {
    tracing::debug!(path = %path.display(), "decoding L0B packets");
    let file = File::open(path).map_err(|error| error.to_string())?;
    let mut reader = BufReader::new(file);

    let packets = crate::l0b_packet::decode_all(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    tracing::debug!(
        packet_count = packets.len(),
        "finished decoding L0B packets"
    );
    Ok(packets)
}
