use crate::l0b_packet::L0bPacket;
use std::path::PathBuf;

use super::workspace;

/// Events emitted by widgets and handled by the application update function.
#[derive(Debug, Clone)]
pub(super) enum Message {
    None,
    OpenL0bRequested,
    OpenArchiveRequested,
    L0bSelected(Option<PathBuf>),
    L0bLoaded(PathBuf, Result<Vec<L0bPacket>, String>),
    Workspace(workspace::Message),
}
