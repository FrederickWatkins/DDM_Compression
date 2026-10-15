use crate::l0b_packet::L0bPacket;
use iced::window::{Direction, Id};
use std::path::PathBuf;

use super::workspace;

/// Events emitted by widgets and handled by the application update function.
#[derive(Debug, Clone)]
pub(super) enum Message {
    MenuHover(Option<MenuTarget>),
    WindowIdResolved(Option<Id>),
    WindowDrag,
    WindowMinimize,
    WindowToggleMaximize,
    WindowClose,
    WindowResize(Direction),
    OpenL0bRequested,
    OpenArchiveRequested,
    ExportL0bRequested,
    ExportArchiveRequested,
    L0bSelected(Option<PathBuf>),
    L0bLoaded(PathBuf, Result<Vec<L0bPacket>, String>),
    Workspace(workspace::Message),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MenuTarget {
    File,
    Open,
    Export,
}
