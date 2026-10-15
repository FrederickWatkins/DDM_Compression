use std::path::PathBuf;
use crate::l0b_packet::L0bPacket;

/// Events emitted by widgets and handled by the application update function.
#[derive(Debug, Clone)]
pub(super) enum Message {
    None,
    OpenL0bRequested,
    OpenArchiveRequested,
    L0bSelected(Option<PathBuf>),
    L0bLoaded(Result<Vec<L0bPacket>, String>),
}
